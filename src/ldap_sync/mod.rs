//! Sync users from LDAP into the DB

use std::sync::Arc;

use tracing::{debug, info, warn};

use crate::{
    config::Config,
    db::{update_users, DBError},
    ldap::LDAPError,
    InShutdown,
};

#[derive(Debug)]
enum SyncError {
    DB(DBError),
    LDAP(LDAPError),
}
impl core::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DB(x) => write!(f, "Error while updating DB: {x}."),
            Self::LDAP(x) => write!(f, "Error while reading from LDAP: {x}."),
        }
    }
}
impl std::error::Error for SyncError {}
impl From<DBError> for SyncError {
    fn from(value: DBError) -> Self {
        Self::DB(value)
    }
}
impl From<LDAPError> for SyncError {
    fn from(value: LDAPError) -> Self {
        Self::LDAP(value)
    }
}

/// Fetch users from LDAP and update, once.
async fn update_users_in_db(config: Arc<Config>) -> Result<(), SyncError> {
    // get users from ldap
    let users = config.ldap_backend.get_all_users().await?;
    update_users(config.pg_pool.clone(), users).await?;
    Ok(())
}

pub async fn continuous_sync(
    config: Arc<Config>,
    mut watcher: tokio::sync::watch::Receiver<InShutdown>,
) {
    info!("Starting LDAP -> DB Sync task.");
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(
        config.user_resync_interval as u64 * 60,
    ));
    interval.tick().await;
    loop {
        debug!("LDAP->DB Sync starting new run.");
        // get new data
        let sync_res = update_users_in_db(config.clone()).await;
        match sync_res {
            Ok(()) => debug!("Successfully updated db."),
            Err(e) => {
                warn!("Failed to update db from CT. Error encountered: {e}");
            }
        };

        // stop on cancellation or continue after the next tick
        tokio::select! {
            _ = watcher.changed() => {
                debug!("Shutting down data gatherer now.");
                return;
            }
            _ = interval.tick() => {}
        }
    }
}
