use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

/// Table browser view state
pub struct TableBrowserState {
    pub connection_name: String,
    pub tables: Vec<String>,
    pub selected_table_index: usize,
    pub table_list_state: TableState,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub selected_row_index: usize,
    pub data_table_state: TableState,
    pub page: usize,
    pub page_size: usize,
    pub total_count: usize,
    pub status: String,
    pub loading: bool,
    pub focus_left: bool,
    // Horizontal scroll for columns
    pub column_offset: usize,
    pub visible_columns: usize,
}

impl Default for TableBrowserState {
    fn default() -> Self {
        Self {
            connection_name: String::new(),
            tables: Vec::new(),
            selected_table_index: 0,
            table_list_state: TableState::default(),
            columns: Vec::new(),
            rows: Vec::new(),
            selected_row_index: 0,
            data_table_state: TableState::default(),
            page: 1,
            page_size: 50,
            total_count: 0,
            status: "Press [Ctrl+L] to load tables".to_string(),
            loading: false,
            focus_left: true,
            column_offset: 0,
            visible_columns: 5, // Default visible columns
        }
    }
}

impl TableBrowserState {
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

    pub fn next_row(&mut self) {
        if !self.rows.is_empty() {
            self.selected_row_index = (self.selected_row_index + 1) % self.rows.len();
        }
    }

    pub fn prev_row(&mut self) {
        if !self.rows.is_empty() {
            self.selected_row_index =
                (self.selected_row_index + self.rows.len() - 1) % self.rows.len();
        }
    }

    pub fn next_page(&mut self) {
        let max_page = (self.total_count + self.page_size - 1) / self.page_size;
        if self.page < max_page {
            self.page += 1;
        }
    }

    pub fn prev_page(&mut self) {
        if self.page > 1 {
            self.page -= 1;
        }
    }

    pub fn toggle_focus(&mut self) {
        self.focus_left = !self.focus_left;
    }

    pub fn total_pages(&self) -> usize {
        if self.total_count == 0 {
            1
        } else {
            (self.total_count + self.page_size - 1) / self.page_size
        }
    }

    pub fn scroll_columns_left(&mut self) {
        if self.column_offset > 0 {
            self.column_offset -= 1;
        }
    }

    pub fn scroll_columns_right(&mut self) {
        if !self.columns.is_empty() && self.column_offset + self.visible_columns < self.columns.len() {
            self.column_offset += 1;
        }
    }

    pub fn reset_column_scroll(&mut self) {
        self.column_offset = 0;
    }
}

/// Draw table browser view
pub fn draw_table_browser(f: &mut Frame, area: Rect, state: &mut TableBrowserState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Main content
            Constraint::Length(1), // Status
        ])
        .split(area);

    // Header
    let header = Paragraph::new(format!(
        "Connection: {} | Page {}/{} ({} rows)",
        if state.connection_name.is_empty() { "<not set>" } else { &state.connection_name },
        state.page,
        state.total_pages(),
        state.total_count,
    ))
    .block(Block::default().borders(Borders::ALL).title("Table Browser"));
    f.render_widget(header, chunks[0]);

    // Main content
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
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

    // Calculate visible column range
    let total_cols = state.columns.len();
    let start_col = state.column_offset.min(total_cols);
    let end_col = (state.column_offset + state.visible_columns).min(total_cols);

    // Data table - only show visible columns
    let visible_columns: Vec<String> = if total_cols > 0 {
        state.columns[start_col..end_col].to_vec()
    } else {
        Vec::new()
    };

    let header_cells: Vec<Cell> = visible_columns
        .iter()
        .map(|h| Cell::from(h.clone()).style(Style::default().fg(Color::Yellow)))
        .collect();
    let data_header = Row::new(header_cells).height(1);

    let data_rows: Vec<Row> = state
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let style = if i == state.selected_row_index {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            // Only show visible columns
            let row_end = end_col.min(row.len());
            let row_start = start_col.min(row.len());
            let cells: Vec<Cell> = row[row_start..row_end]
                .iter()
                .map(|c| Cell::from(c.clone()))
                .collect();
            Row::new(cells).style(style)
        })
        .collect();

    let col_widths: Vec<Constraint> = visible_columns
        .iter()
        .map(|_| Constraint::Min(15))
        .collect();

    let (data_border_style, data_title_style) = if !state.focus_left {
        (
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )
    } else {
        (Style::default().fg(Color::DarkGray), Style::default())
    };

    let selected_table_name = state
        .tables
        .get(state.selected_table_index)
        .cloned()
        .unwrap_or_else(|| "No table selected".to_string());

    // Show column range indicator in title
    let col_indicator = if total_cols > 0 {
        format!(" [Col {}-{}/{}]", start_col + 1, end_col, total_cols)
    } else {
        String::new()
    };

    let data_table = Table::new(data_rows, col_widths)
        .header(data_header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("{}{}", selected_table_name, col_indicator))
                .title_style(data_title_style)
                .border_style(data_border_style),
        );

    state.data_table_state.select(Some(state.selected_row_index));
    f.render_stateful_widget(data_table, main_chunks[1], &mut state.data_table_state);

    // Status / hints
    let status = Paragraph::new(format!(
        "{} | [Ctrl+L]Load [Enter]View [←→]Page [Shift+←→]Cols [Tab]Focus [↑↓]Nav [Esc]Quit",
        state.status
    ))
    .style(Style::default().fg(Color::Cyan));
    f.render_widget(status, chunks[2]);
}
