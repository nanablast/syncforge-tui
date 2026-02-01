use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::db::DiffResult;

/// Schema diff view state
pub struct SchemaDiffState {
    pub source_name: String,
    pub target_name: String,
    pub diffs: Vec<DiffResult>,
    pub selected_index: usize,
    pub table_state: TableState,
    pub status: String,
    pub loading: bool,
}

impl Default for SchemaDiffState {
    fn default() -> Self {
        Self {
            source_name: String::new(),
            target_name: String::new(),
            diffs: Vec::new(),
            selected_index: 0,
            table_state: TableState::default(),
            status: "Press [Enter] to compare schemas".to_string(),
            loading: false,
        }
    }
}

impl SchemaDiffState {
    pub fn next(&mut self) {
        if !self.diffs.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.diffs.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.diffs.is_empty() {
            self.selected_index = (self.selected_index + self.diffs.len() - 1) % self.diffs.len();
        }
    }
}

/// Draw schema diff view
pub fn draw_schema_diff(f: &mut Frame, area: Rect, state: &mut SchemaDiffState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Table
            Constraint::Length(6), // SQL Preview
            Constraint::Length(1), // Status
        ])
        .split(area);

    // Header
    let header = Paragraph::new(format!(
        "Source: {}  ->  Target: {}",
        if state.source_name.is_empty() { "<not set>" } else { &state.source_name },
        if state.target_name.is_empty() { "<not set>" } else { &state.target_name },
    ))
    .block(Block::default().borders(Borders::ALL).title("Schema Diff"));
    f.render_widget(header, chunks[0]);

    // Diff table
    let header_cells = ["Type", "Table", "Details"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
    let header_row = Row::new(header_cells).height(1);

    let rows: Vec<Row> = state
        .diffs
        .iter()
        .enumerate()
        .map(|(i, diff)| {
            let (type_style, type_str) = match diff.diff_type {
                crate::db::DiffType::Added => (Style::default().fg(Color::Green), "ADD"),
                crate::db::DiffType::Removed => (Style::default().fg(Color::Red), "DROP"),
                crate::db::DiffType::Modified => (Style::default().fg(Color::Yellow), "MODIFY"),
            };

            let style = if i == state.selected_index {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(type_str).style(type_style),
                Cell::from(diff.table_name.clone()),
                Cell::from(diff.detail.clone()),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(20),
            Constraint::Min(30),
        ],
    )
    .header(header_row)
    .block(Block::default().borders(Borders::ALL).title(format!(
        "Differences ({})",
        state.diffs.len()
    )));

    state.table_state.select(Some(state.selected_index));
    f.render_stateful_widget(table, chunks[1], &mut state.table_state);

    // SQL Preview
    let sql = state
        .diffs
        .get(state.selected_index)
        .map(|d| d.sql.clone())
        .unwrap_or_else(|| "No difference selected".to_string());

    let sql_preview = Paragraph::new(sql)
        .block(Block::default().borders(Borders::ALL).title("SQL Preview"))
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(sql_preview, chunks[2]);

    // Status / hints
    let status = Paragraph::new(format!(
        "{} | [Enter]Compare [↑↓]Navigate [Esc]Quit",
        state.status
    ))
    .style(Style::default().fg(Color::Cyan));
    f.render_widget(status, chunks[3]);
}
