use std::{str::FromStr, sync::Arc};

use config::Config;
use tracing::{debug, error, info};
use tracing_subscriber::{filter, fmt::format::FmtSpan, prelude::*, EnvFilter};

mod actions;
mod config;
mod db;
mod ldap;
mod ldap_sync;
mod matrix;
mod types;
mod web_server;

enum InShutdown {
    Yes,
    No,
}

async fn signal_handler(
    mut watcher: tokio::sync::watch::Receiver<InShutdown>,
    shutdown_tx: tokio::sync::watch::Sender<InShutdown>,
) -> Result<(), std::io::Error> {
    let mut sigterm = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
    {
        Ok(x) => x,
        Err(e) => {
            error!("Failed to install SIGTERM listener: {e} Aborting.");
            shutdown_tx.send_replace(InShutdown::Yes);
            return Err(e);
        }
    };
    let mut sighup = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup()) {
        Ok(x) => x,
        Err(e) => {
            error!("Failed to install SIGHUP listener: {e} Aborting.");
            shutdown_tx.send_replace(InShutdown::Yes);
            return Err(e);
        }
    };
    let mut sigint = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
    {
        Ok(x) => x,
        Err(e) => {
            error!("Failed to install SIGINT listener: {e} Aborting.");
            shutdown_tx.send_replace(InShutdown::Yes);
            return Err(e);
        }
    };
    // wait for a shutdown signal
    tokio::select! {
        // shutdown the signal handler when some other process signals a shutdown
        _ = watcher.changed() => {}
        _ = sigterm.recv() => {
            info!("Got SIGTERM. Shuting down.");
            shutdown_tx.send_replace(InShutdown::Yes);
        }
        _ = sighup.recv() => {
            info!("Got SIGHUP. Shuting down.");
            shutdown_tx.send_replace(InShutdown::Yes);
        }
        _ = sigint.recv() => {
            info!("Got SIGINT. Shuting down.");
            shutdown_tx.send_replace(InShutdown::Yes);
        }
        x = tokio::signal::ctrl_c() =>  {
            match x {
                Ok(()) => {
                    info!("Received Ctrl-c. Shutting down.");
                    shutdown_tx.send_replace(InShutdown::Yes);
                }
                Err(err) => {
                    error!("Unable to listen for shutdown signal: {}", err);
                    // we also shut down in case of error
                    shutdown_tx.send_replace(InShutdown::Yes);
                }
            }
        }
    };

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("installing crypto provider");
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let config = Arc::new(Config::create().await?);
    println!("got config");

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
    debug!("Successfully migrated db.");

    // cancellation channel
    let (tx, rx) = tokio::sync::watch::channel(InShutdown::No);

    let sync_handle = tokio::spawn(ldap_sync::continuous_sync(config.clone(), rx));

    // start the Signal handler
    let signal_handle = tokio::spawn(signal_handler(tx.subscribe(), tx.clone()));

    // start the web server
    let webserver = web_server::Webserver::new().await?;
    let web_watcher = tx.subscribe();
    let web_handle = tokio::spawn(async move {
        if let Err(e) = webserver.run_web_server(config, web_watcher).await {
            eprintln!("Could not start the web server: {e}");
            panic!("Unable to start web server. Unrecoverable");
        };
    });

    let (signal_res, sync_res, web_res) = tokio::join!(signal_handle, sync_handle, web_handle);
    signal_res??;
    sync_res?;
    web_res?;

    Ok(())
}
