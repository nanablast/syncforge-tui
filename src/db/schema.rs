use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::DbConnection;

/// Column information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub is_primary_key: bool,
    pub extra: String,
    pub position: u64,
}

/// Index information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub is_unique: bool,
}

/// Table information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    pub create_sql: String,
}

/// Database schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaInfo {
    pub database: String,
    pub tables: Vec<TableInfo>,
}

impl DbConnection {
    /// Get list of databases
    pub async fn get_databases(&self) -> Result<Vec<String>> {
        match self {
            DbConnection::MySQL(pool) => {
                let rows: Vec<(String,)> = sqlx::query_as(
                    "SELECT schema_name FROM information_schema.schemata
                     WHERE schema_name NOT IN ('information_schema', 'mysql', 'performance_schema', 'sys')"
                )
                .fetch_all(pool)
                .await?;
                Ok(rows.into_iter().map(|r| r.0).collect())
            }
            DbConnection::PostgreSQL(pool) => {
                let rows: Vec<(String,)> = sqlx::query_as(
                    "SELECT datname FROM pg_database WHERE datistemplate = false AND datname != 'postgres'"
                )
                .fetch_all(pool)
                .await?;
                Ok(rows.into_iter().map(|r| r.0).collect())
            }
            DbConnection::SQLite(_) => {
                Ok(vec!["main".to_string()])
            }
            DbConnection::SQLServer(_) => {
                // SQL Server needs mutable access for queries
                Ok(Vec::new())
            }
        }
    }

    /// Get list of tables
    pub async fn get_tables(&self) -> Result<Vec<String>> {
        match self {
            DbConnection::MySQL(pool) => {
                let rows: Vec<(String,)> = sqlx::query_as("SHOW TABLES")
                    .fetch_all(pool)
                    .await?;
                Ok(rows.into_iter().map(|r| r.0).collect())
            }
            DbConnection::PostgreSQL(pool) => {
                let rows: Vec<(String,)> = sqlx::query_as(
                    "SELECT tablename FROM pg_tables WHERE schemaname = 'public'"
                )
                .fetch_all(pool)
                .await?;
                Ok(rows.into_iter().map(|r| r.0).collect())
            }
            DbConnection::SQLite(pool) => {
                let rows: Vec<(String,)> = sqlx::query_as(
                    "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"
                )
                .fetch_all(pool)
                .await?;
                Ok(rows.into_iter().map(|r| r.0).collect())
            }
            DbConnection::SQLServer(_) => {
                Ok(Vec::new())
            }
        }
    }

    /// Get columns for a table
    pub async fn get_columns(&self, table_name: &str) -> Result<Vec<ColumnInfo>> {
        match self {
            DbConnection::MySQL(pool) => {
                let rows: Vec<(String, String, String, Option<String>, String, u64)> = sqlx::query_as(
                    r#"SELECT COLUMN_NAME, COLUMN_TYPE, IS_NULLABLE, COLUMN_DEFAULT, EXTRA, ORDINAL_POSITION
                       FROM INFORMATION_SCHEMA.COLUMNS
                       WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ?
                       ORDER BY ORDINAL_POSITION"#
                )
                .bind(table_name)
                .fetch_all(pool)
                .await?;

                // Get primary keys
                let pk_rows: Vec<(String,)> = sqlx::query_as(
                    r#"SELECT COLUMN_NAME FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE
                       WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? AND CONSTRAINT_NAME = 'PRIMARY'"#
                )
                .bind(table_name)
                .fetch_all(pool)
                .await?;
                let primary_keys: Vec<String> = pk_rows.into_iter().map(|r| r.0).collect();

                Ok(rows
                    .into_iter()
                    .map(|(name, data_type, nullable, default, extra, position)| ColumnInfo {
                        is_primary_key: primary_keys.contains(&name),
                        name,
                        data_type,
                        nullable: nullable == "YES",
                        default,
                        extra,
                        position,
                    })
                    .collect())
            }
            DbConnection::PostgreSQL(pool) => {
                let rows: Vec<(String, String, String, Option<String>, i64)> = sqlx::query_as(
                    r#"SELECT column_name, data_type, is_nullable, column_default, ordinal_position::bigint
                       FROM information_schema.columns
                       WHERE table_schema = 'public' AND table_name = $1
                       ORDER BY ordinal_position"#
                )
                .bind(table_name)
                .fetch_all(pool)
                .await?;

                // Get primary keys
                let pk_rows: Vec<(String,)> = sqlx::query_as(
                    r#"SELECT a.attname
                       FROM pg_index i
                       JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
                       WHERE i.indrelid = $1::regclass AND i.indisprimary"#
                )
                .bind(table_name)
                .fetch_all(pool)
                .await
                .unwrap_or_default();
                let primary_keys: Vec<String> = pk_rows.into_iter().map(|r| r.0).collect();

                Ok(rows
                    .into_iter()
                    .map(|(name, data_type, nullable, default, position)| ColumnInfo {
                        is_primary_key: primary_keys.contains(&name),
                        name,
                        data_type,
                        nullable: nullable == "YES",
                        default,
                        extra: String::new(),
                        position: position as u64,
                    })
                    .collect())
            }
            DbConnection::SQLite(pool) => {
                let rows: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as(
                    &format!("PRAGMA table_info('{}')", table_name)
                )
                .fetch_all(pool)
                .await?;

                Ok(rows
                    .into_iter()
                    .map(|(cid, name, data_type, notnull, default, pk)| ColumnInfo {
                        name,
                        data_type,
                        nullable: notnull == 0,
                        default,
                        is_primary_key: pk > 0,
                        extra: String::new(),
                        position: (cid + 1) as u64,
                    })
                    .collect())
            }
            DbConnection::SQLServer(_) => {
                Ok(Vec::new())
            }
        }
    }

    /// Get table info including columns and indexes
    pub async fn get_table_info(&self, table_name: &str) -> Result<TableInfo> {
        let columns = self.get_columns(table_name).await?;
        let indexes = self.get_indexes(table_name).await?;
        let create_sql = self.get_create_table_sql(table_name).await?;

        Ok(TableInfo {
            name: table_name.to_string(),
            columns,
            indexes,
            create_sql,
        })
    }

    /// Get indexes for a table
    pub async fn get_indexes(&self, table_name: &str) -> Result<Vec<IndexInfo>> {
        match self {
            DbConnection::MySQL(pool) => {
                let rows: Vec<(String, i32, String)> = sqlx::query_as(
                    &format!("SHOW INDEX FROM `{}`", table_name)
                )
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                // Group by index name
                let mut index_map: std::collections::HashMap<String, IndexInfo> = std::collections::HashMap::new();
                for (key_name, non_unique, column_name) in rows {
                    index_map
                        .entry(key_name.clone())
                        .or_insert_with(|| IndexInfo {
                            name: key_name,
                            columns: Vec::new(),
                            is_unique: non_unique == 0,
                        })
                        .columns
                        .push(column_name);
                }

                Ok(index_map.into_values().collect())
            }
            DbConnection::PostgreSQL(pool) => {
                let rows: Vec<(String, String)> = sqlx::query_as(
                    r#"SELECT indexname, indexdef FROM pg_indexes WHERE schemaname = 'public' AND tablename = $1"#
                )
                .bind(table_name)
                .fetch_all(pool)
                .await?;

                Ok(rows
                    .into_iter()
                    .map(|(name, def)| IndexInfo {
                        name,
                        columns: vec![def],
                        is_unique: false,
                    })
                    .collect())
            }
            DbConnection::SQLite(pool) => {
                let rows: Vec<(i32, String, i32, String, i32)> = sqlx::query_as(
                    &format!("PRAGMA index_list('{}')", table_name)
                )
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                Ok(rows
                    .into_iter()
                    .map(|(_, name, unique, _, _)| IndexInfo {
                        name,
                        columns: Vec::new(),
                        is_unique: unique == 1,
                    })
                    .collect())
            }
            DbConnection::SQLServer(_) => {
                Ok(Vec::new())
            }
        }
    }

    /// Get CREATE TABLE SQL
    pub async fn get_create_table_sql(&self, table_name: &str) -> Result<String> {
        match self {
            DbConnection::MySQL(pool) => {
                let row: (String, String) = sqlx::query_as(
                    &format!("SHOW CREATE TABLE `{}`", table_name)
                )
                .fetch_one(pool)
                .await?;
                Ok(row.1)
            }
            DbConnection::PostgreSQL(_) => {
                // PostgreSQL doesn't have SHOW CREATE TABLE, build from columns
                let columns = self.get_columns(table_name).await?;
                let col_defs: Vec<String> = columns
                    .iter()
                    .map(|c| {
                        let mut def = format!("{} {}", c.name, c.data_type);
                        if !c.nullable {
                            def.push_str(" NOT NULL");
                        }
                        if let Some(ref d) = c.default {
                            def.push_str(&format!(" DEFAULT {}", d));
                        }
                        def
                    })
                    .collect();
                Ok(format!("CREATE TABLE {} (\n  {}\n);", table_name, col_defs.join(",\n  ")))
            }
            DbConnection::SQLite(pool) => {
                let row: (String,) = sqlx::query_as(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name=?"
                )
                .bind(table_name)
                .fetch_one(pool)
                .await?;
                Ok(row.0)
            }
            DbConnection::SQLServer(_) => {
                // SQL Server doesn't have SHOW CREATE TABLE, build from columns
                let columns = self.get_columns(table_name).await?;
                let col_defs: Vec<String> = columns
                    .iter()
                    .map(|c| {
                        let mut def = format!("[{}] {}", c.name, c.data_type);
                        if !c.nullable {
                            def.push_str(" NOT NULL");
                        }
                        if let Some(ref d) = c.default {
                            def.push_str(&format!(" DEFAULT {}", d));
                        }
                        def
                    })
                    .collect();
                Ok(format!("CREATE TABLE [{}] (\n  {}\n);", table_name, col_defs.join(",\n  ")))
            }
        }
    }

    /// Get full schema info
    pub async fn get_schema(&self, database: &str) -> Result<SchemaInfo> {
        let table_names = self.get_tables().await?;
        let mut tables = Vec::new();

        for name in table_names {
            let table_info = self.get_table_info(&name).await?;
            tables.push(table_info);
        }

        Ok(SchemaInfo {
            database: database.to_string(),
            tables,
        })
    }
}
