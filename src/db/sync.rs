use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{DbConnection, DbType};

/// Data diff type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataDiffType {
    Insert,
    Update,
    Delete,
}

/// Data difference result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataDiffResult {
    pub diff_type: DataDiffType,
    pub table_name: String,
    pub primary_key: HashMap<String, String>,
    pub old_values: Option<HashMap<String, String>>,
    pub new_values: Option<HashMap<String, String>>,
    pub sql: String,
}

/// Table data info for sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDataInfo {
    pub table_name: String,
    pub columns: Vec<String>,
    pub primary_keys: Vec<String>,
    pub source_count: i64,
    pub target_count: i64,
    pub insert_count: i64,
    pub update_count: i64,
    pub delete_count: i64,
}

impl DbConnection {
    /// Get row count for a table
    pub async fn get_row_count(&self, table_name: &str) -> Result<i64> {
        let db_type = self.db_type();
        let query = format!(
            "SELECT COUNT(*) FROM {}",
            db_type.quote_identifier(table_name)
        );

        match self {
            DbConnection::MySQL(pool) => {
                let row: (i64,) = sqlx::query_as(&query).fetch_one(pool).await?;
                Ok(row.0)
            }
            DbConnection::PostgreSQL(pool) => {
                let row: (i64,) = sqlx::query_as(&query).fetch_one(pool).await?;
                Ok(row.0)
            }
            DbConnection::SQLite(pool) => {
                let row: (i64,) = sqlx::query_as(&query).fetch_one(pool).await?;
                Ok(row.0)
            }
            DbConnection::SQLServer(ref client) => {
                // SQL Server needs mutable access - return placeholder for now
                let _ = client;
                Ok(0)
            }
        }
    }

    /// Get primary key columns for a table
    pub async fn get_primary_keys(&self, table_name: &str, database: &str) -> Result<Vec<String>> {
        match self {
            DbConnection::MySQL(pool) => {
                let rows: Vec<(String,)> = sqlx::query_as(
                    r#"SELECT COLUMN_NAME FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE
                       WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND CONSTRAINT_NAME = 'PRIMARY'
                       ORDER BY ORDINAL_POSITION"#
                )
                .bind(database)
                .bind(table_name)
                .fetch_all(pool)
                .await?;
                Ok(rows.into_iter().map(|r| r.0).collect())
            }
            DbConnection::PostgreSQL(pool) => {
                let rows: Vec<(String,)> = sqlx::query_as(
                    r#"SELECT a.attname
                       FROM pg_index i
                       JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
                       WHERE i.indrelid = $1::regclass AND i.indisprimary"#
                )
                .bind(table_name)
                .fetch_all(pool)
                .await
                .unwrap_or_default();
                Ok(rows.into_iter().map(|r| r.0).collect())
            }
            DbConnection::SQLite(pool) => {
                let rows: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as(
                    &format!("PRAGMA table_info('{}')", table_name)
                )
                .fetch_all(pool)
                .await?;
                Ok(rows
                    .into_iter()
                    .filter(|r| r.5 > 0)
                    .map(|r| r.1)
                    .collect())
            }
            DbConnection::SQLServer(_) => {
                // SQL Server needs mutable access
                Ok(Vec::new())
            }
        }
    }

    /// Get all data from a table as a map keyed by primary key
    pub async fn get_table_data(
        &self,
        table_name: &str,
        columns: &[String],
        primary_keys: &[String],
    ) -> Result<HashMap<String, HashMap<String, String>>> {
        let db_type = self.db_type();
        let quoted_cols: Vec<String> = columns
            .iter()
            .map(|c| db_type.quote_identifier(c))
            .collect();

        let query = format!(
            "SELECT {} FROM {}",
            quoted_cols.join(", "),
            db_type.quote_identifier(table_name)
        );

        let mut result: HashMap<String, HashMap<String, String>> = HashMap::new();

        match self {
            DbConnection::MySQL(pool) => {
                let rows = sqlx::query(&query).fetch_all(pool).await?;
                for row in rows {
                    let mut row_data: HashMap<String, String> = HashMap::new();
                    for (i, col) in columns.iter().enumerate() {
                        let val: Option<String> = sqlx::Row::try_get(&row, i).ok();
                        row_data.insert(col.clone(), val.unwrap_or_else(|| "NULL".to_string()));
                    }
                    let pk_key = build_pk_key(&row_data, primary_keys);
                    result.insert(pk_key, row_data);
                }
            }
            DbConnection::PostgreSQL(pool) => {
                let rows = sqlx::query(&query).fetch_all(pool).await?;
                for row in rows {
                    let mut row_data: HashMap<String, String> = HashMap::new();
                    for (i, col) in columns.iter().enumerate() {
                        let val: Option<String> = sqlx::Row::try_get(&row, i).ok();
                        row_data.insert(col.clone(), val.unwrap_or_else(|| "NULL".to_string()));
                    }
                    let pk_key = build_pk_key(&row_data, primary_keys);
                    result.insert(pk_key, row_data);
                }
            }
            DbConnection::SQLite(pool) => {
                let rows = sqlx::query(&query).fetch_all(pool).await?;
                for row in rows {
                    let mut row_data: HashMap<String, String> = HashMap::new();
                    for (i, col) in columns.iter().enumerate() {
                        let val: Option<String> = sqlx::Row::try_get(&row, i).ok();
                        row_data.insert(col.clone(), val.unwrap_or_else(|| "NULL".to_string()));
                    }
                    let pk_key = build_pk_key(&row_data, primary_keys);
                    result.insert(pk_key, row_data);
                }
            }
            DbConnection::SQLServer(_) => {
                // SQL Server needs mutable access
            }
        }

        Ok(result)
    }

    /// Get paginated rows from a table for browsing
    pub async fn get_table_rows(
        &self,
        table_name: &str,
        columns: &[String],
        page: usize,
        page_size: usize,
    ) -> Result<Vec<Vec<String>>> {
        let db_type = self.db_type();
        let quoted_cols: Vec<String> = columns
            .iter()
            .map(|c| db_type.quote_identifier(c))
            .collect();

        let offset = (page.saturating_sub(1)) * page_size;

        let query = match db_type {
            DbType::SQLServer => format!(
                "SELECT {} FROM {} ORDER BY (SELECT NULL) OFFSET {} ROWS FETCH NEXT {} ROWS ONLY",
                quoted_cols.join(", "),
                db_type.quote_identifier(table_name),
                offset,
                page_size
            ),
            _ => format!(
                "SELECT {} FROM {} LIMIT {} OFFSET {}",
                quoted_cols.join(", "),
                db_type.quote_identifier(table_name),
                page_size,
                offset
            ),
        };

        let mut result: Vec<Vec<String>> = Vec::new();

        match self {
            DbConnection::MySQL(pool) => {
                let rows = sqlx::query(&query).fetch_all(pool).await?;
                for row in rows {
                    let mut row_data: Vec<String> = Vec::new();
                    for i in 0..columns.len() {
                        let val: Option<String> = sqlx::Row::try_get(&row, i).ok();
                        row_data.push(val.unwrap_or_else(|| "NULL".to_string()));
                    }
                    result.push(row_data);
                }
            }
            DbConnection::PostgreSQL(pool) => {
                let rows = sqlx::query(&query).fetch_all(pool).await?;
                for row in rows {
                    let mut row_data: Vec<String> = Vec::new();
                    for i in 0..columns.len() {
                        let val: Option<String> = sqlx::Row::try_get(&row, i).ok();
                        row_data.push(val.unwrap_or_else(|| "NULL".to_string()));
                    }
                    result.push(row_data);
                }
            }
            DbConnection::SQLite(pool) => {
                let rows = sqlx::query(&query).fetch_all(pool).await?;
                for row in rows {
                    let mut row_data: Vec<String> = Vec::new();
                    for i in 0..columns.len() {
                        let val: Option<String> = sqlx::Row::try_get(&row, i).ok();
                        row_data.push(val.unwrap_or_else(|| "NULL".to_string()));
                    }
                    result.push(row_data);
                }
            }
            DbConnection::SQLServer(_) => {
                // SQL Server needs mutable access
            }
        }

        Ok(result)
    }
}

/// Build a unique key from primary key values
fn build_pk_key(row: &HashMap<String, String>, primary_keys: &[String]) -> String {
    primary_keys
        .iter()
        .map(|pk| row.get(pk).map(|v| v.as_str()).unwrap_or(""))
        .collect::<Vec<_>>()
        .join("|")
}

/// Compare data between two connections
pub async fn compare_table_data(
    source: &DbConnection,
    target: &DbConnection,
    table_name: &str,
    database: &str,
) -> Result<Vec<DataDiffResult>> {
    let target_db_type = target.db_type();

    // Get primary keys from source
    let primary_keys = source.get_primary_keys(table_name, database).await?;
    if primary_keys.is_empty() {
        return Err(anyhow::anyhow!("Table {} has no primary key", table_name));
    }

    // Get columns from source
    let columns = source.get_columns(table_name).await?;
    let column_names: Vec<String> = columns.iter().map(|c| c.name.clone()).collect();

    // Get data from both
    let source_data = source
        .get_table_data(table_name, &column_names, &primary_keys)
        .await?;
    let target_data = target
        .get_table_data(table_name, &column_names, &primary_keys)
        .await?;

    let mut results = Vec::new();

    // Find inserts and updates
    for (pk_key, source_row) in &source_data {
        if let Some(target_row) = target_data.get(pk_key) {
            // Check for updates
            if source_row != target_row {
                let pk = extract_primary_key(source_row, &primary_keys);
                results.push(DataDiffResult {
                    diff_type: DataDiffType::Update,
                    table_name: table_name.to_string(),
                    primary_key: pk.clone(),
                    old_values: Some(target_row.clone()),
                    new_values: Some(source_row.clone()),
                    sql: generate_update_sql(target_db_type, table_name, source_row, &primary_keys),
                });
            }
        } else {
            // Insert
            let pk = extract_primary_key(source_row, &primary_keys);
            results.push(DataDiffResult {
                diff_type: DataDiffType::Insert,
                table_name: table_name.to_string(),
                primary_key: pk,
                old_values: None,
                new_values: Some(source_row.clone()),
                sql: generate_insert_sql(target_db_type, table_name, source_row, &column_names),
            });
        }
    }

    // Find deletes
    for (pk_key, target_row) in &target_data {
        if !source_data.contains_key(pk_key) {
            let pk = extract_primary_key(target_row, &primary_keys);
            results.push(DataDiffResult {
                diff_type: DataDiffType::Delete,
                table_name: table_name.to_string(),
                primary_key: pk.clone(),
                old_values: Some(target_row.clone()),
                new_values: None,
                sql: generate_delete_sql(target_db_type, table_name, &primary_keys, &pk),
            });
        }
    }

    Ok(results)
}

fn extract_primary_key(row: &HashMap<String, String>, primary_keys: &[String]) -> HashMap<String, String> {
    primary_keys
        .iter()
        .filter_map(|pk| row.get(pk).map(|v| (pk.clone(), v.clone())))
        .collect()
}

fn generate_insert_sql(
    db_type: DbType,
    table_name: &str,
    row: &HashMap<String, String>,
    columns: &[String],
) -> String {
    let cols: Vec<String> = columns
        .iter()
        .filter(|c| row.contains_key(*c))
        .map(|c| db_type.quote_identifier(c))
        .collect();

    let vals: Vec<String> = columns
        .iter()
        .filter_map(|c| row.get(c))
        .map(|v| escape_value(v))
        .collect();

    format!(
        "INSERT INTO {} ({}) VALUES ({});",
        db_type.quote_identifier(table_name),
        cols.join(", "),
        vals.join(", ")
    )
}

fn generate_update_sql(
    db_type: DbType,
    table_name: &str,
    row: &HashMap<String, String>,
    primary_keys: &[String],
) -> String {
    let sets: Vec<String> = row
        .iter()
        .filter(|(k, _)| !primary_keys.contains(k))
        .map(|(k, v)| format!("{} = {}", db_type.quote_identifier(k), escape_value(v)))
        .collect();

    let wheres: Vec<String> = primary_keys
        .iter()
        .filter_map(|pk| row.get(pk).map(|v| format!("{} = {}", db_type.quote_identifier(pk), escape_value(v))))
        .collect();

    format!(
        "UPDATE {} SET {} WHERE {};",
        db_type.quote_identifier(table_name),
        sets.join(", "),
        wheres.join(" AND ")
    )
}

fn generate_delete_sql(
    db_type: DbType,
    table_name: &str,
    primary_keys: &[String],
    pk_values: &HashMap<String, String>,
) -> String {
    let wheres: Vec<String> = primary_keys
        .iter()
        .filter_map(|pk| pk_values.get(pk).map(|v| format!("{} = {}", db_type.quote_identifier(pk), escape_value(v))))
        .collect();

    format!(
        "DELETE FROM {} WHERE {};",
        db_type.quote_identifier(table_name),
        wheres.join(" AND ")
    )
}

fn escape_value(val: &str) -> String {
    if val == "NULL" {
        return "NULL".to_string();
    }
    format!("'{}'", val.replace('\'', "''"))
}
