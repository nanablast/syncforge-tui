use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::db::{DataDiffResult, DataDiffType};

/// Data sync view state
pub struct DataSyncState {
    pub source_name: String,
    pub target_name: String,
    pub tables: Vec<String>,
    pub selected_table_index: usize,
    pub table_list_state: TableState,
    pub diffs: Vec<DataDiffResult>,
    pub selected_diff_index: usize,
    pub diff_table_state: TableState,
    pub status: String,
    pub loading: bool,
    pub focus_left: bool, // true = table list, false = diff list
}

impl Default for DataSyncState {
    fn default() -> Self {
        Self {
            source_name: String::new(),
            target_name: String::new(),
            tables: Vec::new(),
            selected_table_index: 0,
            table_list_state: TableState::default(),
            diffs: Vec::new(),
            selected_diff_index: 0,
            diff_table_state: TableState::default(),
            status: "Press [Ctrl+L] to load tables".to_string(),
            loading: false,
            focus_left: true,
        }
    }
}

impl DataSyncState {
    pub fn next_table(&mut self) {
        if !self.tables.is_empty() {
            self.selected_table_index = (self.selected_table_index + 1) % self.tables.len();
        }
    }

    pub fn prev_table(&mut self) {
        if !self.tables.is_empty() {
            self.selected_table_index =
                (self.selected_table_index + self.tables.len() - 1) % self.tables.len();
        }
    }

    pub fn next_diff(&mut self) {
        if !self.diffs.is_empty() {
            self.selected_diff_index = (self.selected_diff_index + 1) % self.diffs.len();
        }
    }

    pub fn prev_diff(&mut self) {
        if !self.diffs.is_empty() {
            self.selected_diff_index =
                (self.selected_diff_index + self.diffs.len() - 1) % self.diffs.len();
        }
    }

    pub fn toggle_focus(&mut self) {
        self.focus_left = !self.focus_left;
    }
}

/// Draw data sync view
pub fn draw_data_sync(f: &mut Frame, area: Rect, state: &mut DataSyncState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Main content
            Constraint::Length(5), // SQL Preview
            Constraint::Length(1), // Status
        ])
        .split(area);

    // Header
    let header = Paragraph::new(format!(
        "Source: {}  ->  Target: {}",
        if state.source_name.is_empty() { "<not set>" } else { &state.source_name },
        if state.target_name.is_empty() { "<not set>" } else { &state.target_name },
    ))
    .block(Block::default().borders(Borders::ALL).title("Data Sync"));
    f.render_widget(header, chunks[0]);

    // Main content: table list + diff list
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[1]);

    // Table list
    let table_rows: Vec<Row> = state
        .tables
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let style = if i == state.selected_table_index {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Row::new(vec![Cell::from(name.clone())]).style(style)
        })
        .collect();

    let (table_border_style, table_title_style) = if state.focus_left {
        (
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )
    } else {
        (Style::default().fg(Color::DarkGray), Style::default())
    };

    let table_list = Table::new(table_rows, [Constraint::Min(10)])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Tables ({})", state.tables.len()))
                .title_style(table_title_style)
                .border_style(table_border_style),
        );

    state.table_list_state.select(Some(state.selected_table_index));
    f.render_stateful_widget(table_list, main_chunks[0], &mut state.table_list_state);

    // Diff list
    let diff_header_cells = ["Type", "Primary Key", "Changes"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
    let diff_header = Row::new(diff_header_cells).height(1);

    let diff_rows: Vec<Row> = state
        .diffs
        .iter()
        .enumerate()
        .map(|(i, diff)| {
            let (type_style, type_str) = match diff.diff_type {
                DataDiffType::Insert => (Style::default().fg(Color::Green), "INSERT"),
                DataDiffType::Update => (Style::default().fg(Color::Yellow), "UPDATE"),
                DataDiffType::Delete => (Style::default().fg(Color::Red), "DELETE"),
            };

            let pk_str: String = diff
                .primary_key
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(", ");

            let change_str = match diff.diff_type {
                DataDiffType::Insert => "New row".to_string(),
                DataDiffType::Delete => "Remove row".to_string(),
                DataDiffType::Update => {
                    let changed: Vec<String> = diff
                        .new_values
                        .as_ref()
                        .map(|nv| {
                            nv.keys()
                                .filter(|k| {
                                    diff.old_values
                                        .as_ref()
                                        .map(|ov| ov.get(*k) != nv.get(*k))
                                        .unwrap_or(true)
                                })
                                .cloned()
                                .collect()
                        })
                        .unwrap_or_default();
                    if changed.is_empty() {
                        "Modified".to_string()
                    } else {
                        changed.join(", ")
                    }
                }
            };

            let style = if i == state.selected_diff_index {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(type_str).style(type_style),
                Cell::from(pk_str),
                Cell::from(change_str),
            ])
            .style(style)
        })
        .collect();

    let (diff_border_style, diff_title_style) = if !state.focus_left {
        (
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )
    } else {
        (Style::default().fg(Color::DarkGray), Style::default())
    };

    let diff_table = Table::new(
        diff_rows,
        [
            Constraint::Length(8),
            Constraint::Length(20),
            Constraint::Min(20),
        ],
    )
    .header(diff_header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Data Differences ({})", state.diffs.len()))
            .title_style(diff_title_style)
            .border_style(diff_border_style),
    );

    state.diff_table_state.select(Some(state.selected_diff_index));
    f.render_stateful_widget(diff_table, main_chunks[1], &mut state.diff_table_state);

    // SQL Preview
    let sql = state
        .diffs
        .get(state.selected_diff_index)
        .map(|d| d.sql.clone())
        .unwrap_or_else(|| "No difference selected".to_string());

    let sql_preview = Paragraph::new(sql)
        .block(Block::default().borders(Borders::ALL).title("SQL Preview"))
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(sql_preview, chunks[2]);

    // Status / hints
    let status = Paragraph::new(format!(
        "{} | [Ctrl+L]Load [Enter]Compare [Tab]Focus [↑↓]Navigate [Esc]Quit",
        state.status
    ))
    .style(Style::default().fg(Color::Cyan));
    f.render_widget(status, chunks[3]);
}
