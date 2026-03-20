use crate::error::Result;
use crate::net::Link;
use crate::service::{SrunService, parse_mac};
use inquire::{Password, PasswordDisplayMode, Select, Text};
use std::fmt::{Display, Formatter};
use std::sync::Arc;

pub async fn run(service: Arc<SrunService>) -> Result<()> {
    println!("SRUN Auto Dialer");

    let mode = Select::new("Select your dial mode:", vec![
        DialMacMode::Local,
        DialMacMode::Custom,
        DialMacMode::Random,
    ])
    .prompt()?;

    match mode {
        DialMacMode::Local => local_mode(&service).await,
        DialMacMode::Custom => custom_mode(&service).await,
        DialMacMode::Random => random_mode(&service).await,
    }
}

async fn local_mode(service: &SrunService) -> Result<()> {
    let link = select_link(service, "Select a link:").await?;
    let operation = select_operation()?;

    match operation {
        Operation::Login => {
            let creds = get_credentials()?;
            let result = service.login_local(&link.name, creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str()))).await?;
            println!("Login successful, IP: {}", result.ip);
        }
        Operation::Logout => {
            service.logout_local(&link.name).await?;
            println!("Logout successful");
        }
        Operation::Status => {
            let status = service.get_status(&link.name).await?;
            print_status(&status);
        }
    }
    Ok(())
}

async fn custom_mode(service: &SrunService) -> Result<()> {
    let link = select_link(service, "Select the parent link:").await?;
    let mac_input = Text::new("Enter the custom MAC address (e.g., AA:BB:CC:DD:EE:FF):")
        .prompt()?;
    let mac = parse_mac(&mac_input)?;

    let operation = select_operation()?;
    match operation {
        Operation::Login => {
            let creds = get_credentials()?;
            let result = service
                .login_macvlan(&link.name, &mac, creds.as_ref().map(|(u, p)| (u.as_str(), p.as_str())))
                .await?;
            println!(
                "Login successful, User: {}, IP: {}",
                result.username, result.ip
            );
        }
        Operation::Logout => {
            service.logout_macvlan(&link.name).await?;
            println!("Logout successful");
        }
        Operation::Status => {
            // For status with macvlan, use local status on parent
            let status = service.get_status(&link.name).await?;
            print_status(&status);
        }
    }
    Ok(())
}

async fn random_mode(service: &SrunService) -> Result<()> {
    let link = select_link(service, "Select the parent link:").await?;
    let count: u32 = Text::new("Enter the number of random MAC addresses to try:")
        .with_default("1")
        .prompt()?
        .parse()
        .map_err(|_| crate::error::SrunError::Config("invalid number".to_string()))?;

    let results = service.login_random(&link.name, count).await?;

    println!("\n--- Results ---");
    for r in &results {
        match &r.result {
            Ok(login) => println!(
                "MAC: {} -> Login OK, User: {}, IP: {}",
                r.mac, login.username, login.ip
            ),
            Err(e) => println!("MAC: {} -> Failed: {}", r.mac, e),
        }
    }

    let success_count = results.iter().filter(|r| r.result.is_ok()).count();
    println!(
        "\nTotal: {}, Success: {}, Failed: {}",
        results.len(),
        success_count,
        results.len() - success_count
    );
    Ok(())
}

// ---- UI helpers ----

fn get_credentials() -> Result<Option<(String, String)>> {
    let mode = Select::new("Select how to input user information:", vec![
        UserMode::Input,
        UserMode::Read,
    ])
    .prompt()?;

    match mode {
        UserMode::Input => {
            let username = Text::new("Enter username:").prompt()?;
            let password = Password::new("Enter password:")
                .with_display_mode(PasswordDisplayMode::Masked)
                .without_confirmation()
                .prompt()?;
            Ok(Some((username, password)))
        }
        UserMode::Read => Ok(None),
    }
}

async fn select_link(service: &SrunService, msg: &str) -> Result<Link> {
    let links = service.list_interfaces().await?;
    let link = Select::new(msg, links).prompt()?;
    Ok(link)
}

fn select_operation() -> Result<Operation> {
    let op = Select::new("Select operation:", vec![
        Operation::Login,
        Operation::Logout,
        Operation::Status,
    ])
    .prompt()?;
    Ok(op)
}

fn print_status(status: &crate::service::StatusResult) {
    println!("IP: {}", status.ip);
    match (&status.online_user, &status.online_mac) {
        (Some(user), Some(mac)) => println!("Online: {} (MAC: {})", user, mac),
        _ => println!("No user currently online"),
    }
}

// ---- Display enums ----

#[derive(Debug, Clone, Copy)]
enum DialMacMode {
    Local,
    Custom,
    Random,
}

impl Display for DialMacMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local => write!(f, "Using the local network adapter's MAC address"),
            Self::Custom => write!(f, "Using a custom MAC address"),
            Self::Random => write!(
                f,
                "Using random MAC addresses (reads users from userinfo.json)"
            ),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Operation {
    Login,
    Logout,
    Status,
}

impl Display for Operation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Login => write!(f, "Login"),
            Self::Logout => write!(f, "Logout"),
            Self::Status => write!(f, "Check status"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum UserMode {
    Input,
    Read,
}

impl Display for UserMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input => write!(f, "Enter username and password manually"),
            Self::Read => write!(f, "Read from userinfo.json"),
        }
    }
}
