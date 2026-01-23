mod dhcpc;
mod netlink;
mod srun;

use dhcpc::dhcp_client;
use inquire::PasswordDisplayMode;
use inquire::{Password, Select, Text};
use inquire_derive::Selectable;
use netlink::*;
use pnet::ipnetwork::{IpNetwork, Ipv4Network};
use rand::seq::IndexedRandom;
use rand::{Rng, rng};
use reqwest::Client;
use rtnetlink::{Handle, new_connection};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{Display, Formatter},
    net::Ipv4Addr,
};
use tokio::fs::read_to_string;

const MACVLAN_NAME: &str = "srun";

#[derive(Debug, Serialize, Deserialize)]
struct User {
    username: String,
    password: String,
}

// struct LoginStatus {
//     success: Vec<Vec<u8>>,
//     failure: Vec<(Vec<u8>, String)>,
// }

#[tokio::main]
async fn main() -> Result<(), String> {
    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);

    println!("SRUN Auto Dialer");
    let mode = DialMacMode::select("Select your dial mode:")
        .prompt()
        .map_err(|e| format!("{e}"))?;
    match mode {
        DialMacMode::Local => local_mac_dial(handle.clone()).await?,
        DialMacMode::Custom => custom_mac_dial(handle.clone()).await?,
        DialMacMode::Random => random_mac_dial(handle.clone()).await?,
    }

    Ok(())
}

async fn local_mac_dial(handle: Handle) -> Result<(), String> {
    let link = select_link(handle.clone(), "Select a link:").await?;
    let default_headers = srun::utils::build_default_header();
    let client = reqwest::Client::builder()
        .default_headers(default_headers)
        .interface(&link.name)
        .build()
        .map_err(|e| format!("{e}"))?;

    let callback = srun::utils::generate_jsonp_callback();
    // println!("Generated JSONP callback: {}", callback);

    match select_operation()?.as_str() {
        "Login" => {
            let users = get_user(false).await?;
            let user = users.choose(&mut rng()).ok_or("No user found")?;
            login(
                client.clone(),
                &callback,
                &user.username,
                &user.password,
                &link.name,
                None,
            )
            .await?;
            println!("Login successful");
        }
        "Logout" => {
            logout(client.clone(), &callback).await?;
            println!("Logout successful");
        }
        _ => return Err("Invalid operation selected".to_string()),
    }

    Ok(())
}

async fn custom_mac_dial(handle: Handle) -> Result<(), String> {
    let link = select_link(handle.clone(), "Select the parent link:").await?;
    let mac_address_input = Text::new("Enter the custom MAC address (e.g., AA:BB:CC:DD:EE:FF):")
        .prompt()
        .map_err(|e| format!("{e}"))?;
    let mac_address: Option<Vec<u8>> = Some(
        mac_address_input
            .split(':')
            .map(|s| {
                u8::from_str_radix(s, 16)
                    .map_err(|e| format!("Invalid MAC address segment {}: {}", s, e))
            })
            .collect::<Result<_, _>>()?,
    );

    let dhcpinfo_ip = add_macvlan(handle.clone(), &link.name, MACVLAN_NAME, mac_address).await?;
    let result = async {
        let default_headers = srun::utils::build_default_header();
        let client = reqwest::Client::builder()
            .default_headers(default_headers)
            .interface(MACVLAN_NAME)
            .build()
            .map_err(|e| format!("{e}"))?;
        let callback = srun::utils::generate_jsonp_callback();
        // println!("Generated JSONP callback: {}", callback);

        match select_operation()?.as_str() {
            "Login" => {
                let users = get_user(false).await?;
                let user = users.choose(&mut rng()).ok_or("No user found")?;
                login(
                    client.clone(),
                    &callback,
                    &user.username,
                    &user.password,
                    MACVLAN_NAME,
                    Some(dhcpinfo_ip),
                )
                .await?;
                println!("Login successful, User: {}", user.username);
            }
            "Logout" => {
                logout(client.clone(), &callback).await?;
                println!("Logout successful");
            }
            _ => return Err("Invalid operation selected".to_string()),
        }
        Ok(())
    }
    .await;

    del_macvlan(handle, MACVLAN_NAME.to_string())
        .await
        .map_err(|e| format!("{e}"))?;
    match result {
        Ok(_) => Ok(()),
        Err(e) => return Err(e),
    }
}

async fn random_mac_dial(handle: Handle) -> Result<(), String> {
    let link = select_link(handle.clone(), "Select the parent link:").await?;

    let times = Text::new("Enter the number of random MAC addresses to try:")
        .with_default("1")
        .prompt()
        .map_err(|e| format!("{e}"))?
        .parse::<u32>()
        .map_err(|e| format!("Invalid number: {}", e))?;

    // let _status = LoginStatus {
    //     success: Vec::new(),
    //     failure: Vec::new(),
    // };
    let users = get_user(true).await?;

    for _ in 1..=times {
        let mac_address: Option<Vec<u8>> = Some(generate_mac_address());

        let result: Result<String, String> = async {
            let dhcpinfo_ip = add_macvlan(
                handle.clone(),
                &link.name,
                MACVLAN_NAME,
                mac_address.clone(),
            )
            .await?;

            let default_headers = srun::utils::build_default_header();
            let client = reqwest::Client::builder()
                .default_headers(default_headers)
                .interface(MACVLAN_NAME)
                .build()
                .map_err(|e| format!("{e}"))?;
            let callback = srun::utils::generate_jsonp_callback();
            // println!("Generated JSONP callback: {}", callback);
            let user = users.choose(&mut rng()).ok_or("No user found")?;

            login(
                client.clone(),
                &callback,
                &user.username,
                &user.password,
                MACVLAN_NAME,
                Some(dhcpinfo_ip),
            )
            .await?;
            Ok(user.username.clone())
        }
        .await;

        match result {
            Ok(username) => {
                // println!(
                //     "Login successful with MAC address {:02X?}",
                //     mac_address.as_ref().unwrap()
                // );
                // status.success.push(mac_address.unwrap());
                println!(
                    "MAC Address: {} login successful, User: {}",
                    format_mac(&mac_address.unwrap()),
                    username
                );
            }
            Err(e) => {
                // println!(
                //     "Login failed with MAC address {:02X?}: {}",
                //     mac_address.as_ref().unwrap(),
                //     e
                // );
                // status.failure.push((mac_address.unwrap(), e));
                println!(
                    "MAC Address: {} login failed, Reason: {}",
                    format_mac(&mac_address.unwrap()),
                    e
                );
            }
        }

        // logout(client.clone(), &callback).await?;

        del_macvlan(handle.clone(), MACVLAN_NAME.to_string())
            .await
            .map_err(|e| format!("{e}"))?;
    }

    // println!("Login Summary:");
    // println!("Successful logins:");
    // for mac in status.success {
    //     println!("  MAC Address: {}", format_mac(&mac));
    // }
    // println!("Failed logins:");
    // for (mac, reason) in status.failure {
    //     println!("  MAC Address: {}, Reason: {}", format_mac(&mac), reason);
    // }

    Ok(())
}

fn format_mac(mac: &[u8]) -> String {
    mac.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(":")
}

async fn get_user(random: bool) -> Result<Vec<User>, String> {
    match random {
        false => {
            let mode = UserMode::select("Select how to input user information:")
                .prompt()
                .map_err(|e| format!("{e}"))?;
            match mode {
                UserMode::Input => {
                    let username = Text::new("Enter username:")
                        .prompt()
                        .map_err(|e| format!("{e}"))?;
                    let password = Password::new("Enter password:")
                        .with_display_mode(PasswordDisplayMode::Masked)
                        .without_confirmation()
                        .prompt()
                        .map_err(|e| format!("{e}"))?;
                    Ok(vec![User { username, password }])
                }
                UserMode::Read => {
                    let contents = read_to_string("userinfo.json")
                        .await
                        .map_err(|e| format!("Failed to open file: {}", e))?;
                    let users: Vec<User> = serde_json::from_str(&contents)
                        .map_err(|e| format!("Failed to parse JSON: {}", e))?;
                    Ok(users)
                }
            }
        }
        true => {
            let contents = read_to_string("userinfo.json")
                .await
                .map_err(|e| format!("Failed to open file: {}", e))?;
            let users: Vec<User> = serde_json::from_str(&contents)
                .map_err(|e| format!("Failed to parse JSON: {}", e))?;
            Ok(users)
        }
    }
    // let mode = UserMode::select("Select how to input user information:")
    //     .prompt()
    //     .map_err(|e| format!("{e}"))?;
    // match mode {
    //     UserMode::Input => {
    //         let username = Text::new("Enter username:")
    //             .prompt()
    //             .map_err(|e| format!("{e}"))?;
    //         let password = Password::new("Enter password:")
    //             .with_display_mode(PasswordDisplayMode::Masked)
    //             .without_confirmation()
    //             .prompt()
    //             .map_err(|e| format!("{e}"))?;
    //         Ok(vec![User { username, password }])
    //     }
    //     UserMode::Read => {
    //         let contents = read_to_string("userinfo.json")
    //             .await
    //             .map_err(|e| format!("Failed to open file: {}", e))?;
    //         let users: Vec<User> = serde_json::from_str(&contents)
    //             .map_err(|e| format!("Failed to parse JSON: {}", e))?;
    //         Ok(users)
    //     }
    // }
}

async fn login(
    client: Client,
    callback: &str,
    username: &str,
    password: &str,
    link_name: &str,
    dhcp_ip: Option<Ipv4Addr>,
) -> Result<(), String> {
    let userinfo = srun::get_userinfo(client.clone(), callback).await?;
    // println!("Current IP from userinfo: {}", userinfo.ip);

    if dhcp_ip.is_some() && dhcp_ip.unwrap() != userinfo.ip {
        return Err(format!(
            "IP mismatch: DHCP assigned IP is {}, but userinfo reports {}",
            dhcp_ip.unwrap(),
            userinfo.ip
        ));
    }

    if userinfo.online_user.is_some() && userinfo.online_mac.is_some() {
        // println!(
        //     "MAC address {} is already online, online user: {}",
        //     userinfo.online_mac.unwrap(),
        //     userinfo.online_user.unwrap()
        // );
        return Err(format!(
            "MAC address {} is already online, online user: {}",
            userinfo.online_mac.unwrap(),
            userinfo.online_user.unwrap()
        ));
    }
    let challenge = srun::get_challenge(client.clone(), &callback, username, userinfo.ip).await?;
    // println!("Received challenge: {}", challenge);

    srun::login(
        client.clone(),
        &callback,
        username,
        password,
        userinfo.ip,
        &challenge,
    )
    .await?;
    // println!("Login successful");
    // sleep(Duration::from_secs(2));
    // let client_test = reqwest::Client::builder()
    //     .interface(link_name)
    //     .build()
    //     .map_err(|e| format!("{e}"))?;
    // let _resp = client_test
    //     .get("http://4.ipw.cn/")
    //     .send()
    //     .await
    //     .map_err(|e| format!("{e}"))?
    //     .text()
    //     .await
    //     .map_err(|e| format!("{e}"))?;
    // println!("Access test response: {}", resp);

    Ok(())
}

async fn logout(client: Client, callback: &str) -> Result<(), String> {
    let userinfo = srun::get_userinfo(client.clone(), callback).await?;
    // println!("Current IP from userinfo: {}", userinfo.ip);
    if userinfo.online_user.is_none() {
        // println!("No user is currently online.");
        return Err("No user is currently online.".to_string());
    }

    srun::logout(
        client,
        callback,
        userinfo.online_user.as_deref().unwrap_or(""),
        userinfo.ip,
    )
    .await?;
    // println!("Logout successful");

    Ok(())
}

async fn select_link(handle: Handle, msg: &str) -> Result<Link, String> {
    let links = dump_links(handle.clone())
        .await
        .map_err(|e| format!("{e}"))?;
    let link = Select::new(msg, links)
        .prompt()
        .map_err(|e| format!("{e}"))?;
    Ok(link)
}

fn select_operation() -> Result<String, String> {
    let operations = vec!["Login", "Logout"];
    let operation = Select::new("Select operation:", operations)
        .prompt()
        .map_err(|e| format!("{e}"))?
        .to_string();
    Ok(operation)
}

async fn add_macvlan(
    handle: Handle,
    link_name: &str,
    macvlan_name: &str,
    mac_address: Option<Vec<u8>>,
) -> Result<Ipv4Addr, String> {
    create_macvlan(
        handle.clone(),
        link_name.to_string(),
        macvlan_name.to_string(),
        mac_address,
    )
    .await
    .map_err(|e| format!("{e}"))?;
    set_link_up(handle.clone(), macvlan_name.to_string())
        .await
        .map_err(|e| format!("{e}"))?;
    // println!("macvlan created");

    let dhcp_info = dhcp_client(macvlan_name)
        .await
        .map_err(|e| format!("{e}"))?;

    let prefix = dhcp_info
        .netmask
        .octets()
        .iter()
        .fold(0, |acc, &b| acc + b.count_ones()) as u8;
    let ip = Ipv4Network::new(dhcp_info.ip, prefix).map_err(|e| format!("{e}"))?;
    let ip = IpNetwork::V4(ip);

    add_address(handle.clone(), macvlan_name, ip)
        .await
        .map_err(|e| format!("{e}"))?;
    // println!("IP address assigned: {}", dhcp_info.ip);
    add_default_route(
        handle.clone(),
        macvlan_name,
        dhcp_info.gateway,
        dhcp_info.ip,
    )
    .await
    .map_err(|e| format!("{e}"))?;
    // println!("Default route added via {}", dhcp_info.gateway);

    Ok(dhcp_info.ip)
}

fn generate_mac_address() -> Vec<u8> {
    let mut rng = rng();
    let mut mac = [0u8; 6];
    rng.fill(&mut mac[0..6]);
    // Set the locally administered bit and clear the multicast bit
    mac[0] = (mac[0] & 0b11111110) | 0b00000010;
    mac.to_vec()
}

#[derive(Debug, Copy, Clone, Selectable)]
enum DialMacMode {
    Local,
    Custom,
    Random,
}

impl Display for DialMacMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            DialMacMode::Local => write!(f, "Using the local network adapter's MAC address"),
            DialMacMode::Custom => write!(f, "Using the custom MAC address"),
            DialMacMode::Random => write!(
                f,
                "Using random MAC addresses (Only support login and read user from file)"
            ),
        }
    }
}

#[derive(Debug, Copy, Clone, Selectable)]
enum UserMode {
    Input,
    Read,
}

impl Display for UserMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            UserMode::Input => write!(f, "Enter username and password manually"),
            UserMode::Read => write!(f, "Read username and password from file"),
        }
    }
}
