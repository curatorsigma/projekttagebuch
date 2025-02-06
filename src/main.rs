use std::{str::FromStr, sync::Arc};

use config::Config;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    filter,
    fmt::{self, format::FmtSpan},
    prelude::*,
    EnvFilter,
};

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

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("projekttagebuch.log")
        .build(&config.log_location)
        .expect("Should be able to create file appender");
    let (writer, _guard) = tracing_appender::non_blocking(file_appender);

    let my_crate_filter = EnvFilter::new("projekttagebuch");

    let level_filter = filter::LevelFilter::from_str(&config.log_level)?;

    let subscriber = tracing_subscriber::registry()
        .with(my_crate_filter)
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
                .with_line_number(true)
                .with_filter(level_filter),
        )
        .with(fmt::Layer::default().with_writer(writer));
    if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("Error setting global tracing subscriber: {e}");
        Err(e)?;
    };

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
