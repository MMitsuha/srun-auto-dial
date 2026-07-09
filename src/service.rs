use crate::config::Config;
use crate::error::{Result, SrunError};
use crate::net::{self, DhcpInfo, Link};
use crate::srun::{SrunClient, UserInfo, utils as srun_utils};
use pnet::ipnetwork::{IpNetwork, Ipv4Network};
use rand::{Rng, rng};
use reqwest::Client;
use rtnetlink::Handle;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::ErrorKind;
use std::net::Ipv4Addr;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr as UnixSocketAddr, UnixDatagram};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

const MACVLAN_PREFIX: &str = "srn";
const MACVLAN_NAME_LEN: usize = 15;
const NETWORK_LOCK_NAME: &[u8] = b"srun-auto-dial-network-operation-v1";
const MAX_BATCH_COUNT: u32 = 100;
const MAX_LOGIN_PER_USER: u32 = 3;
const MAX_USERINFO_BYTES: u64 = 1024 * 1024;
const MAX_USERS: usize = 10_000;

#[derive(Clone, Deserialize)]
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
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<LoginResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AttemptError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttemptError {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RandomLoginBatchResult {
    pub requested: u32,
    pub attempted: u32,
    pub succeeded: u32,
    pub failed: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopped_reason: Option<String>,
    pub results: Vec<RandomLoginResult>,
}

/// Owns exactly one temporary interface. If the request future is cancelled,
/// `Drop` schedules a best-effort cleanup for the interface this session made.
struct MacvlanSession {
    handle: Handle,
    name: String,
    lease: Option<DhcpInfo>,
    active: bool,
}

/// An abstract Unix socket is scoped to the Linux network namespace. Holding
/// this socket serializes route/authentication mutations across processes and
/// across containers that share `--network host`, even with separate PID and
/// mount namespaces. The kernel releases it automatically on process exit.
struct NetworkNamespaceLock {
    _socket: UnixDatagram,
}

impl NetworkNamespaceLock {
    fn try_acquire() -> Result<Self> {
        let address =
            UnixSocketAddr::from_abstract_name(NETWORK_LOCK_NAME).map_err(SrunError::Io)?;
        match UnixDatagram::bind_addr(&address) {
            Ok(socket) => Ok(Self { _socket: socket }),
            Err(error) if error.kind() == ErrorKind::AddrInUse => Err(SrunError::OperationBusy),
            Err(error) => Err(SrunError::Io(error)),
        }
    }
}

impl MacvlanSession {
    fn new(handle: Handle, name: String) -> Self {
        Self {
            handle,
            name,
            lease: None,
            active: true,
        }
    }

    async fn cleanup(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        for attempt in 1..=3 {
            match net::del_macvlan(self.handle.clone(), &self.name).await {
                Ok(()) | Err(SrunError::InterfaceNotFound(_)) => {
                    self.active = false;
                    return Ok(());
                }
                Err(error) if attempt < 3 => {
                    warn!(
                        interface = %self.name,
                        attempt,
                        error = %error,
                        "macvlan cleanup attempt failed; retrying"
                    );
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

impl Drop for MacvlanSession {
    fn drop(&mut self) {
        if !self.active {
            return;
        }

        let handle = self.handle.clone();
        let name = self.name.clone();
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => {
                runtime.spawn(async move {
                    // A cancelled netlink create future may complete in the kernel
                    // just after its Rust future is dropped. Briefly retry a missing
                    // link so that race cannot leave the temporary interface behind.
                    for attempt in 0..5 {
                        match net::del_macvlan(handle.clone(), &name).await {
                            Ok(()) => return,
                            Err(SrunError::InterfaceNotFound(_)) if attempt < 4 => {
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                            Err(SrunError::InterfaceNotFound(_)) => return,
                            Err(cleanup_error) => {
                                error!(
                                    interface = %name,
                                    error = %cleanup_error,
                                    "deferred macvlan cleanup failed"
                                );
                                return;
                            }
                        }
                    }
                });
            }
            Err(cleanup_error) => {
                error!(
                    interface = %name,
                    error = %cleanup_error,
                    "could not schedule deferred macvlan cleanup"
                );
            }
        }
    }
}

pub struct SrunService {
    config: Arc<Config>,
    handle: Handle,
    srun_client: SrunClient,
    macvlan_lock: Mutex<()>,
    local_mutation_lock: Mutex<()>,
}

impl SrunService {
    pub fn new(config: Arc<Config>, handle: Handle) -> Self {
        let srun_client = SrunClient::new(&config);
        Self {
            config,
            handle,
            srun_client,
            macvlan_lock: Mutex::new(()),
            local_mutation_lock: Mutex::new(()),
        }
    }

    fn build_client(&self, interface: &str) -> Result<Client> {
        let headers = srun_utils::build_default_headers(&self.config);
        Client::builder()
            .default_headers(headers)
            .interface(interface)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| SrunError::Request(error.without_url()))
    }

    /// Get online status via a network interface.
    pub async fn get_status(&self, interface: &str) -> Result<StatusResult> {
        validate_interface(interface, "interface")?;
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
        let mut links = net::dump_links(self.handle.clone()).await?;
        links.retain(|link| !is_managed_macvlan_name(&link.name));
        links.sort_by_key(|link| (link.name == "lo", link.index));
        Ok(links)
    }

    // ---- Local mode ----

    pub async fn login_local(
        &self,
        interface: &str,
        credentials: Option<(&str, &str)>,
        userinfo_path: Option<&str>,
    ) -> Result<LoginResult> {
        validate_interface(interface, "interface")?;
        let (username, password) = match credentials {
            Some((u, p)) => {
                validate_credentials(u, p)?;
                (u.to_string(), p.to_string())
            }
            None => {
                let user = self.random_user(userinfo_path).await?;
                (user.username, user.password)
            }
        };
        let _guard = self
            .local_mutation_lock
            .try_lock()
            .map_err(|_| SrunError::OperationBusy)?;
        let _namespace_guard = NetworkNamespaceLock::try_acquire()?;

        let client = self.build_client(interface)?;
        let callback = srun_utils::generate_jsonp_callback();

        let userinfo = self.srun_client.get_userinfo(&client, &callback).await?;
        check_not_online(&userinfo)?;

        let challenge = self
            .srun_client
            .get_challenge(&client, &callback, &username, userinfo.ip)
            .await?;

        self.srun_client
            .login(
                &client,
                &callback,
                &username,
                &password,
                userinfo.ip,
                &challenge,
            )
            .await?;

        info!(username = %username, ip = %userinfo.ip, "login successful (local)");
        Ok(LoginResult {
            ip: userinfo.ip,
            username,
            mac: None,
        })
    }

    pub async fn logout_local(&self, interface: &str) -> Result<()> {
        validate_interface(interface, "interface")?;
        let _guard = self
            .local_mutation_lock
            .try_lock()
            .map_err(|_| SrunError::OperationBusy)?;
        let _namespace_guard = NetworkNamespaceLock::try_acquire()?;
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
        userinfo_path: Option<&str>,
    ) -> Result<LoginResult> {
        validate_interface(parent, "parent_interface")?;
        validate_mac_bytes(mac)?;
        let (username, password) = match credentials {
            Some((u, p)) => {
                validate_credentials(u, p)?;
                (u.to_string(), p.to_string())
            }
            None => {
                let user = self.random_user(userinfo_path).await?;
                (user.username, user.password)
            }
        };
        let _guard = self
            .macvlan_lock
            .try_lock()
            .map_err(|_| SrunError::OperationBusy)?;
        let _namespace_guard = NetworkNamespaceLock::try_acquire()?;
        self.login_macvlan_locked(parent, mac, &username, &password)
            .await
    }

    pub async fn get_status_macvlan(&self, parent: &str, mac: &[u8]) -> Result<StatusResult> {
        validate_interface(parent, "parent_interface")?;
        validate_mac_bytes(mac)?;
        let _guard = self
            .macvlan_lock
            .try_lock()
            .map_err(|_| SrunError::OperationBusy)?;
        let _namespace_guard = NetworkNamespaceLock::try_acquire()?;
        let mut session = self.setup_macvlan(parent, mac).await?;
        let result = self
            .do_macvlan_status(&session.name, session_ip(&session)?)
            .await;
        finish_session(&mut session, result, "status check").await
    }

    pub async fn logout_macvlan(&self, parent: &str, mac: &[u8]) -> Result<()> {
        validate_interface(parent, "parent_interface")?;
        validate_mac_bytes(mac)?;
        let _guard = self
            .macvlan_lock
            .try_lock()
            .map_err(|_| SrunError::OperationBusy)?;
        let _namespace_guard = NetworkNamespaceLock::try_acquire()?;
        let mut session = self.setup_macvlan(parent, mac).await?;
        let result = self
            .do_macvlan_logout(&session.name, session_ip(&session)?)
            .await;
        finish_session(&mut session, result, "logout").await
    }

    /// Batch login with random MACs, reading users from the chosen userinfo file.
    /// Each account is used at most 3 times to avoid kicking off previous sessions.
    pub async fn login_random(
        &self,
        parent: &str,
        count: u32,
        userinfo_path: Option<&str>,
    ) -> Result<RandomLoginBatchResult> {
        validate_interface(parent, "parent_interface")?;
        if !(1..=MAX_BATCH_COUNT).contains(&count) {
            return Err(SrunError::Validation {
                field: "count",
                message: format!("count must be between 1 and {MAX_BATCH_COUNT}."),
            });
        }
        let users = self.load_users(userinfo_path).await?;
        let _guard = self
            .macvlan_lock
            .try_lock()
            .map_err(|_| SrunError::OperationBusy)?;
        let _namespace_guard = NetworkNamespaceLock::try_acquire()?;
        let mut usage: HashMap<String, u32> = HashMap::new();
        let mut results = Vec::with_capacity(count as usize);
        let mut stopped_reason = None;

        for _ in 0..count {
            let available: Vec<_> = users
                .iter()
                .filter(|u| *usage.get(&u.username).unwrap_or(&0) < MAX_LOGIN_PER_USER)
                .collect();

            if available.is_empty() {
                warn!("all users have reached max login count ({MAX_LOGIN_PER_USER}), stopping");
                stopped_reason = Some(format!(
                    "Stopped because every account reached the per-batch limit of {MAX_LOGIN_PER_USER} attempts."
                ));
                break;
            }

            let user = available[rng().random_range(0..available.len())];
            *usage.entry(user.username.clone()).or_insert(0) += 1;

            let mac = generate_mac_address();
            let mac_str = format_mac(&mac);
            let result = self
                .login_macvlan_locked(parent, &mac, &user.username, &user.password)
                .await;
            let cleanup_failed = matches!(
                &result,
                Err(SrunError::CleanupFailed { .. } | SrunError::OperationAndCleanupFailed { .. })
            );
            results.push(RandomLoginResult::from_result(mac_str, result));
            if cleanup_failed {
                stopped_reason = Some(
                    "Stopped because a temporary interface could not be cleaned up safely."
                        .to_string(),
                );
                break;
            }
        }

        let succeeded = results.iter().filter(|result| result.success).count() as u32;
        let attempted = results.len() as u32;
        Ok(RandomLoginBatchResult {
            requested: count,
            attempted,
            succeeded,
            failed: attempted - succeeded,
            stopped_reason,
            results,
        })
    }

    /// Load users from the given path, or fall back to the configured userinfo file.
    pub async fn load_users(&self, userinfo_path: Option<&str>) -> Result<Vec<User>> {
        let request_supplied_path = userinfo_path.is_some();
        let path = userinfo_path
            .or(self.config.userinfo_path.as_deref())
            .unwrap_or("userinfo.json");
        if path.trim().is_empty() {
            return Err(SrunError::Validation {
                field: "userinfo_path",
                message: "userinfo_path cannot be empty.".to_string(),
            });
        }
        let metadata = tokio::fs::metadata(path).await.map_err(|error| {
            credential_file_error(
                request_supplied_path,
                format!("could not read credentials file '{path}': {error}"),
            )
        })?;
        if !metadata.is_file() {
            return Err(credential_file_error(
                request_supplied_path,
                format!("credentials path '{path}' is not a file"),
            ));
        }
        if metadata.len() > MAX_USERINFO_BYTES {
            return Err(credential_file_error(
                request_supplied_path,
                format!("credentials file '{path}' exceeds the {MAX_USERINFO_BYTES}-byte limit"),
            ));
        }
        let contents = tokio::fs::read_to_string(path).await.map_err(|error| {
            credential_file_error(
                request_supplied_path,
                format!("could not read credentials file '{path}': {error}"),
            )
        })?;
        let users: Vec<User> = serde_json::from_str(&contents).map_err(|error| {
            credential_file_error(
                request_supplied_path,
                format!(
                    "credentials file '{path}' is not a valid user array (line {}, column {})",
                    error.line(),
                    error.column()
                ),
            )
        })?;
        if users.is_empty() {
            return Err(credential_file_error(
                request_supplied_path,
                format!("credentials file '{path}' does not contain any users"),
            ));
        }
        if users.len() > MAX_USERS {
            return Err(credential_file_error(
                request_supplied_path,
                format!("credentials file '{path}' contains more than {MAX_USERS} users"),
            ));
        }

        let mut usernames = HashSet::with_capacity(users.len());
        for (index, user) in users.iter().enumerate() {
            validate_credentials(&user.username, &user.password).map_err(|_| {
                credential_file_error(
                    request_supplied_path,
                    format!(
                        "credentials file '{path}' has an empty username or password at item {}",
                        index + 1
                    ),
                )
            })?;
            if !usernames.insert(user.username.clone()) {
                return Err(credential_file_error(
                    request_supplied_path,
                    format!(
                        "credentials file '{path}' contains a duplicate username at item {}",
                        index + 1
                    ),
                ));
            }
        }
        Ok(users)
    }

    /// Pick a random user from the chosen userinfo file.
    async fn random_user(&self, userinfo_path: Option<&str>) -> Result<User> {
        let users = self.load_users(userinfo_path).await?;
        Ok(users[rng().random_range(0..users.len())].clone())
    }

    // ---- Internal macvlan helpers ----

    async fn setup_macvlan(&self, parent: &str, mac: &[u8]) -> Result<MacvlanSession> {
        self.cleanup_stale_macvlans().await?;
        let name = generate_macvlan_name();
        let mut session = MacvlanSession::new(self.handle.clone(), name);
        if let Err(error) =
            net::create_macvlan(self.handle.clone(), parent, &session.name, Some(mac)).await
        {
            // A completed creation error means this call does not own the name;
            // do not let Drop remove a pre-existing interface after a collision.
            session.active = false;
            return Err(error);
        }
        let result = async {
            net::set_link_up(self.handle.clone(), &session.name).await?;

            let dhcp_info: DhcpInfo = net::dhcp_client(&session.name).await?;
            let prefix = netmask_prefix(dhcp_info.netmask)?;
            let ip_net = Ipv4Network::new(dhcp_info.ip, prefix)
                .map_err(|e| SrunError::Dhcp(format!("invalid IP/prefix: {}", e)))?;

            net::add_address(self.handle.clone(), &session.name, IpNetwork::V4(ip_net)).await?;
            net::add_default_route(
                self.handle.clone(),
                &session.name,
                dhcp_info.gateway,
                dhcp_info.ip,
            )
            .await?;

            session.lease = Some(dhcp_info);
            Ok(())
        }
        .await;

        if let Err(error) = result {
            debug!(
                interface = %session.name,
                error = %error,
                "macvlan setup failed, cleaning up partial interface"
            );
            return Err(finish_failed_session(&mut session, error, "macvlan setup").await);
        }

        Ok(session)
    }

    async fn cleanup_stale_macvlans(&self) -> Result<()> {
        for link in net::dump_links(self.handle.clone()).await? {
            if !is_managed_macvlan_name(&link.name) {
                continue;
            }

            match net::del_macvlan(self.handle.clone(), &link.name).await {
                Ok(()) | Err(SrunError::InterfaceNotFound(_)) => {
                    info!(interface = %link.name, "removed stale macvlan");
                }
                Err(error) => {
                    warn!(
                        interface = %link.name,
                        error = %error,
                        "failed to remove stale macvlan"
                    );
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    async fn login_macvlan_locked(
        &self,
        parent: &str,
        mac: &[u8],
        username: &str,
        password: &str,
    ) -> Result<LoginResult> {
        let mac_str = format_mac(mac);
        let mut session = self.setup_macvlan(parent, mac).await?;
        let result = self
            .do_macvlan_login(
                &session.name,
                session_ip(&session)?,
                username,
                password,
                &mac_str,
            )
            .await;
        let result = finish_session(&mut session, result, "login").await;

        match &result {
            Ok(login) => {
                info!(username = %login.username, ip = %login.ip, mac = %mac_str, "login successful (macvlan)")
            }
            Err(error) => debug!(mac = %mac_str, error = %error, "login failed (macvlan)"),
        }
        result
    }

    async fn do_macvlan_login(
        &self,
        interface: &str,
        expected_ip: Ipv4Addr,
        username: &str,
        password: &str,
        mac_str: &str,
    ) -> Result<LoginResult> {
        let client = self.build_client(interface)?;
        let callback = srun_utils::generate_jsonp_callback();

        let userinfo = self.srun_client.get_userinfo(&client, &callback).await?;
        check_ip(expected_ip, userinfo.ip)?;
        check_not_online(&userinfo)?;

        let challenge = self
            .srun_client
            .get_challenge(&client, &callback, username, userinfo.ip)
            .await?;

        self.srun_client
            .login(
                &client,
                &callback,
                username,
                password,
                userinfo.ip,
                &challenge,
            )
            .await?;

        Ok(LoginResult {
            ip: userinfo.ip,
            username: username.to_string(),
            mac: Some(mac_str.to_string()),
        })
    }

    async fn do_macvlan_status(
        &self,
        interface: &str,
        expected_ip: Ipv4Addr,
    ) -> Result<StatusResult> {
        let client = self.build_client(interface)?;
        let callback = srun_utils::generate_jsonp_callback();
        let userinfo = self.srun_client.get_userinfo(&client, &callback).await?;
        check_ip(expected_ip, userinfo.ip)?;
        Ok(StatusResult {
            ip: userinfo.ip,
            online_user: userinfo.online_user,
            online_mac: userinfo.online_mac,
        })
    }

    async fn do_macvlan_logout(&self, interface: &str, expected_ip: Ipv4Addr) -> Result<()> {
        let client = self.build_client(interface)?;
        let callback = srun_utils::generate_jsonp_callback();

        let userinfo = self.srun_client.get_userinfo(&client, &callback).await?;
        check_ip(expected_ip, userinfo.ip)?;
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

impl RandomLoginResult {
    fn from_result(mac: String, result: Result<LoginResult>) -> Self {
        match result {
            Ok(data) => Self {
                mac,
                success: true,
                data: Some(data),
                error: None,
            },
            Err(error) => Self {
                mac,
                success: false,
                data: None,
                error: Some(AttemptError {
                    code: error.code(),
                    message: error.public_message(),
                }),
            },
        }
    }
}

fn check_not_online(userinfo: &UserInfo) -> Result<()> {
    if let Some(user) = &userinfo.online_user {
        return Err(SrunError::AlreadyOnline {
            user: user.clone(),
            mac: userinfo
                .online_mac
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
        });
    }
    Ok(())
}

fn check_ip(expected: Ipv4Addr, portal: Ipv4Addr) -> Result<()> {
    if expected != portal {
        return Err(SrunError::IpMismatch {
            dhcp: expected,
            portal,
        });
    }
    Ok(())
}

fn validate_interface(interface: &str, field: &'static str) -> Result<()> {
    if interface.trim().is_empty() {
        return Err(SrunError::Validation {
            field,
            message: format!("{field} is required."),
        });
    }
    if interface != interface.trim() || interface.len() > 15 || interface.contains('\0') {
        return Err(SrunError::Validation {
            field,
            message: format!("{field} is not a valid Linux interface name."),
        });
    }
    Ok(())
}

fn validate_credentials(username: &str, password: &str) -> Result<()> {
    if username.trim().is_empty() {
        return Err(SrunError::Validation {
            field: "username",
            message: "username cannot be empty.".to_string(),
        });
    }
    if password.is_empty() {
        return Err(SrunError::Validation {
            field: "password",
            message: "password cannot be empty.".to_string(),
        });
    }
    Ok(())
}

fn credential_file_error(request_supplied_path: bool, details: String) -> SrunError {
    if request_supplied_path {
        SrunError::UserData(details)
    } else {
        SrunError::CredentialStore(details)
    }
}

fn netmask_prefix(netmask: Ipv4Addr) -> Result<u8> {
    let bits = u32::from(netmask);
    let inverted = !bits;
    if bits == 0 || inverted & inverted.wrapping_add(1) != 0 {
        return Err(SrunError::Dhcp(format!(
            "invalid non-contiguous subnet mask {netmask}"
        )));
    }
    Ok(bits.count_ones() as u8)
}

fn session_ip(session: &MacvlanSession) -> Result<Ipv4Addr> {
    session
        .lease
        .as_ref()
        .map(|lease| lease.ip)
        .ok_or_else(|| SrunError::Dhcp("temporary interface has no DHCP lease".to_string()))
}

async fn finish_session<T>(
    session: &mut MacvlanSession,
    result: Result<T>,
    operation: &'static str,
) -> Result<T> {
    match session.cleanup().await {
        Ok(()) => result,
        Err(cleanup_error) => {
            error!(
                interface = %session.name,
                error = %cleanup_error,
                "macvlan cleanup failed; a deferred retry will be scheduled"
            );
            match result {
                Ok(_) => Err(SrunError::CleanupFailed {
                    operation,
                    details: cleanup_error.to_string(),
                }),
                Err(primary_error) => Err(combine_operation_and_cleanup_errors(
                    operation,
                    primary_error,
                    cleanup_error,
                )),
            }
        }
    }
}

async fn finish_failed_session(
    session: &mut MacvlanSession,
    primary_error: SrunError,
    operation: &'static str,
) -> SrunError {
    match session.cleanup().await {
        Ok(()) => primary_error,
        Err(cleanup_error) => {
            error!(
                interface = %session.name,
                error = %cleanup_error,
                "macvlan cleanup also failed; a deferred retry will be scheduled"
            );
            combine_operation_and_cleanup_errors(operation, primary_error, cleanup_error)
        }
    }
}

fn combine_operation_and_cleanup_errors(
    operation: &'static str,
    primary_error: SrunError,
    cleanup_error: SrunError,
) -> SrunError {
    error!(
        error = %primary_error,
        cleanup_error = %cleanup_error,
        "operation and cleanup both failed"
    );
    SrunError::OperationAndCleanupFailed {
        operation,
        primary: primary_error.to_string(),
        cleanup: cleanup_error.to_string(),
    }
}

fn generate_macvlan_name() -> String {
    let suffix: u64 = rng().random::<u64>() & 0x0000_ffff_ffff_ffff;
    format!("{MACVLAN_PREFIX}{suffix:012x}")
}

fn is_managed_macvlan_name(name: &str) -> bool {
    if name.len() != MACVLAN_NAME_LEN || !name.starts_with(MACVLAN_PREFIX) {
        return false;
    }
    let suffix = &name[MACVLAN_PREFIX.len()..];
    suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn format_mac(mac: &[u8]) -> String {
    mac.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(":")
}

pub fn parse_mac(s: &str) -> Result<[u8; 6]> {
    let value = s.trim();
    let separator = match (value.contains(':'), value.contains('-')) {
        (true, false) => ':',
        (false, true) => '-',
        _ => {
            return Err(SrunError::InvalidMac(
                "use six two-digit octets separated by ':' or '-'".to_string(),
            ));
        }
    };
    let segments: Vec<_> = value.split(separator).collect();
    if segments.len() != 6 {
        return Err(SrunError::InvalidMac(format!(
            "expected 6 octets, got {}",
            segments.len()
        )));
    }

    let mut mac = [0u8; 6];
    for (index, segment) in segments.iter().enumerate() {
        if segment.len() != 2 || !segment.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(SrunError::InvalidMac(format!(
                "octet {} must contain exactly two hexadecimal digits",
                index + 1
            )));
        }
        mac[index] = u8::from_str_radix(segment, 16).map_err(|_| {
            SrunError::InvalidMac(format!("octet {} is not hexadecimal", index + 1))
        })?;
    }
    validate_mac_bytes(&mac)?;
    Ok(mac)
}

fn validate_mac_bytes(mac: &[u8]) -> Result<()> {
    if mac.len() != 6 {
        return Err(SrunError::InvalidMac(format!(
            "expected 6 octets, got {}",
            mac.len()
        )));
    }
    if mac.iter().all(|byte| *byte == 0) {
        return Err(SrunError::InvalidMac(
            "the all-zero address is not usable".to_string(),
        ));
    }
    if mac.iter().all(|byte| *byte == 0xff) {
        return Err(SrunError::InvalidMac(
            "the broadcast address is not usable".to_string(),
        ));
    }
    if mac[0] & 1 != 0 {
        return Err(SrunError::InvalidMac(
            "multicast addresses are not usable for a network interface".to_string(),
        ));
    }
    Ok(())
}

pub fn generate_mac_address() -> [u8; 6] {
    let mut r = rng();
    let mut mac = [0u8; 6];
    r.fill(&mut mac);
    // Set locally administered bit, clear multicast bit
    mac[0] = (mac[0] & 0b11111110) | 0b00000010;
    mac
}

#[cfg(test)]
mod tests {
    use super::{
        NetworkNamespaceLock, UserInfo, check_not_online, generate_mac_address,
        generate_macvlan_name, is_managed_macvlan_name, netmask_prefix, parse_mac,
    };
    use crate::error::SrunError;
    use std::net::Ipv4Addr;

    #[test]
    fn parses_and_normalizes_valid_mac_addresses() {
        assert_eq!(
            parse_mac("02:1A:2b:3C:4d:5E").unwrap(),
            [0x02, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e]
        );
        assert_eq!(
            parse_mac("02-1a-2b-3c-4d-5e").unwrap(),
            [0x02, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e]
        );
    }

    #[test]
    fn rejects_malformed_or_unusable_mac_addresses() {
        for value in [
            "2:1a:2b:3c:4d:5e",
            "01:1a:2b:3c:4d:5e",
            "00:00:00:00:00:00",
            "ff:ff:ff:ff:ff:ff",
            "02:1a-2b:3c:4d:5e",
        ] {
            assert!(parse_mac(value).is_err(), "{value} should be rejected");
        }
    }

    #[test]
    fn generated_mac_is_local_unicast() {
        for _ in 0..32 {
            let mac = generate_mac_address();
            assert_eq!(mac[0] & 0b0000_0011, 0b0000_0010);
        }
    }

    #[test]
    fn validates_contiguous_netmasks() {
        assert_eq!(netmask_prefix(Ipv4Addr::new(255, 255, 255, 0)).unwrap(), 24);
        assert!(netmask_prefix(Ipv4Addr::new(255, 0, 255, 0)).is_err());
    }

    #[test]
    fn username_alone_still_means_online() {
        let info = UserInfo {
            ip: Ipv4Addr::new(192, 0, 2, 1),
            online_user: Some("alice".to_string()),
            online_mac: None,
        };
        assert!(check_not_online(&info).is_err());
    }

    #[test]
    fn managed_macvlan_names_are_linux_safe_and_recognizable() {
        let name = generate_macvlan_name();
        assert_eq!(name.len(), 15);
        assert!(is_managed_macvlan_name(&name));
        assert!(!is_managed_macvlan_name("srun"));
    }

    #[test]
    fn namespace_lock_allows_only_one_mutation() {
        let first = NetworkNamespaceLock::try_acquire().unwrap();
        assert!(matches!(
            NetworkNamespaceLock::try_acquire(),
            Err(SrunError::OperationBusy)
        ));
        drop(first);
        assert!(NetworkNamespaceLock::try_acquire().is_ok());
    }
}
