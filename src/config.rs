//! Handling the Config and associated objects.

use axum_server::tls_rustls::RustlsConfig;
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use tracing::{event, Level};

use crate::ldap::{LDAPBackend, LDAPError};



#[derive(Debug)]
pub(crate) enum ConfigError {
    PoolCreationError(sqlx::Error),
    TlsConfigError(std::io::Error),
    ReadConfigFileError(std::io::Error),
    ParseConfigFileError(toml::de::Error),
    LdapConnectionError(LDAPError),
}
impl core::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PoolCreationError(x) => {
                write!(f, "Error creating PSQL Pool: {x}")
            },
            Self::TlsConfigError(x) => {
                write!(f, "Error creating TLS config: {x}")
            }
            Self::ReadConfigFileError(x) => {
                write!(f, "Error reading Config File: {x}")
            }
            Self::ParseConfigFileError(x) => {
                write!(f, "Error parsing Config File as toml: {x}")
            }
            Self::LdapConnectionError(x) => {
                write!(f, "Error connecting to LDAP: {x}")
            }
        }
    }
}
impl std::error::Error for ConfigError {}



/// Config as present in file. This object will be used to create a Config object.
#[derive(Debug, Deserialize)]
struct ConfigData {
    ldap: LdapConfigData,
    db: DbConfigData,
    web: WebConfigData,
}

#[derive(Deserialize)]
struct LdapConfigData {
    server_host: String,
    server_port: u16,
    bind_dn: String,
    bind_password: String,
    trust_ca: String,
    user_base_dn: String,
    user_filter: String,
    write_access_filter: String,
}
impl core::fmt::Debug for LdapConfigData {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("ConfigFileData")
            .field("server_host", &self.server_host)
            .field("server_port", &self.server_port)
            .field("bind_dn", &self.bind_dn)
            .field("bind_password", &"[redacted]")
            .field("trust_ca", &self.trust_ca)
            .field("user_base_dn", &self.user_base_dn)
            .field("user_filter", &self.user_filter)
            .field("write_access_filter", &self.write_access_filter)
            .finish()
    }
}

#[derive(Deserialize)]
struct DbConfigData {
    host: String,
    port: u16,
    database: String,
    user: String,
    password: String,
}
impl core::fmt::Debug for DbConfigData {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("ConfigFileData")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("user", &self.user)
            .field("password", &"[redacted]")
            .finish()
    }
}

#[derive(Debug, Deserialize)]
struct WebConfigData {
    bind_address: String,
    bind_port: u16,
    bind_port_tls: u16,
    cert_file: String,
    key_file: String,
}
impl WebConfigData {
    async fn try_into_web_config(self) -> Result<WebConfig, ConfigError> {
        // webserver settings
        let bind_string = format!(
            "{}:{}",
            self.bind_address, self.bind_port
        );
        let bind_string_tls = format!(
            "{}:{}",
            self.bind_address, self.bind_port_tls
        );
        let rustls_config =
            match RustlsConfig::from_pem_file(self.cert_file, self.key_file)
                .await
            {
                Ok(x) => x,
                Err(e) => {
                    event!(
                        Level::ERROR,
                        "There was a problem reading the TLS cert/key: {e}"
                    );
                    return Err(ConfigError::TlsConfigError(e));
                }
            };
        Ok(WebConfig{ bind_string, bind_string_tls, rustls_config, })
    }
}


struct DbConfig {
    pool: Pool<Postgres>,
}
impl DbConfig {
    async fn try_from_db_config_data(value: DbConfigData) -> Result<Self, ConfigError> {
        // postgres settings
        let url = format!(
            "postgres://{}:{}@{}:{}/{}",
            value.user,
            value.password,
            value.host,
            value.port,
            value.database
        );
        let pool = match sqlx::postgres::PgPool::connect(&url).await {
            Ok(x) => x,
            Err(e) => {
                event!(Level::ERROR, "Could not connect to postgres: {e}");
                return Err(ConfigError::PoolCreationError(e));
            }
        };
        Ok(Self { pool, })
    }
}


#[derive(Debug)]
struct WebConfig {
    bind_string: String,
    bind_string_tls: String,
    rustls_config: RustlsConfig,
}


/// Create a pg_pool from the [`DbConfigData`]
async fn pg_pool_from_db_config_data(value: DbConfigData) -> Result<Pool<Postgres>, ConfigError> {
    // postgres settings
    let url = format!(
        "postgres://{}:{}@{}:{}/{}",
        value.user,
        value.password,
        value.host,
        value.port,
        value.database
    );
    match sqlx::postgres::PgPool::connect(&url).await {
        Ok(pool) => {
            Ok(pool)
        }
        Err(e) => {
            event!(Level::ERROR, "Could not connect to postgres: {e}");
            Err(ConfigError::PoolCreationError(e))
        }
    }
}

pub(crate) struct Config {
    ldap_backend: LDAPBackend,
    pg_pool: Pool<Postgres>,
    web_config: WebConfig,
}
impl Config {
    pub async fn create() -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string("/etc/projekttagebuch/config.toml")
            .map_err(|x| ConfigError::ReadConfigFileError(x))?;
        let config_data: ConfigData = toml::from_str(&content)
            .map_err(|x| ConfigError::ParseConfigFileError(x))?;

        // LDAP
        let ldap_backend = match crate::ldap::LDAPBackend::new(
            &config_data.ldap.server_host,
            config_data.ldap.server_port,
            &config_data.ldap.bind_dn,
            &config_data.ldap.bind_password,
            &config_data.ldap.user_filter,
            &config_data.ldap.write_access_filter,
            &config_data.ldap.user_base_dn,
        )
        .await
        {
            Ok(x) => x,
            Err(e) => {
                event!(
                    Level::ERROR,
                    "LDAP connection could not be established: {e}"
                );
                return Err(ConfigError::LdapConnectionError(e));
            }
        };

        // DB
        let pg_pool = pg_pool_from_db_config_data(config_data.db).await?;

        // Web
        let web_config = config_data.web.try_into_web_config().await?;
        Ok(Self { ldap_backend, pg_pool, web_config, })
    }
}
