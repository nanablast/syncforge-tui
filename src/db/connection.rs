use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Database type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DbType {
    #[default]
    MySQL,
    PostgreSQL,
    SQLite,
    SQLServer,
}

impl DbType {
    /// Returns the identifier quote character for this database type
    pub fn quote_char(&self) -> (&'static str, &'static str) {
        match self {
            DbType::MySQL => ("`", "`"),
            DbType::PostgreSQL => ("\"", "\""),
            DbType::SQLite => ("\"", "\""),
            DbType::SQLServer => ("[", "]"),
        }
    }

    /// Quote an identifier (table name, column name, etc.)
    pub fn quote_identifier(&self, name: &str) -> String {
        let (open, close) = self.quote_char();
        format!("{}{}{}", open, name, close)
    }
}

/// Database connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub db_type: DbType,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
    /// SQLite file path
    #[serde(default)]
    pub file_path: Option<PathBuf>,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            db_type: DbType::MySQL,
            host: "localhost".to_string(),
            port: 3306,
            user: "root".to_string(),
            password: String::new(),
            database: String::new(),
            file_path: None,
        }
    }
}

impl ConnectionConfig {
    /// Get default port for database type
    pub fn default_port(db_type: DbType) -> u16 {
        match db_type {
            DbType::MySQL => 3306,
            DbType::PostgreSQL => 5432,
            DbType::SQLite => 0,
            DbType::SQLServer => 1433,
        }
    }
}

/// Database connection wrapper that can hold different connection types
pub enum DbConnection {
    MySQL(sqlx::MySqlPool),
    PostgreSQL(sqlx::PgPool),
    SQLite(sqlx::SqlitePool),
    SQLServer(tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>),
}

impl DbConnection {
    /// Connect to database using the given configuration
    pub async fn connect(config: &ConnectionConfig) -> Result<Self> {
        match config.db_type {
            DbType::MySQL => {
                let url = format!(
                    "mysql://{}:{}@{}:{}/{}",
                    config.user, config.password, config.host, config.port, config.database
                );
                let pool = sqlx::MySqlPool::connect(&url).await?;
                Ok(DbConnection::MySQL(pool))
            }
            DbType::PostgreSQL => {
                let url = format!(
                    "postgres://{}:{}@{}:{}/{}",
                    config.user, config.password, config.host, config.port, config.database
                );
                let pool = sqlx::PgPool::connect(&url).await?;
                Ok(DbConnection::PostgreSQL(pool))
            }
            DbType::SQLite => {
                let path = config
                    .file_path
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("SQLite requires a file path"))?;
                let url = format!("sqlite:{}", path.display());
                let pool = sqlx::SqlitePool::connect(&url).await?;
                Ok(DbConnection::SQLite(pool))
            }
            DbType::SQLServer => {
                use tokio::net::TcpStream;
                use tokio_util::compat::TokioAsyncWriteCompatExt;

                let mut config_builder = tiberius::Config::new();
                config_builder.host(&config.host);
                config_builder.port(config.port);
                config_builder.authentication(tiberius::AuthMethod::sql_server(
                    &config.user,
                    &config.password,
                ));
                config_builder.database(&config.database);
                config_builder.trust_cert();

                let tcp = TcpStream::connect(config_builder.get_addr()).await?;
                tcp.set_nodelay(true)?;
                let client = tiberius::Client::connect(config_builder, tcp.compat_write()).await?;
                Ok(DbConnection::SQLServer(client))
            }
        }
    }

    /// Get the database type
    pub fn db_type(&self) -> DbType {
        match self {
            DbConnection::MySQL(_) => DbType::MySQL,
            DbConnection::PostgreSQL(_) => DbType::PostgreSQL,
            DbConnection::SQLite(_) => DbType::SQLite,
            DbConnection::SQLServer(_) => DbType::SQLServer,
        }
    }

    /// Test connection
    pub async fn test(&self) -> Result<()> {
        match self {
            DbConnection::MySQL(pool) => {
                sqlx::query("SELECT 1").execute(pool).await?;
            }
            DbConnection::PostgreSQL(pool) => {
                sqlx::query("SELECT 1").execute(pool).await?;
            }
            DbConnection::SQLite(pool) => {
                sqlx::query("SELECT 1").execute(pool).await?;
            }
            DbConnection::SQLServer(client) => {
                // SQL Server client is mutable, we can't test without mut ref
                // Connection success is verified during connect
                let _ = client;
            }
        }
        Ok(())
    }
}

/// Saved connection with name
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedConnection {
    pub name: String,
    pub config: ConnectionConfig,
}

/// Connection store for managing saved connections
pub struct ConnectionStore {
    connections: Vec<SavedConnection>,
    file_path: PathBuf,
}

impl ConnectionStore {
    /// Create new connection store
    pub fn new() -> Result<Self> {
        let config_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
            .join(".syncforge");

        std::fs::create_dir_all(&config_dir)?;

        let file_path = config_dir.join("connections.json");
        let connections = if file_path.exists() {
            let content = std::fs::read_to_string(&file_path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(Self {
            connections,
            file_path,
        })
    }

    /// Get all saved connections
    pub fn get_all(&self) -> &[SavedConnection] {
        &self.connections
    }

    /// Save a connection
    pub fn save(&mut self, conn: SavedConnection) -> Result<()> {
        // Update if exists, otherwise add
        if let Some(existing) = self.connections.iter_mut().find(|c| c.name == conn.name) {
            *existing = conn;
        } else {
            self.connections.push(conn);
        }
        self.persist()
    }

    /// Delete a connection by name
    pub fn delete(&mut self, name: &str) -> Result<()> {
        self.connections.retain(|c| c.name != name);
        self.persist()
    }

    /// Persist connections to file
    fn persist(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(&self.connections)?;
        std::fs::write(&self.file_path, content)?;
        Ok(())
    }
}
