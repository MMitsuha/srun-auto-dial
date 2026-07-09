use crate::error::{Result, SrunError};
use dhcproto::v4::{DhcpOption, Flags, Message, MessageType, OptionCode};
use dhcproto::{Decodable, Decoder, Encodable, Encoder};
use pnet::datalink::{self, Channel::Ethernet, DataLinkReceiver, DataLinkSender};
use pnet::packet::Packet;
use pnet::packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::ipv4::{Ipv4Packet, MutableIpv4Packet, checksum};
use pnet::packet::udp::{MutableUdpPacket, UdpPacket};
use pnet::util::MacAddr;
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

const DHCP_CLIENT_PORT: u16 = 68;
const DHCP_SERVER_PORT: u16 = 67;
const ETH_HEADER_LEN: usize = 14;
const MAX_RETRIES: u32 = 3;
const REPLY_TIMEOUT: Duration = Duration::from_secs(2);
const IO_POLL_TIMEOUT: Duration = Duration::from_millis(200);
const WRITE_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DhcpInfo {
    pub ip: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub gateway: Ipv4Addr,
}

pub async fn dhcp_client(iface_name: &str) -> Result<DhcpInfo> {
    let iface = iface_name.to_string();
    tokio::task::spawn_blocking(move || dhcp_client_blocking(&iface))
        .await
        .map_err(|e| SrunError::Dhcp(format!("DHCP task failed: {}", e)))?
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
        read_timeout: Some(IO_POLL_TIMEOUT),
        write_timeout: Some(WRITE_TIMEOUT),
        ..Default::default()
    };

    let (mut tx, mut rx) = match datalink::channel(&interface, config)
        .map_err(|e| SrunError::Dhcp(format!("failed to open datalink channel: {}", e)))?
    {
        Ethernet(tx, rx) => (tx, rx),
        _ => return Err(SrunError::Dhcp("unsupported channel type".to_string())),
    };

    let xid = rand::random::<u32>();

    // --- DHCP Discover with retry ---
    let discover_msg = discover_message(xid, &chaddr);

    let mut offer_msg = None;
    for attempt in 1..=MAX_RETRIES {
        send_dhcp_message(&discover_msg, macaddr, &mut tx)?;
        debug!(attempt = attempt, "DHCP Discover sent");

        match recv_dhcp_message(&mut rx, MessageType::Offer, xid, &chaddr, REPLY_TIMEOUT)? {
            ReceiveOutcome::Reply(msg) => {
                debug!("DHCP Offer received");
                offer_msg = Some(msg);
                break;
            }
            ReceiveOutcome::Nak(msg) => {
                return Err(SrunError::DhcpRejected(nak_reason(&msg)));
            }
            ReceiveOutcome::Timeout => {
                warn!(
                    attempt = attempt,
                    max_attempts = MAX_RETRIES,
                    "DHCP Offer attempt timed out"
                );
            }
        }
    }

    let offer_msg = offer_msg.ok_or(SrunError::DhcpTimeout {
        phase: "offer",
        attempts: MAX_RETRIES,
    })?;

    let offered_ip = offered_ip(&offer_msg)?;
    let server_id = match offer_msg.opts().get(OptionCode::ServerIdentifier) {
        Some(DhcpOption::ServerIdentifier(ip)) if !ip.is_unspecified() => *ip,
        Some(DhcpOption::ServerIdentifier(_)) => {
            return Err(SrunError::Dhcp(
                "DHCP Offer contained an unspecified Server Identifier".to_string(),
            ));
        }
        _ => {
            return Err(SrunError::Dhcp(
                "missing Server Identifier in Offer".to_string(),
            ));
        }
    };

    // --- DHCP Request with retry ---
    let request_msg = request_message(xid, &chaddr, offered_ip, server_id);

    let mut ack_msg = None;
    for attempt in 1..=MAX_RETRIES {
        send_dhcp_message(&request_msg, macaddr, &mut tx)?;
        debug!(attempt = attempt, "DHCP Request sent");

        match recv_dhcp_message(&mut rx, MessageType::Ack, xid, &chaddr, REPLY_TIMEOUT)? {
            ReceiveOutcome::Reply(msg) => {
                debug!("DHCP Ack received");
                ack_msg = Some(msg);
                break;
            }
            ReceiveOutcome::Nak(msg) => {
                return Err(SrunError::DhcpRejected(nak_reason(&msg)));
            }
            ReceiveOutcome::Timeout => {
                warn!(
                    attempt = attempt,
                    max_attempts = MAX_RETRIES,
                    "DHCP Ack attempt timed out"
                );
            }
        }
    }

    let ack_msg = ack_msg.ok_or(SrunError::DhcpTimeout {
        phase: "ack",
        attempts: MAX_RETRIES,
    })?;
    let lease = validated_lease(offered_ip, &ack_msg)?;

    info!(ip = %lease.ip, netmask = %lease.netmask, gateway = %lease.gateway, "DHCP completed");

    Ok(lease)
}

fn client_message(xid: u32, chaddr: &[u8], msg_type: MessageType) -> Message {
    let mut msg = Message::default();
    msg.set_xid(xid)
        .set_flags(Flags::default().set_broadcast())
        .set_chaddr(chaddr);
    msg.opts_mut().insert(DhcpOption::MessageType(msg_type));
    msg.opts_mut().insert(DhcpOption::ParameterRequestList(vec![
        OptionCode::SubnetMask,
        OptionCode::Router,
        OptionCode::DomainNameServer,
        OptionCode::DomainName,
    ]));
    msg.opts_mut()
        .insert(DhcpOption::ClientIdentifier(chaddr.to_vec()));
    msg
}

fn discover_message(xid: u32, chaddr: &[u8]) -> Message {
    client_message(xid, chaddr, MessageType::Discover)
}

fn request_message(xid: u32, chaddr: &[u8], offered_ip: Ipv4Addr, server_id: Ipv4Addr) -> Message {
    let mut msg = client_message(xid, chaddr, MessageType::Request);
    msg.opts_mut()
        .insert(DhcpOption::RequestedIpAddress(offered_ip));
    msg.opts_mut()
        .insert(DhcpOption::ServerIdentifier(server_id));
    msg
}

fn offered_ip(offer: &Message) -> Result<Ipv4Addr> {
    let ip = offer.yiaddr();
    if ip.is_unspecified() {
        return Err(SrunError::Dhcp(
            "DHCP Offer did not include a usable offered IP address".to_string(),
        ));
    }
    Ok(ip)
}

fn validated_lease(offered_ip: Ipv4Addr, ack: &Message) -> Result<DhcpInfo> {
    if offered_ip.is_unspecified() {
        return Err(SrunError::Dhcp(
            "DHCP Offer did not include a usable offered IP address".to_string(),
        ));
    }

    let netmask = match ack.opts().get(OptionCode::SubnetMask) {
        Some(DhcpOption::SubnetMask(mask)) if mask.is_unspecified() => {
            return Err(SrunError::Dhcp(
                "DHCP Ack contained an unspecified subnet mask".to_string(),
            ));
        }
        Some(DhcpOption::SubnetMask(mask)) => *mask,
        _ => {
            return Err(SrunError::Dhcp(
                "missing Subnet Mask in DHCP Ack".to_string(),
            ));
        }
    };
    if netmask_prefix_len(netmask).is_none() {
        return Err(SrunError::Dhcp(format!(
            "DHCP Ack contained a non-contiguous subnet mask: {netmask}"
        )));
    }

    let gateway = match ack.opts().get(OptionCode::Router) {
        Some(DhcpOption::Router(routers)) => routers.first().copied().ok_or_else(|| {
            SrunError::Dhcp("DHCP Ack contained an empty Router option".to_string())
        })?,
        _ => {
            return Err(SrunError::Dhcp("missing Router in DHCP Ack".to_string()));
        }
    };
    if gateway.is_unspecified() {
        return Err(SrunError::Dhcp(
            "DHCP Ack contained an unspecified router".to_string(),
        ));
    }

    Ok(DhcpInfo {
        ip: offered_ip,
        netmask,
        gateway,
    })
}

fn netmask_prefix_len(netmask: Ipv4Addr) -> Option<u8> {
    let bits = u32::from(netmask);
    let host_bits = !bits;
    (host_bits & host_bits.wrapping_add(1) == 0).then_some(bits.count_ones() as u8)
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

#[derive(Debug, PartialEq, Eq)]
enum ReplyDisposition {
    Accept,
    Reject,
    Ignore,
}

enum ReceiveOutcome {
    Reply(Message),
    Nak(Message),
    Timeout,
}

fn classify_reply(
    msg: &Message,
    expected_type: MessageType,
    xid: u32,
    chaddr: &[u8],
) -> ReplyDisposition {
    if msg.xid() != xid || msg.chaddr() != chaddr {
        return ReplyDisposition::Ignore;
    }

    match msg.opts().msg_type() {
        Some(msg_type) if msg_type == expected_type => ReplyDisposition::Accept,
        Some(MessageType::Nak) => ReplyDisposition::Reject,
        _ => ReplyDisposition::Ignore,
    }
}

fn nak_reason(msg: &Message) -> String {
    match msg.opts().get(OptionCode::Message) {
        Some(DhcpOption::Message(reason)) if !reason.trim().is_empty() => reason.clone(),
        _ => "the DHCP server rejected the requested address".to_string(),
    }
}

fn recv_dhcp_message(
    rx: &mut Box<dyn DataLinkReceiver>,
    expected_type: MessageType,
    xid: u32,
    chaddr: &[u8],
    timeout: Duration,
) -> Result<ReceiveOutcome> {
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        match rx.next() {
            Ok(packet) => {
                let Some(ethernet) = EthernetPacket::new(packet) else {
                    continue;
                };
                if ethernet.get_ethertype() != EtherTypes::Ipv4 {
                    continue;
                }
                let Some(ipv4) = Ipv4Packet::new(ethernet.payload()) else {
                    continue;
                };
                if ipv4.get_next_level_protocol() != IpNextHeaderProtocols::Udp {
                    continue;
                }
                let Some(udp) = UdpPacket::new(ipv4.payload()) else {
                    continue;
                };
                if udp.get_source() != DHCP_SERVER_PORT || udp.get_destination() != DHCP_CLIENT_PORT
                {
                    continue;
                }
                if let Ok(msg) = Message::decode(&mut Decoder::new(udp.payload())) {
                    match classify_reply(&msg, expected_type, xid, chaddr) {
                        ReplyDisposition::Accept => return Ok(ReceiveOutcome::Reply(msg)),
                        ReplyDisposition::Reject => return Ok(ReceiveOutcome::Nak(msg)),
                        ReplyDisposition::Ignore => {}
                    }
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                // The short socket poll elapsed; keep waiting until this attempt's deadline.
            }
            Err(error) => {
                return Err(SrunError::Dhcp(format!(
                    "failed to receive DHCP response: {error}"
                )));
            }
        }
    }

    Ok(ReceiveOutcome::Timeout)
}

#[cfg(test)]
mod tests {
    use super::*;

    const XID: u32 = 0x1020_3040;
    const CLIENT_MAC: [u8; 6] = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];

    fn reply(msg_type: MessageType) -> Message {
        let mut msg = Message::default();
        msg.set_xid(XID).set_chaddr(&CLIENT_MAC);
        msg.opts_mut().insert(DhcpOption::MessageType(msg_type));
        msg
    }

    fn valid_ack() -> Message {
        let mut ack = reply(MessageType::Ack);
        ack.opts_mut()
            .insert(DhcpOption::SubnetMask(Ipv4Addr::new(255, 255, 255, 0)));
        ack.opts_mut()
            .insert(DhcpOption::Router(vec![Ipv4Addr::new(192, 0, 2, 1)]));
        ack
    }

    #[test]
    fn discover_and_request_share_transaction_and_client_identity() {
        let offered_ip = Ipv4Addr::new(192, 0, 2, 10);
        let server_id = Ipv4Addr::new(192, 0, 2, 1);
        let discover = discover_message(XID, &CLIENT_MAC);
        let request = request_message(XID, &CLIENT_MAC, offered_ip, server_id);

        assert_eq!(discover.xid(), XID);
        assert_eq!(request.xid(), XID);
        assert_eq!(discover.chaddr(), CLIENT_MAC);
        assert_eq!(request.chaddr(), CLIENT_MAC);
        assert_eq!(
            request.opts().get(OptionCode::ClientIdentifier),
            Some(&DhcpOption::ClientIdentifier(CLIENT_MAC.to_vec()))
        );
        assert_eq!(
            request.opts().get(OptionCode::RequestedIpAddress),
            Some(&DhcpOption::RequestedIpAddress(offered_ip))
        );
        assert_eq!(
            request.opts().get(OptionCode::ServerIdentifier),
            Some(&DhcpOption::ServerIdentifier(server_id))
        );
    }

    #[test]
    fn netmask_prefix_accepts_only_contiguous_masks() {
        assert_eq!(
            netmask_prefix_len(Ipv4Addr::new(255, 255, 255, 255)),
            Some(32)
        );
        assert_eq!(
            netmask_prefix_len(Ipv4Addr::new(255, 255, 254, 0)),
            Some(23)
        );
        assert_eq!(netmask_prefix_len(Ipv4Addr::new(0, 0, 0, 0)), Some(0));
        assert_eq!(netmask_prefix_len(Ipv4Addr::new(255, 0, 255, 0)), None);
        assert_eq!(netmask_prefix_len(Ipv4Addr::new(255, 255, 255, 1)), None);
    }

    #[test]
    fn reply_selection_requires_matching_transaction_and_hardware_address() {
        let ack = reply(MessageType::Ack);
        assert_eq!(
            classify_reply(&ack, MessageType::Ack, XID, &CLIENT_MAC),
            ReplyDisposition::Accept
        );
        assert_eq!(
            classify_reply(&ack, MessageType::Ack, XID + 1, &CLIENT_MAC),
            ReplyDisposition::Ignore
        );
        assert_eq!(
            classify_reply(&ack, MessageType::Ack, XID, &[0x02, 0, 0, 0, 0, 0x02]),
            ReplyDisposition::Ignore
        );
        assert_eq!(
            classify_reply(&ack, MessageType::Offer, XID, &CLIENT_MAC),
            ReplyDisposition::Ignore
        );
    }

    #[test]
    fn reply_selection_distinguishes_matching_nak() {
        let nak = reply(MessageType::Nak);
        assert_eq!(
            classify_reply(&nak, MessageType::Ack, XID, &CLIENT_MAC),
            ReplyDisposition::Reject
        );
    }

    #[test]
    fn lease_validation_requires_explicit_usable_network_settings() {
        let offered_ip = Ipv4Addr::new(192, 0, 2, 10);
        let lease = validated_lease(offered_ip, &valid_ack()).expect("valid lease");
        assert_eq!(lease.ip, offered_ip);
        assert_eq!(lease.netmask, Ipv4Addr::new(255, 255, 255, 0));
        assert_eq!(lease.gateway, Ipv4Addr::new(192, 0, 2, 1));

        assert!(validated_lease(Ipv4Addr::UNSPECIFIED, &valid_ack()).is_err());

        let missing_mask = reply(MessageType::Ack);
        assert!(validated_lease(offered_ip, &missing_mask).is_err());

        let mut unspecified_mask = valid_ack();
        unspecified_mask
            .opts_mut()
            .insert(DhcpOption::SubnetMask(Ipv4Addr::UNSPECIFIED));
        assert!(validated_lease(offered_ip, &unspecified_mask).is_err());

        let mut non_contiguous_mask = valid_ack();
        non_contiguous_mask
            .opts_mut()
            .insert(DhcpOption::SubnetMask(Ipv4Addr::new(255, 0, 255, 0)));
        assert!(validated_lease(offered_ip, &non_contiguous_mask).is_err());

        let mut missing_router = valid_ack();
        missing_router.opts_mut().remove(OptionCode::Router);
        assert!(validated_lease(offered_ip, &missing_router).is_err());

        let mut empty_router = valid_ack();
        empty_router
            .opts_mut()
            .insert(DhcpOption::Router(Vec::new()));
        assert!(validated_lease(offered_ip, &empty_router).is_err());

        let mut unspecified_router = valid_ack();
        unspecified_router
            .opts_mut()
            .insert(DhcpOption::Router(vec![Ipv4Addr::UNSPECIFIED]));
        assert!(validated_lease(offered_ip, &unspecified_router).is_err());
    }

    #[test]
    fn offer_validation_rejects_unspecified_address() {
        let mut offer = reply(MessageType::Offer);
        assert!(offered_ip(&offer).is_err());

        let expected = Ipv4Addr::new(192, 0, 2, 10);
        offer.set_yiaddr(expected);
        assert_eq!(offered_ip(&offer).expect("offered address"), expected);
    }
}
