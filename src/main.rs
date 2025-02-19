use std::{str::FromStr, sync::Arc};

use config::Config;
use tracing::debug;
use tracing_subscriber::{filter, fmt::format::FmtSpan, prelude::*, EnvFilter};

mod config;
mod db;
mod ldap;
mod sync;
mod types;
mod web_server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let config = Arc::new(Config::create().await?);

    let my_crate_filter = EnvFilter::new("projekttagebuch");

    let level_filter = filter::LevelFilter::from_str(&config.log_level)?;

    let subscriber = tracing_subscriber::registry().with(my_crate_filter).with(
        tracing_subscriber::fmt::layer()
            .compact()
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
            .with_line_number(true)
            .with_filter(level_filter),
    );
    if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("Error setting global tracing subscriber: {e}");
        Err(e)?;
    };
    debug!("Successfully instantiated tracing.");

    sqlx::migrate!().run(&config.pg_pool).await?;

    // start the web server
    let webserver = web_server::Webserver::new().await?;
    let web_handle = tokio::spawn(async move {
        if let Err(e) = webserver.run_web_server(config).await {
            eprintln!("Could not start the web server: {e}");
            panic!("Unable to start web server. Unrecoverable");
        };
    });
    let res = tokio::join!(web_handle);
    res.0?;

    Ok(())
}
