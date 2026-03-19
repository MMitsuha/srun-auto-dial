use crate::error::{Result, SrunError};
use futures_util::stream::TryStreamExt;
use netlink_packet_route::link::LinkAttribute;
use pnet::ipnetwork::{IpNetwork, Ipv4Network};
use rtnetlink::{Handle, LinkMacVlan, LinkUnspec, RouteMessageBuilder, packet_route::link::MacVlanMode};
use std::fmt::{Display, Formatter};
use std::net::Ipv4Addr;
use tracing::{debug, info};

pub struct Link {
    pub index: u32,
    pub name: String,
}

impl Display for Link {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Index {}: {}", self.index, self.name)
    }
}

pub async fn dump_links(handle: Handle) -> Result<Vec<Link>> {
    let mut links = handle.link().get().execute();
    let mut link_list = Vec::new();
    while let Some(msg) = links.try_next().await? {
        for nla in msg.attributes.iter() {
            if let LinkAttribute::IfName(name) = nla {
                link_list.push(Link {
                    index: msg.header.index,
                    name: name.clone(),
                });
                break;
            }
        }
    }
    Ok(link_list)
}

async fn get_link_index(handle: &Handle, name: &str) -> Result<u32> {
    let mut links = handle.link().get().match_name(name.to_string()).execute();
    let link = links
        .try_next()
        .await?
        .ok_or_else(|| SrunError::InterfaceNotFound(name.to_string()))?;
    Ok(link.header.index)
}

pub async fn set_link_up(handle: Handle, link_name: &str) -> Result<()> {
    let index = get_link_index(&handle, link_name).await?;
    handle
        .link()
        .set(LinkUnspec::new_with_index(index).up().build())
        .execute()
        .await?;
    debug!(interface = %link_name, "link set up");
    Ok(())
}

pub async fn create_macvlan(
    handle: Handle,
    parent_name: &str,
    macvlan_name: &str,
    mac_address: Option<&[u8]>,
) -> Result<()> {
    let parent_index = get_link_index(&handle, parent_name).await?;
    let mut builder = LinkMacVlan::new(macvlan_name, parent_index, MacVlanMode::Bridge);
    if let Some(mac) = mac_address {
        builder = builder.address(mac.to_vec());
    }
    let message = builder.build();
    handle.link().add(message).execute().await?;
    info!(parent = %parent_name, macvlan = %macvlan_name, "macvlan created");
    Ok(())
}

pub async fn del_macvlan(handle: Handle, macvlan_name: &str) -> Result<()> {
    let index = get_link_index(&handle, macvlan_name).await?;
    handle.link().del(index).execute().await?;
    info!(macvlan = %macvlan_name, "macvlan deleted");
    Ok(())
}

pub async fn add_address(handle: Handle, link_name: &str, ip: IpNetwork) -> Result<()> {
    let index = get_link_index(&handle, link_name).await?;
    handle
        .address()
        .add(index, ip.ip(), ip.prefix())
        .execute()
        .await?;
    debug!(interface = %link_name, ip = %ip, "address added");
    Ok(())
}

pub async fn add_default_route(
    handle: Handle,
    link_name: &str,
    gateway: Ipv4Addr,
    source: Ipv4Addr,
) -> Result<()> {
    let index = get_link_index(&handle, link_name).await?;
    let dest = Ipv4Network::new(Ipv4Addr::new(0, 0, 0, 0), 0).expect("0.0.0.0/0 is always valid");
    let route = RouteMessageBuilder::<Ipv4Addr>::new()
        .destination_prefix(dest.ip(), dest.prefix())
        .gateway(gateway)
        .output_interface(index)
        .pref_source(source)
        .build();
    handle.route().add(route).execute().await?;
    debug!(interface = %link_name, gateway = %gateway, "default route added");
    Ok(())
}
