use config::Config;

mod config;
mod ldap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::create().await?;
    Ok(())
}
