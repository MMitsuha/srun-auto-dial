use futures_util::stream::TryStreamExt;
use netlink_packet_route::link::LinkAttribute;
use pnet::ipnetwork::{IpNetwork, Ipv4Network};
use rtnetlink::{
    Error, Handle, LinkMacVlan, LinkUnspec, RouteMessageBuilder, packet_route::link::MacVlanMode,
};
use std::{
    fmt::{Display, Formatter},
    net::Ipv4Addr,
};

pub struct Link {
    pub index: u32,
    pub name: String,
}

impl Display for Link {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Index {}: {}", self.index, self.name)
    }
}

// pub async fn get_link_by_index(handle: Handle, index: u32) -> Result<String, Error> {
//     let mut links = handle.link().get().match_index(index).execute();
//     let msg = if let Some(msg) = links.try_next().await? {
//         msg
//     } else {
//         eprintln!("no link with index {index} found");
//         return Ok(String::new());
//     };
//     // We should have received only one message
//     assert!(links.try_next().await?.is_none());

//     for nla in msg.attributes.into_iter() {
//         if let LinkAttribute::IfName(name) = nla {
//             println!("found link with index {index} (name = {name})");
//             return Ok(name);
//         }
//     }
//     eprintln!("found link with index {index}, but this link does not have a name");
//     Ok(String::new())
// }

// pub async fn get_link_by_name(handle: Handle, name: String) -> Result<(), Error> {
//     let mut links = handle.link().get().match_name(name.clone()).execute();
//     if (links.try_next().await?).is_some() {
//         println!("found link {name}");
//         // We should only have one link with that name
//         assert!(links.try_next().await?.is_none());
//     } else {
//         println!("no link link {name} found");
//     }
//     Ok(())
// }

pub async fn dump_links(handle: Handle) -> Result<Vec<Link>, Error> {
    let mut links = handle.link().get().execute();
    let mut link_list = Vec::new();
    'outer: while let Some(msg) = links.try_next().await? {
        for nla in msg.attributes.into_iter() {
            if let LinkAttribute::IfName(name) = nla {
                // println!("found link {} ({})", msg.header.index, name);
                link_list.push(Link {
                    index: msg.header.index,
                    name,
                });
                continue 'outer;
            }
        }
        eprintln!("found link {}, but the link has no name", msg.header.index);
    }
    Ok(link_list)
}

// #[cfg(not(target_os = "freebsd"))]
// pub async fn dump_bridge_filter_info(handle: Handle) -> Result<(), Error> {
//     let mut links = handle
//         .link()
//         .get()
//         .set_filter_mask(AddressFamily::Bridge, vec![LinkExtentMask::Brvlan])
//         .execute();
//     'outer: while let Some(msg) = links.try_next().await? {
//         for nla in msg.attributes.into_iter() {
//             if let LinkAttribute::AfSpecBridge(data) = nla {
//                 println!(
//                     "found interface {} with AfSpecBridge data {:?})",
//                     msg.header.index, data
//                 );
//                 continue 'outer;
//             }
//         }
//     }
//     Ok(())
// }

pub async fn set_link_up(handle: Handle, link_name: String) -> Result<(), Error> {
    let mut links = handle.link().get().match_name(link_name.clone()).execute();
    if let Some(link) = links.try_next().await? {
        handle
            .link()
            .set(LinkUnspec::new_with_index(link.header.index).up().build())
            .execute()
            .await?;
    } else {
        println!("no link {link_name} found");
    }
    Ok(())
}

pub async fn create_macvlan(
    handle: Handle,
    link_name: String,
    macvlan_name: String,
    mac_address: Option<Vec<u8>>,
) -> Result<(), Error> {
    let mut parent_links = handle.link().get().match_name(link_name.clone()).execute();
    if let Some(parent) = parent_links.try_next().await? {
        let mut builder = LinkMacVlan::new(&macvlan_name, parent.header.index, MacVlanMode::Bridge);
        if let Some(mac) = mac_address {
            builder = builder.address(mac);
        }
        let message = builder.build();
        let request = handle.link().add(message);

        request.execute().await?;
    } else {
        println!("no link {link_name} found");
    }
    Ok(())
}

pub async fn del_macvlan(handle: Handle, macvlan_name: String) -> Result<(), Error> {
    let mut links = handle
        .link()
        .get()
        .match_name(macvlan_name.clone())
        .execute();
    if let Some(link) = links.try_next().await? {
        handle.link().del(link.header.index).execute().await?;
    } else {
        println!("no link {macvlan_name} found");
    }
    Ok(())
}

pub async fn add_address(handle: Handle, link_name: &str, ip: IpNetwork) -> Result<(), Error> {
    let mut links = handle
        .link()
        .get()
        .match_name(link_name.to_string())
        .execute();
    if let Some(link) = links.try_next().await? {
        handle
            .address()
            .add(link.header.index, ip.ip(), ip.prefix())
            .execute()
            .await?
    }
    Ok(())
}

pub async fn add_default_route(
    handle: Handle,
    link_name: &str,
    gateway: Ipv4Addr,
    source: Ipv4Addr,
) -> Result<(), Error> {
    let mut links = handle
        .link()
        .get()
        .match_name(link_name.to_string())
        .execute();
    let dest = Ipv4Network::new(Ipv4Addr::new(0, 0, 0, 0), 0).unwrap();
    if let Some(link) = links.try_next().await? {
        let route = RouteMessageBuilder::<Ipv4Addr>::new()
            .destination_prefix(dest.ip(), dest.prefix())
            .gateway(gateway)
            .output_interface(link.header.index)
            .pref_source(source)
            .build();
        handle.route().add(route).execute().await?;
    }
    Ok(())
}
