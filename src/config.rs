//! Handling the Config and associated objects.

use axum_server::tls_rustls::RustlsConfig;
use matrix_sdk::{Client, ClientBuildError};
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use tracing::{event, Level};

use crate::ldap::{LDAPBackend, LDAPError};
use crate::matrix::MatrixClient;

#[derive(Debug)]
pub(crate) enum ConfigError {
    PoolCreationError(sqlx::Error),
    TlsCertKeyError(std::io::Error),
    ReadConfigFileError(std::io::Error),
    ParseConfigFileError(toml::de::Error),
    LdapConnectionError(LDAPError),
    MatrixClientCreationError(ClientBuildError),
    MatrixLoginError(matrix_sdk::Error),
}
impl core::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PoolCreationError(x) => {
                write!(f, "Error creating PSQL Pool: {x}")
            }
            Self::TlsCertKeyError(x) => {
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
            Self::MatrixClientCreationError(e) => {
                write!(f, "Error creating Matrix Client: {e}")
            }
            Self::MatrixLoginError(e) => {
                write!(f, "Error logging in to Matrix: {e}")
            }
        }
    }
}
impl std::error::Error for ConfigError {}

/// Config as present in file. This object will be used to create a Config object.
#[derive(Debug, Deserialize)]
struct ConfigData {
    log_level: String,
    log_location: Option<String>,
    user_resync_interval: Option<u32>,
    room_resync_interval: Option<u32>,
    ldap: LdapConfigData,
    db: DbConfigData,
    web: WebConfigData,
    matrix: MatrixConfigData,
}

#[derive(Deserialize)]
struct LdapConfigData {
    server_host: String,
    server_port: u16,
    bind_dn: String,
    bind_password: String,
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
        let rustls_config = match RustlsConfig::from_pem_file(self.cert_file, self.key_file).await {
            Ok(x) => x,
            Err(e) => {
                event!(
                    Level::ERROR,
                    "There was a problem reading the TLS cert/key: {e}"
                );
                return Err(ConfigError::TlsCertKeyError(e));
            }
        };
        Ok(WebConfig {
            bind_address: self.bind_address,
            bind_port: self.bind_port,
            bind_port_tls: self.bind_port_tls,
            rustls_config,
        })
    }
}

#[derive(Deserialize)]
pub(crate) struct MatrixConfigData {
    /// server name to which rooms and users are relative (example.com)
    servername: String,
    /// Name of the associated webmatrix (we call it element for convenience)
    element_servername: String,
    /// server url that actually hosts the matrix server (https://matrix.example.com)
    homeserver_url: String,
    /// username local part to log in with (exampleuser, NOT @exampleuser:example.com)
    /// This user will be used to create rooms and invite users
    username: String,
    /// password for that user
    password: String,
}
impl core::fmt::Debug for MatrixConfigData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MatrixConfigData")
            .field("homeserver_url", &self.homeserver_url)
            .field("servername", &self.servername)
            .field("element_servername", &self.element_servername)
            .field("username", &self.username)
            .field("password", &"[redacted]")
            .finish()
    }
}
impl MatrixConfigData {
    async fn try_into_matrix_client(self) -> Result<MatrixClient, ConfigError> {
        let client = Client::builder()
            .homeserver_url(self.homeserver_url)
            .build()
            .await
            .map_err(ConfigError::MatrixClientCreationError)?;
        client
            .matrix_auth()
            .login_username(
                format!("@{}:{}", self.username, self.servername),
                &self.password,
            )
            .send()
            .await
            .map_err(ConfigError::MatrixLoginError)?;
        Ok(MatrixClient::new(client, self.servername, self.element_servername))
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
            value.user, value.password, value.host, value.port, value.database
        );
        let pool = match sqlx::postgres::PgPool::connect(&url).await {
            Ok(x) => x,
            Err(e) => {
                event!(Level::ERROR, "Could not connect to postgres: {e}");
                return Err(ConfigError::PoolCreationError(e));
            }
        };
        Ok(Self { pool })
    }
}

#[derive(Debug)]
pub(crate) struct WebConfig {
    pub(crate) bind_address: String,
    pub(crate) bind_port: u16,
    pub(crate) bind_port_tls: u16,
    pub(crate) rustls_config: RustlsConfig,
}

/// Create a pg_pool from the [`DbConfigData`]
async fn pg_pool_from_db_config_data(value: DbConfigData) -> Result<Pool<Postgres>, ConfigError> {
    // postgres settings
    let url = format!(
        "postgres://{}:{}@{}:{}/{}",
        value.user, value.password, value.host, value.port, value.database
    );
    match sqlx::postgres::PgPool::connect(&url).await {
        Ok(pool) => Ok(pool),
        Err(e) => {
            event!(Level::ERROR, "Could not connect to postgres: {e}");
            Err(ConfigError::PoolCreationError(e))
        }
    }
}

#[derive(Debug)]
pub(crate) struct Config {
    pub(crate) log_level: String,
    pub(crate) log_location: String,
    pub(crate) user_resync_interval: u32,
    pub(crate) room_resync_interval: u32,
    pub(crate) ldap_backend: LDAPBackend,
    pub(crate) pg_pool: Pool<Postgres>,
    pub(crate) web_config: WebConfig,
    pub(crate) matrix_client: MatrixClient,
}
impl Config {
    pub async fn create() -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string("/etc/projekttagebuch/config.toml")
            .map_err(ConfigError::ReadConfigFileError)?;
        let config_data: ConfigData =
            toml::from_str(&content).map_err(ConfigError::ParseConfigFileError)?;

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

        // Matrix
        let matrix_client = config_data.matrix.try_into_matrix_client().await?;

        Ok(Self {
            log_level: config_data.log_level,
            log_location: config_data
                .log_location
                .unwrap_or("/var/log/projekttagebuch".to_owned()),
            user_resync_interval: config_data.user_resync_interval.unwrap_or(10),
            room_resync_interval: config_data.room_resync_interval.unwrap_or(10),
            ldap_backend,
            pg_pool,
            web_config,
            matrix_client,
        })
    }
}
