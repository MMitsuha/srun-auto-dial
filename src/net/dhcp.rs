use crate::error::{Result, SrunError};
use dhcproto::v4::{DhcpOption, Flags, Message, MessageType, OptionCode};
use dhcproto::{Decodable, Decoder, Encodable, Encoder};
use pnet::datalink::{self, Channel::Ethernet, DataLinkReceiver, DataLinkSender};
use pnet::packet::Packet;
use pnet::packet::ethernet::{EtherTypes, MutableEthernetPacket};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::ipv4::{Ipv4Packet, MutableIpv4Packet, checksum};
use pnet::packet::udp::{MutableUdpPacket, UdpPacket};
use pnet::util::MacAddr;
use std::net::Ipv4Addr;
use std::time::Duration;
use tracing::{debug, info, warn};

const DHCP_CLIENT_PORT: u16 = 68;
const DHCP_SERVER_PORT: u16 = 67;
const ETH_HEADER_LEN: usize = 14;
const MAX_RETRIES: u32 = 3;
const RECV_TIMEOUT: Duration = Duration::from_secs(1);
const OVERALL_TIMEOUT: Duration = Duration::from_secs(15);

pub struct DhcpInfo {
    pub ip: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub gateway: Ipv4Addr,
}

pub async fn dhcp_client(iface_name: &str) -> Result<DhcpInfo> {
    // Wrap blocking DHCP in a tokio blocking task with overall timeout
    let iface = iface_name.to_string();
    let result = tokio::time::timeout(OVERALL_TIMEOUT, tokio::task::spawn_blocking(move || {
        dhcp_client_blocking(&iface)
    }))
    .await
    .map_err(|_| SrunError::Dhcp(format!("DHCP timeout after {}s", OVERALL_TIMEOUT.as_secs())))?
    .map_err(|e| SrunError::Dhcp(format!("DHCP task failed: {}", e)))?;

    result
}

fn dhcp_client_blocking(iface_name: &str) -> Result<DhcpInfo> {
    let interface = datalink::interfaces()
        .into_iter()
        .find(|iface| iface.name == iface_name)
        .ok_or_else(|| SrunError::InterfaceNotFound(iface_name.to_string()))?;

    let macaddr = interface
        .mac
        .ok_or_else(|| SrunError::Dhcp("no MAC address on interface".to_string()))?;
    let chaddr = macaddr.octets().to_vec();

    let config = pnet::datalink::Config {
        read_timeout: Some(RECV_TIMEOUT),
        ..Default::default()
    };

    let (mut tx, mut rx) = match datalink::channel(&interface, config)
        .map_err(|e| SrunError::Dhcp(format!("failed to open datalink channel: {}", e)))?
    {
        Ethernet(tx, rx) => (tx, rx),
        _ => return Err(SrunError::Dhcp("unsupported channel type".to_string())),
    };

    // --- DHCP Discover with retry ---
    let mut discover_msg = Message::default();
    discover_msg
        .set_flags(Flags::default().set_broadcast())
        .set_chaddr(&chaddr)
        .opts_mut()
        .insert(DhcpOption::MessageType(MessageType::Discover));
    discover_msg
        .opts_mut()
        .insert(DhcpOption::ParameterRequestList(vec![
            OptionCode::SubnetMask,
            OptionCode::Router,
            OptionCode::DomainNameServer,
            OptionCode::DomainName,
        ]));
    discover_msg
        .opts_mut()
        .insert(DhcpOption::ClientIdentifier(chaddr.clone()));

    let mut offer_msg = None;
    for attempt in 1..=MAX_RETRIES {
        send_dhcp_message(&discover_msg, macaddr, &mut tx)?;
        debug!(attempt = attempt, "DHCP Discover sent");

        if let Some(msg) = recv_dhcp_message(&mut rx, MessageType::Offer) {
            debug!("DHCP Offer received");
            offer_msg = Some(msg);
            break;
        }
        warn!(attempt = attempt, "no DHCP Offer received, retrying");
    }

    let offer_msg = offer_msg.ok_or_else(|| {
        SrunError::Dhcp(format!(
            "no DHCP Offer after {} attempts",
            MAX_RETRIES
        ))
    })?;

    let offered_ip = offer_msg.yiaddr();
    let server_id = match offer_msg.opts().get(OptionCode::ServerIdentifier) {
        Some(DhcpOption::ServerIdentifier(ip)) => Ipv4Addr::from(*ip),
        _ => {
            return Err(SrunError::Dhcp(
                "missing Server Identifier in Offer".to_string(),
            ))
        }
    };

    // --- DHCP Request with retry ---
    let mut request_msg = Message::default();
    request_msg
        .set_flags(Flags::default().set_broadcast())
        .set_chaddr(&macaddr.octets())
        .opts_mut()
        .insert(DhcpOption::MessageType(MessageType::Request));
    request_msg
        .opts_mut()
        .insert(DhcpOption::ParameterRequestList(vec![
            OptionCode::SubnetMask,
            OptionCode::Router,
            OptionCode::DomainNameServer,
            OptionCode::DomainName,
        ]));
    request_msg
        .opts_mut()
        .insert(DhcpOption::RequestedIpAddress(offered_ip));
    request_msg
        .opts_mut()
        .insert(DhcpOption::ServerIdentifier(server_id));

    let mut ack_msg = None;
    for attempt in 1..=MAX_RETRIES {
        send_dhcp_message(&request_msg, macaddr, &mut tx)?;
        debug!(attempt = attempt, "DHCP Request sent");

        if let Some(msg) = recv_dhcp_message(&mut rx, MessageType::Ack) {
            debug!("DHCP Ack received");
            ack_msg = Some(msg);
            break;
        }
        warn!(attempt = attempt, "no DHCP Ack received, retrying");
    }

    let ack_msg = ack_msg.ok_or_else(|| {
        SrunError::Dhcp(format!("no DHCP Ack after {} attempts", MAX_RETRIES))
    })?;

    let netmask = match ack_msg.opts().get(OptionCode::SubnetMask) {
        Some(DhcpOption::SubnetMask(mask)) => Ipv4Addr::from(*mask),
        _ => Ipv4Addr::new(255, 255, 255, 0),
    };
    let gateway = match ack_msg.opts().get(OptionCode::Router) {
        Some(DhcpOption::Router(routers)) => routers
            .first()
            .cloned()
            .unwrap_or(Ipv4Addr::UNSPECIFIED),
        _ => Ipv4Addr::UNSPECIFIED,
    };

    info!(ip = %offered_ip, netmask = %netmask, gateway = %gateway, "DHCP completed");

    Ok(DhcpInfo {
        ip: offered_ip,
        netmask,
        gateway,
    })
}

fn build_eth_ipv4_udp(dhcp_buf: &[u8], macaddr: MacAddr) -> Result<Vec<u8>> {
    // UDP
    let udp_len = 8 + dhcp_buf.len();
    let mut udp_buf = vec![0u8; udp_len];
    {
        let mut udp = MutableUdpPacket::new(&mut udp_buf).ok_or(SrunError::PacketBuild)?;
        udp.set_source(DHCP_CLIENT_PORT);
        udp.set_destination(DHCP_SERVER_PORT);
        udp.set_length(udp_len as u16);
        udp.set_payload(dhcp_buf);
    }

    // IPv4
    let src_ip = Ipv4Addr::UNSPECIFIED;
    let dst_ip = Ipv4Addr::BROADCAST;
    let ip_len = 20 + udp_buf.len();
    let mut ip_buf = vec![0u8; ip_len];
    {
        let mut ip = MutableIpv4Packet::new(&mut ip_buf).ok_or(SrunError::PacketBuild)?;
        ip.set_version(4);
        ip.set_header_length(5);
        ip.set_total_length(ip_len as u16);
        ip.set_ttl(64);
        ip.set_next_level_protocol(IpNextHeaderProtocols::Udp);
        ip.set_source(src_ip);
        ip.set_destination(dst_ip);

        // UDP checksum
        let udp_cksum = pnet::packet::udp::ipv4_checksum(
            &UdpPacket::new(&udp_buf).ok_or(SrunError::PacketBuild)?,
            &src_ip,
            &dst_ip,
        );
        {
            let mut udp = MutableUdpPacket::new(&mut udp_buf).ok_or(SrunError::PacketBuild)?;
            udp.set_checksum(udp_cksum);
        }
        ip.set_payload(&udp_buf);

        let ip_cksum = checksum(&ip.to_immutable());
        ip.set_checksum(ip_cksum);
    }

    // Ethernet
    let eth_len = ETH_HEADER_LEN + ip_buf.len();
    let mut eth_buf = vec![0u8; eth_len];
    {
        let mut eth = MutableEthernetPacket::new(&mut eth_buf).ok_or(SrunError::PacketBuild)?;
        eth.set_source(macaddr);
        eth.set_destination(MacAddr::broadcast());
        eth.set_ethertype(EtherTypes::Ipv4);
        eth.set_payload(&ip_buf);
    }

    Ok(eth_buf)
}

fn send_dhcp_message(
    msg: &Message,
    macaddr: MacAddr,
    tx: &mut Box<dyn DataLinkSender>,
) -> Result<()> {
    let mut buf = Vec::new();
    msg.encode(&mut Encoder::new(&mut buf))
        .map_err(|e| SrunError::Dhcp(format!("DHCP encode error: {}", e)))?;
    let eth_frame = build_eth_ipv4_udp(&buf, macaddr)?;
    tx.send_to(&eth_frame, None)
        .ok_or(SrunError::Dhcp("send_to returned None".to_string()))?
        .map_err(|e| SrunError::Dhcp(format!("send error: {}", e)))?;
    Ok(())
}

fn recv_dhcp_message(rx: &mut Box<dyn DataLinkReceiver>, msg_type: MessageType) -> Option<Message> {
    // Try receiving for a few seconds (relying on read_timeout set on the channel)
    let deadline = std::time::Instant::now() + Duration::from_secs(5);

    while std::time::Instant::now() < deadline {
        match rx.next() {
            Ok(packet) => {
                if packet.len() < ETH_HEADER_LEN + 20 {
                    continue;
                }
                if let Some(ipv4) = Ipv4Packet::new(&packet[ETH_HEADER_LEN..]) {
                    if ipv4.get_next_level_protocol() != IpNextHeaderProtocols::Udp {
                        continue;
                    }
                    if let Some(udp) = UdpPacket::new(ipv4.payload()) {
                        if udp.get_source() != DHCP_SERVER_PORT
                            || udp.get_destination() != DHCP_CLIENT_PORT
                        {
                            continue;
                        }
                        if let Ok(msg) = Message::decode(&mut Decoder::new(udp.payload())) {
                            if msg.opts().msg_type() == Some(msg_type) {
                                return Some(msg);
                            }
                        }
                    }
                }
            }
            Err(_) => {
                // read_timeout elapsed, loop will check deadline
                continue;
            }
        }
    }

    None
}
