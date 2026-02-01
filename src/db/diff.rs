use serde::{Deserialize, Serialize};

use super::{ColumnInfo, DbType, SchemaInfo, TableInfo};

/// Diff type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffType {
    Added,
    Removed,
    Modified,
}

/// Schema difference result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    pub diff_type: DiffType,
    pub table_name: String,
    pub detail: String,
    pub sql: String,
}

/// Compare two schemas and return differences
pub fn compare_schemas(source: &SchemaInfo, target: &SchemaInfo, target_db_type: DbType) -> Vec<DiffResult> {
    let mut results = Vec::new();

    let source_tables: std::collections::HashMap<&str, &TableInfo> =
        source.tables.iter().map(|t| (t.name.as_str(), t)).collect();
    let target_tables: std::collections::HashMap<&str, &TableInfo> =
        target.tables.iter().map(|t| (t.name.as_str(), t)).collect();

    // Find tables only in source (need to add to target)
    for (name, table) in &source_tables {
        if !target_tables.contains_key(name) {
            results.push(DiffResult {
                diff_type: DiffType::Added,
                table_name: name.to_string(),
                detail: "Table exists in source but not in target".to_string(),
                sql: format!("{};", table.create_sql),
            });
        }
    }

    // Find tables only in target (need to remove from target)
    for name in target_tables.keys() {
        if !source_tables.contains_key(name) {
            results.push(DiffResult {
                diff_type: DiffType::Removed,
                table_name: name.to_string(),
                detail: "Table exists in target but not in source".to_string(),
                sql: format!("DROP TABLE {};", target_db_type.quote_identifier(name)),
            });
        }
    }

    // Compare existing tables
    for (name, source_table) in &source_tables {
        if let Some(target_table) = target_tables.get(name) {
            let table_diffs = compare_tables(name, source_table, target_table, target_db_type);
            results.extend(table_diffs);
        }
    }

    // Sort results
    results.sort_by(|a, b| {
        let type_order = |t: &DiffType| match t {
            DiffType::Added => 0,
            DiffType::Modified => 1,
            DiffType::Removed => 2,
        };
        type_order(&a.diff_type)
            .cmp(&type_order(&b.diff_type))
            .then_with(|| a.table_name.cmp(&b.table_name))
    });

    results
}

/// Compare two tables
fn compare_tables(
    table_name: &str,
    source: &TableInfo,
    target: &TableInfo,
    db_type: DbType,
) -> Vec<DiffResult> {
    let mut results = Vec::new();

    let source_cols: std::collections::HashMap<&str, &ColumnInfo> =
        source.columns.iter().map(|c| (c.name.as_str(), c)).collect();
    let target_cols: std::collections::HashMap<&str, &ColumnInfo> =
        target.columns.iter().map(|c| (c.name.as_str(), c)).collect();

    // Find added columns
    for (col_name, col) in &source_cols {
        if !target_cols.contains_key(col_name) {
            let col_def = build_column_def(col);
            results.push(DiffResult {
                diff_type: DiffType::Modified,
                table_name: table_name.to_string(),
                detail: format!("Add column: {}", col_name),
                sql: format!(
                    "ALTER TABLE {} ADD COLUMN {} {};",
                    db_type.quote_identifier(table_name),
                    db_type.quote_identifier(col_name),
                    col_def
                ),
            });
        }
    }

    // Find removed columns
    for col_name in target_cols.keys() {
        if !source_cols.contains_key(col_name) {
            results.push(DiffResult {
                diff_type: DiffType::Modified,
                table_name: table_name.to_string(),
                detail: format!("Drop column: {}", col_name),
                sql: format!(
                    "ALTER TABLE {} DROP COLUMN {};",
                    db_type.quote_identifier(table_name),
                    db_type.quote_identifier(col_name)
                ),
            });
        }
    }

    // Find modified columns
    for (col_name, source_col) in &source_cols {
        if let Some(target_col) = target_cols.get(col_name) {
            if !columns_equal(source_col, target_col) {
                let col_def = build_column_def(source_col);
                results.push(DiffResult {
                    diff_type: DiffType::Modified,
                    table_name: table_name.to_string(),
                    detail: format!(
                        "Modify column: {} ({} -> {})",
                        col_name, target_col.data_type, source_col.data_type
                    ),
                    sql: format!(
                        "ALTER TABLE {} MODIFY COLUMN {} {};",
                        db_type.quote_identifier(table_name),
                        db_type.quote_identifier(col_name),
                        col_def
                    ),
                });
            }
        }
    }

    results
}

/// Build column definition string
fn build_column_def(col: &ColumnInfo) -> String {
    let mut def = col.data_type.clone();
    if !col.nullable {
        def.push_str(" NOT NULL");
    }
    if let Some(ref default) = col.default {
        if is_numeric_or_special(default) {
            def.push_str(&format!(" DEFAULT {}", default));
        } else {
            def.push_str(&format!(" DEFAULT '{}'", default));
        }
    }
    if !col.extra.is_empty() {
        def.push(' ');
        def.push_str(&col.extra);
    }
    def
}

/// Check if value is numeric or special (doesn't need quotes)
fn is_numeric_or_special(val: &str) -> bool {
    let upper = val.to_uppercase();
    let specials = ["NULL", "CURRENT_TIMESTAMP", "CURRENT_DATE", "CURRENT_TIME", "NOW()", "TRUE", "FALSE"];

    if specials.contains(&upper.as_str()) {
        return true;
    }
    if upper.ends_with("()") || upper.starts_with('(') {
        return true;
    }
    // Check if numeric
    val.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-')
}

/// Check if two columns are equal
fn columns_equal(a: &ColumnInfo, b: &ColumnInfo) -> bool {
    a.data_type == b.data_type
        && a.nullable == b.nullable
        && a.default == b.default
        && a.extra == b.extra
}
