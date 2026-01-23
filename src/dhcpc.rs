use anyhow::{Result, anyhow};
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
use std::time::{Duration, Instant};

pub struct DhcpInfo {
    pub ip: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub gateway: Ipv4Addr,
    pub _dns: Vec<Ipv4Addr>,
}

pub async fn dhcp_client(iface_name: &str) -> Result<DhcpInfo> {
    let interface = datalink::interfaces()
        .into_iter()
        .find(|iface| iface.name == iface_name)
        .ok_or_else(|| anyhow!("Interface not found"))?;
    // println!("Using interface: {:?}", interface);

    let macaddr = interface
        .mac
        .ok_or_else(|| anyhow!("No MAC address found"))?;
    let chaddr = macaddr.octets().to_vec();

    let (mut tx, mut rx) = match datalink::channel(&interface, Default::default())? {
        Ethernet(tx, rx) => (tx, rx),
        _ => return Err(anyhow!("Unsupported channel type")),
    };

    // --- DHCP Discover ---
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

    send_dhcp_message(&discover_msg, macaddr, &mut tx)?;
    // println!("[*] DHCP Discover sent");

    let offer_msg = recv_dhcp_message(&mut rx, MessageType::Offer)
        .ok_or_else(|| anyhow!("No DHCP Offer received"))?;
    // println!("[+] DHCP Offer received: {:?}", offer_msg);

    let offered_ip = offer_msg.yiaddr();
    let server_id = match offer_msg.opts().get(OptionCode::ServerIdentifier) {
        Some(DhcpOption::ServerIdentifier(ip)) => Ipv4Addr::from(*ip),
        _ => return Err(anyhow!("Invalid Server Identifier in DHCP Offer")),
    };

    // --- DHCP Request ---
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

    send_dhcp_message(&request_msg, macaddr, &mut tx)?;
    // println!("[*] DHCP Request sent");

    let ack_msg = recv_dhcp_message(&mut rx, MessageType::Ack)
        .ok_or_else(|| anyhow!("No DHCP Ack received"))?;
    // println!("[+] DHCP Ack received: {:?}", ack_msg);

    Ok(DhcpInfo {
        ip: offered_ip,
        netmask: match ack_msg.opts().get(OptionCode::SubnetMask) {
            Some(DhcpOption::SubnetMask(mask)) => Ipv4Addr::from(*mask),
            _ => Ipv4Addr::new(255, 255, 255, 0),
        },
        gateway: match ack_msg.opts().get(OptionCode::Router) {
            Some(DhcpOption::Router(routers)) => routers
                .first()
                .cloned()
                .unwrap_or(Ipv4Addr::new(0, 0, 0, 0)),
            _ => Ipv4Addr::new(0, 0, 0, 0),
        },
        _dns: match ack_msg.opts().get(OptionCode::DomainNameServer) {
            Some(DhcpOption::DomainNameServer(servers)) => {
                servers.iter().map(|ip| Ipv4Addr::from(*ip)).collect()
            }
            _ => Vec::new(),
        },
    })
}

fn build_eth_ipv4_udp(dhcp_buf: &[u8], macaddr: MacAddr) -> Vec<u8> {
    // UDP
    let mut udp_buf = vec![0u8; 8 + dhcp_buf.len()];
    {
        let mut udp = MutableUdpPacket::new(&mut udp_buf).unwrap();
        udp.set_source(68);
        udp.set_destination(67);
        udp.set_length((8 + dhcp_buf.len()) as u16);
        udp.set_payload(dhcp_buf);
    }

    // IP
    let src_ip = Ipv4Addr::new(0, 0, 0, 0);
    let dst_ip = Ipv4Addr::new(255, 255, 255, 255);
    let mut ip_buf = vec![0u8; 20 + udp_buf.len()];
    {
        let mut ip = MutableIpv4Packet::new(&mut ip_buf).unwrap();
        ip.set_version(4);
        ip.set_header_length(5);
        ip.set_total_length((20 + udp_buf.len()) as u16);
        ip.set_ttl(64);
        ip.set_next_level_protocol(IpNextHeaderProtocols::Udp);
        ip.set_source(src_ip);
        ip.set_destination(dst_ip);

        // UDP checksum
        let udp_checksum =
            pnet::packet::udp::ipv4_checksum(&UdpPacket::new(&udp_buf).unwrap(), &src_ip, &dst_ip);
        {
            let mut udp = MutableUdpPacket::new(&mut udp_buf).unwrap();
            udp.set_checksum(udp_checksum);
        }
        ip.set_payload(&udp_buf);

        // IP checksum
        let ip_checksum = checksum(&ip.to_immutable());
        ip.set_checksum(ip_checksum);
    }

    // Ethernet
    let mut eth_buf = vec![0u8; 14 + ip_buf.len()];
    {
        let mut eth = MutableEthernetPacket::new(&mut eth_buf).unwrap();
        eth.set_source(macaddr);
        eth.set_destination(pnet::util::MacAddr::broadcast());
        eth.set_ethertype(EtherTypes::Ipv4);
        eth.set_payload(&ip_buf);
    }

    eth_buf
}

fn send_dhcp_message(
    msg: &Message,
    macaddr: MacAddr,
    tx: &mut Box<dyn DataLinkSender>,
) -> Result<()> {
    let mut buf = Vec::new();
    msg.encode(&mut Encoder::new(&mut buf))?;
    let eth_frame = build_eth_ipv4_udp(&buf, macaddr);
    tx.send_to(&eth_frame, None)
        .ok_or_else(|| anyhow!("Failed to send packet"))??;
    Ok(())
}

fn recv_dhcp_message(rx: &mut Box<dyn DataLinkReceiver>, msg_type: MessageType) -> Option<Message> {
    let deadline = Instant::now() + Duration::from_secs(10);

    while Instant::now() < deadline {
        if let Ok(packet) = rx.next() {
            if let Some(ipv4) = Ipv4Packet::new(&packet[14..]) {
                if ipv4.get_next_level_protocol() != IpNextHeaderProtocols::Udp {
                    continue;
                }
                if let Some(udp) = UdpPacket::new(ipv4.payload()) {
                    if udp.get_source() != 67 || udp.get_destination() != 68 {
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
    }

    None
}
