use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::db::{ConnectionConfig, DbType, SavedConnection};

/// Connection form state
pub struct ConnectionFormState {
    pub saved_connections: Vec<SavedConnection>,
    pub selected_index: usize,
    pub list_state: ListState,

    // Form fields
    pub name: String,
    pub db_type: DbType,
    pub host: String,
    pub port: String,
    pub user: String,
    pub password: String,
    pub database: String,
    pub file_path: String,

    // Current focused field
    pub focused_field: usize,
    pub editing: bool,

    // Status message
    pub status: String,
    pub status_error: bool,
}

impl Default for ConnectionFormState {
    fn default() -> Self {
        Self {
            saved_connections: Vec::new(),
            selected_index: 0,
            list_state: ListState::default(),
            name: String::new(),
            db_type: DbType::MySQL,
            host: "localhost".to_string(),
            port: "3306".to_string(),
            user: "root".to_string(),
            password: String::new(),
            database: String::new(),
            file_path: String::new(),
            focused_field: 0,
            editing: false,
            status: "Set source (F5) and target (F6) before using other features".to_string(),
            status_error: false,
        }
    }
}

impl ConnectionFormState {
    pub fn field_count(&self) -> usize {
        match self.db_type {
            DbType::SQLite => 3, // name, db_type, file_path
            _ => 7,             // name, db_type, host, port, user, password, database
        }
    }

    pub fn next_field(&mut self) {
        self.focused_field = (self.focused_field + 1) % self.field_count();
    }

    pub fn prev_field(&mut self) {
        self.focused_field = (self.focused_field + self.field_count() - 1) % self.field_count();
    }

    pub fn next_db_type(&mut self) {
        self.db_type = match self.db_type {
            DbType::MySQL => DbType::PostgreSQL,
            DbType::PostgreSQL => DbType::SQLite,
            DbType::SQLite => DbType::SQLServer,
            DbType::SQLServer => DbType::MySQL,
        };
        self.port = ConnectionConfig::default_port(self.db_type).to_string();
    }

    pub fn prev_db_type(&mut self) {
        self.db_type = match self.db_type {
            DbType::MySQL => DbType::SQLServer,
            DbType::PostgreSQL => DbType::MySQL,
            DbType::SQLite => DbType::PostgreSQL,
            DbType::SQLServer => DbType::SQLite,
        };
        self.port = ConnectionConfig::default_port(self.db_type).to_string();
    }

    pub fn to_config(&self) -> ConnectionConfig {
        ConnectionConfig {
            db_type: self.db_type,
            host: self.host.clone(),
            port: self.port.parse().unwrap_or(3306),
            user: self.user.clone(),
            password: self.password.clone(),
            database: self.database.clone(),
            file_path: if self.file_path.is_empty() {
                None
            } else {
                Some(self.file_path.clone().into())
            },
        }
    }

    pub fn load_connection(&mut self, conn: &SavedConnection) {
        self.name = conn.name.clone();
        self.db_type = conn.config.db_type;
        self.host = conn.config.host.clone();
        self.port = conn.config.port.to_string();
        self.user = conn.config.user.clone();
        self.password = conn.config.password.clone();
        self.database = conn.config.database.clone();
        self.file_path = conn
            .config
            .file_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
    }

    pub fn clear(&mut self) {
        self.name.clear();
        self.db_type = DbType::MySQL;
        self.host = "localhost".to_string();
        self.port = "3306".to_string();
        self.user = "root".to_string();
        self.password.clear();
        self.database.clear();
        self.file_path.clear();
        self.focused_field = 0;
    }

    pub fn set_status(&mut self, msg: &str, is_error: bool) {
        self.status = msg.to_string();
        self.status_error = is_error;
    }
}

/// Draw connection form
pub fn draw_connection_form(
    f: &mut Frame,
    area: Rect,
    state: &mut ConnectionFormState,
    source_name: &str,
    target_name: &str,
) {
    // Main layout: content + hints at bottom
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),   // Main content
            Constraint::Length(1), // Hints
        ])
        .split(area);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(main_chunks[0]);

    // Left: saved connections list with "New" option at top
    let mut items: Vec<ListItem> = vec![
        ListItem::new(Line::from(vec![
            Span::styled("[+]", Style::default().fg(Color::Green)),
            Span::raw(" "),
            Span::styled("New Connection", Style::default().fg(Color::Green)),
        ]))
    ];

    items.extend(state.saved_connections.iter().map(|c| {
        let db_type = format!("[{:?}]", c.config.db_type);
        ListItem::new(Line::from(vec![
            Span::styled(db_type, Style::default().fg(Color::Cyan)),
            Span::raw(" "),
            Span::raw(&c.name),
        ]))
    }));

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Saved Connections"),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    state.list_state.select(Some(state.selected_index));
    f.render_stateful_widget(list, content_chunks[0], &mut state.list_state);

    // Right: connection form
    let form_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1), // source/target status
            Constraint::Length(2), // name
            Constraint::Length(2), // db_type
            Constraint::Length(2), // host/file_path
            Constraint::Length(2), // port
            Constraint::Length(2), // user
            Constraint::Length(2), // password
            Constraint::Length(2), // database
            Constraint::Length(2), // status
            Constraint::Min(0),    // spacer
        ])
        .split(content_chunks[1]);

    let form_block = Block::default()
        .borders(Borders::ALL)
        .title("Connection Details");
    f.render_widget(form_block, content_chunks[1]);

    // Source/Target status line
    let source_display = if source_name.is_empty() { "<not set>" } else { source_name };
    let target_display = if target_name.is_empty() { "<not set>" } else { target_name };
    let conn_status = Paragraph::new(format!(
        "Source: {}  |  Target: {}",
        source_display, target_display
    ))
    .style(Style::default().fg(Color::Magenta));
    f.render_widget(conn_status, form_chunks[0]);

    let field_style = |idx: usize, focused: usize| {
        if idx == focused {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        }
    };

    // Name field
    let name_p = Paragraph::new(format!("Name: {}", state.name))
        .style(field_style(0, state.focused_field));
    f.render_widget(name_p, form_chunks[1]);

    // DB Type field
    let db_type_p = Paragraph::new(format!("Type: {:?} (←→ or Space to switch)", state.db_type))
        .style(field_style(1, state.focused_field));
    f.render_widget(db_type_p, form_chunks[2]);

    match state.db_type {
        DbType::SQLite => {
            // File path
            let file_p = Paragraph::new(format!("File Path: {}", state.file_path))
                .style(field_style(2, state.focused_field));
            f.render_widget(file_p, form_chunks[3]);
        }
        _ => {
            // Host
            let host_p = Paragraph::new(format!("Host: {}", state.host))
                .style(field_style(2, state.focused_field));
            f.render_widget(host_p, form_chunks[3]);

            // Port
            let port_p = Paragraph::new(format!("Port: {}", state.port))
                .style(field_style(3, state.focused_field));
            f.render_widget(port_p, form_chunks[4]);

            // User
            let user_p = Paragraph::new(format!("User: {}", state.user))
                .style(field_style(4, state.focused_field));
            f.render_widget(user_p, form_chunks[5]);

            // Password
            let pwd_display = "●".repeat(state.password.len());
            let pwd_p = Paragraph::new(format!("Password: {}", pwd_display))
                .style(field_style(5, state.focused_field));
            f.render_widget(pwd_p, form_chunks[6]);

            // Database
            let db_p = Paragraph::new(format!("Database: {}", state.database))
                .style(field_style(6, state.focused_field));
            f.render_widget(db_p, form_chunks[7]);
        }
    }

    // Status
    let status_style = if state.status_error {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Green)
    };
    let status_p = Paragraph::new(state.status.clone()).style(status_style);
    f.render_widget(status_p, form_chunks[8]);

    // Hints at bottom of page
    let hints = Paragraph::new(
        "[↑↓]Select [Tab]Field [Enter]Test [Ctrl+S]Save [Ctrl+D]Delete [Ctrl+N]New [F5]Source [F6]Target [Esc]Quit"
    )
    .style(Style::default().fg(Color::Cyan));
    f.render_widget(hints, main_chunks[1]);
}
