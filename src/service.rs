use crate::config::Config;
use crate::error::{Result, SrunError};
use crate::net::{self, DhcpInfo, Link};
use crate::srun::{SrunClient, UserInfo, utils as srun_utils};
use pnet::ipnetwork::{IpNetwork, Ipv4Network};
use std::collections::HashMap;
use rand::{Rng, rng};
use reqwest::Client;
use rtnetlink::Handle;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::sync::Arc;
use tracing::{debug, info};

const MACVLAN_NAME: &str = "srun";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoginResult {
    pub ip: Ipv4Addr,
    pub username: String,
    pub mac: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusResult {
    pub ip: Ipv4Addr,
    pub online_user: Option<String>,
    pub online_mac: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RandomLoginResult {
    pub mac: String,
    pub result: std::result::Result<LoginResult, String>,
}

pub struct SrunService {
    config: Arc<Config>,
    handle: Handle,
    srun_client: SrunClient,
}

impl SrunService {
    pub fn new(config: Arc<Config>, handle: Handle) -> Self {
        let srun_client = SrunClient::new(&config);
        Self {
            config,
            handle,
            srun_client,
        }
    }

    fn build_client(&self, interface: &str) -> Result<Client> {
        let headers = srun_utils::build_default_headers(&self.config);
        Client::builder()
            .default_headers(headers)
            .interface(interface)
            .build()
            .map_err(SrunError::Request)
    }

    /// Get online status via a network interface.
    pub async fn get_status(&self, interface: &str) -> Result<StatusResult> {
        let client = self.build_client(interface)?;
        let callback = srun_utils::generate_jsonp_callback();
        let info = self.srun_client.get_userinfo(&client, &callback).await?;
        Ok(StatusResult {
            ip: info.ip,
            online_user: info.online_user,
            online_mac: info.online_mac,
        })
    }

    /// List available network interfaces.
    pub async fn list_interfaces(&self) -> Result<Vec<Link>> {
        net::dump_links(self.handle.clone()).await
    }

    // ---- Local mode ----

    pub async fn login_local(
        &self,
        interface: &str,
        credentials: Option<(&str, &str)>,
    ) -> Result<LoginResult> {
        let (username, password) = match credentials {
            Some((u, p)) => (u.to_string(), p.to_string()),
            None => {
                let user = self.random_user().await?;
                (user.username, user.password)
            }
        };

        let client = self.build_client(interface)?;
        let callback = srun_utils::generate_jsonp_callback();

        let userinfo = self.srun_client.get_userinfo(&client, &callback).await?;
        check_not_online(&userinfo)?;

        let challenge = self
            .srun_client
            .get_challenge(&client, &callback, &username, userinfo.ip)
            .await?;

        self.srun_client
            .login(&client, &callback, &username, &password, userinfo.ip, &challenge)
            .await?;

        info!(username = %username, ip = %userinfo.ip, "login successful (local)");
        Ok(LoginResult {
            ip: userinfo.ip,
            username,
            mac: None,
        })
    }

    pub async fn logout_local(&self, interface: &str) -> Result<()> {
        let client = self.build_client(interface)?;
        let callback = srun_utils::generate_jsonp_callback();

        let userinfo = self.srun_client.get_userinfo(&client, &callback).await?;
        let username = userinfo
            .online_user
            .as_deref()
            .ok_or(SrunError::NoUserOnline)?;

        self.srun_client
            .logout(&client, &callback, username, userinfo.ip)
            .await?;

        info!(username = %username, "logout successful (local)");
        Ok(())
    }

    // ---- Macvlan mode ----

    pub async fn login_macvlan(
        &self,
        parent: &str,
        mac: &[u8],
        credentials: Option<(&str, &str)>,
    ) -> Result<LoginResult> {
        let (username, password) = match credentials {
            Some((u, p)) => (u.to_string(), p.to_string()),
            None => {
                let user = self.random_user().await?;
                (user.username, user.password)
            }
        };
        let mac_str = format_mac(mac);

        // Setup macvlan
        self.setup_macvlan(parent, mac).await?;

        // Run login, ensuring cleanup
        let result = self
            .do_macvlan_login(&username, &password, &mac_str)
            .await;

        // Always cleanup
        self.cleanup_macvlan().await;

        match &result {
            Ok(r) => info!(username = %r.username, ip = %r.ip, mac = %mac_str, "login successful (macvlan)"),
            Err(e) => debug!(mac = %mac_str, error = %e, "login failed (macvlan)"),
        }
        result
    }

    pub async fn logout_macvlan(&self, parent: &str) -> Result<()> {
        let random_mac = generate_mac_address();
        self.setup_macvlan(parent, &random_mac).await?;

        let result = self.do_macvlan_logout().await;

        self.cleanup_macvlan().await;
        result
    }

    /// Batch login with random MACs, reading users from userinfo.json.
    /// Each account is used at most 3 times to avoid kicking off previous sessions.
    pub async fn login_random(
        &self,
        parent: &str,
        count: u32,
    ) -> Result<Vec<RandomLoginResult>> {
        const MAX_LOGIN_PER_USER: u32 = 3;

        let users = self.load_users().await?;
        let mut usage: HashMap<String, u32> = HashMap::new();
        let mut results = Vec::with_capacity(count as usize);

        for _ in 0..count {
            let available: Vec<_> = users
                .iter()
                .filter(|u| *usage.get(&u.username).unwrap_or(&0) < MAX_LOGIN_PER_USER)
                .collect();

            if available.is_empty() {
                tracing::warn!("all users have reached max login count ({MAX_LOGIN_PER_USER}), stopping");
                break;
            }

            let user = available[rng().random_range(0..available.len())];
            *usage.entry(user.username.clone()).or_insert(0) += 1;

            let mac = generate_mac_address();
            let mac_str = format_mac(&mac);
            let creds = Some((user.username.as_str(), user.password.as_str()));

            let result = self.login_macvlan(parent, &mac, creds).await;

            results.push(RandomLoginResult {
                mac: mac_str,
                result: result.map_err(|e| e.to_string()),
            });
        }

        Ok(results)
    }

    /// Load users from the configured userinfo file.
    pub async fn load_users(&self) -> Result<Vec<User>> {
        let path = self
            .config
            .userinfo_path
            .as_deref()
            .unwrap_or("userinfo.json");
        let contents = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| SrunError::Config(format!("failed to read {}: {}", path, e)))?;
        let users: Vec<User> = serde_json::from_str(&contents)?;
        if users.is_empty() {
            return Err(SrunError::Config("userinfo.json is empty".to_string()));
        }
        Ok(users)
    }

    /// Pick a random user from userinfo.json.
    async fn random_user(&self) -> Result<User> {
        let users = self.load_users().await?;
        Ok(users[rng().random_range(0..users.len())].clone())
    }

    // ---- Internal macvlan helpers ----

    async fn setup_macvlan(&self, parent: &str, mac: &[u8]) -> Result<()> {
        net::create_macvlan(self.handle.clone(), parent, MACVLAN_NAME, Some(mac)).await?;
        net::set_link_up(self.handle.clone(), MACVLAN_NAME).await?;

        let dhcp_info: DhcpInfo = net::dhcp_client(MACVLAN_NAME).await?;

        let prefix = dhcp_info
            .netmask
            .octets()
            .iter()
            .fold(0u8, |acc, &b| acc + b.count_ones() as u8);
        let ip_net = Ipv4Network::new(dhcp_info.ip, prefix)
            .map_err(|e| SrunError::Dhcp(format!("invalid IP/prefix: {}", e)))?;

        net::add_address(self.handle.clone(), MACVLAN_NAME, IpNetwork::V4(ip_net)).await?;
        net::add_default_route(
            self.handle.clone(),
            MACVLAN_NAME,
            dhcp_info.gateway,
            dhcp_info.ip,
        )
        .await?;

        Ok(())
    }

    async fn cleanup_macvlan(&self) {
        if let Err(e) = net::del_macvlan(self.handle.clone(), MACVLAN_NAME).await {
            debug!(error = %e, "failed to delete macvlan during cleanup");
        }
    }

    async fn do_macvlan_login(
        &self,
        username: &str,
        password: &str,
        mac_str: &str,
    ) -> Result<LoginResult> {
        let client = self.build_client(MACVLAN_NAME)?;
        let callback = srun_utils::generate_jsonp_callback();

        let userinfo = self.srun_client.get_userinfo(&client, &callback).await?;
        check_not_online(&userinfo)?;

        let challenge = self
            .srun_client
            .get_challenge(&client, &callback, username, userinfo.ip)
            .await?;

        self.srun_client
            .login(&client, &callback, username, password, userinfo.ip, &challenge)
            .await?;

        Ok(LoginResult {
            ip: userinfo.ip,
            username: username.to_string(),
            mac: Some(mac_str.to_string()),
        })
    }

    async fn do_macvlan_logout(&self) -> Result<()> {
        let client = self.build_client(MACVLAN_NAME)?;
        let callback = srun_utils::generate_jsonp_callback();

        let userinfo = self.srun_client.get_userinfo(&client, &callback).await?;
        let username = userinfo
            .online_user
            .as_deref()
            .ok_or(SrunError::NoUserOnline)?;

        self.srun_client
            .logout(&client, &callback, username, userinfo.ip)
            .await?;

        info!(username = %username, "logout successful (macvlan)");
        Ok(())
    }
}

fn check_not_online(userinfo: &UserInfo) -> Result<()> {
    if let (Some(user), Some(mac)) = (&userinfo.online_user, &userinfo.online_mac)
        && !user.is_empty()
        && !mac.is_empty()
    {
        return Err(SrunError::AlreadyOnline {
            user: user.clone(),
            mac: mac.clone(),
        });
    }
    Ok(())
}

pub fn format_mac(mac: &[u8]) -> String {
    mac.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(":")
}

pub fn parse_mac(s: &str) -> Result<Vec<u8>> {
    s.split(':')
        .map(|seg| {
            u8::from_str_radix(seg, 16)
                .map_err(|_| SrunError::InvalidMac(format!("invalid segment: {}", seg)))
        })
        .collect::<Result<Vec<u8>>>()
        .and_then(|v| {
            if v.len() == 6 {
                Ok(v)
            } else {
                Err(SrunError::InvalidMac(format!(
                    "expected 6 octets, got {}",
                    v.len()
                )))
            }
        })
}

pub fn generate_mac_address() -> Vec<u8> {
    let mut r = rng();
    let mut mac = [0u8; 6];
    r.fill(&mut mac);
    // Set locally administered bit, clear multicast bit
    mac[0] = (mac[0] & 0b11111110) | 0b00000010;
    mac.to_vec()
}
