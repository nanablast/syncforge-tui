mod db;
mod ui;

use std::io;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    Terminal,
};

use db::{ConnectionStore, DbConnection, SavedConnection};
use ui::{
    draw_tabs, Tab,
    ConnectionFormState, draw_connection_form,
    SchemaDiffState, draw_schema_diff,
    DataSyncState, draw_data_sync,
    TableBrowserState, draw_table_browser,
    Spinner, draw_spinner,
};

/// Background task result
enum TaskResult {
    ConnectionTest(Result<(), String>),
    SchemaCompare(Result<Vec<db::DiffResult>, String>),
    LoadTables(Result<Vec<String>, String>),
    CompareData(Result<Vec<db::DataDiffResult>, String>),
    LoadTableData {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
        total_count: usize,
    },
    LoadTableDataError(String),
}

/// Application state
struct App {
    running: bool,
    current_tab: Tab,
    connection_store: ConnectionStore,

    // Tab states
    connection_state: ConnectionFormState,
    schema_diff_state: SchemaDiffState,
    data_sync_state: DataSyncState,
    table_browser_state: TableBrowserState,

    // Active connections
    source_connection: Option<SavedConnection>,
    target_connection: Option<SavedConnection>,

    // Spinner for async operations
    spinner: Spinner,

    // Background task receiver
    task_rx: Option<tokio::sync::mpsc::Receiver<TaskResult>>,
}

impl App {
    fn new() -> Result<Self> {
        let connection_store = ConnectionStore::new()?;
        let saved = connection_store.get_all().to_vec();

        let mut connection_state = ConnectionFormState::default();
        connection_state.saved_connections = saved;

        Ok(Self {
            running: true,
            current_tab: Tab::Connection,
            connection_store,
            connection_state,
            schema_diff_state: SchemaDiffState::default(),
            data_sync_state: DataSyncState::default(),
            table_browser_state: TableBrowserState::default(),
            source_connection: None,
            target_connection: None,
            spinner: Spinner::default(),
            task_rx: None,
        })
    }

    fn refresh_connections(&mut self) {
        self.connection_state.saved_connections = self.connection_store.get_all().to_vec();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new()?;

    // Main loop
    let res = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        if !app.running {
            break;
        }

        // Update spinner animation
        app.spinner.tick();

        // Check for background task results
        if let Some(ref mut rx) = app.task_rx {
            match rx.try_recv() {
                Ok(result) => {
                    handle_task_result(app, result);
                    app.spinner.stop();
                    app.task_rx = None;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                    // Task still running
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    app.spinner.stop();
                    app.task_rx = None;
                }
            }
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Tabs
                    Constraint::Min(0),     // Main content
                    Constraint::Length(1),  // Spinner (only when active)
                ])
                .split(f.area());

            draw_tabs(f, chunks[0], app.current_tab);

            // Adjust main area based on spinner
            let main_area = if app.spinner.active {
                chunks[1]
            } else {
                // Merge main and spinner area when spinner is inactive
                ratatui::layout::Rect {
                    x: chunks[1].x,
                    y: chunks[1].y,
                    width: chunks[1].width,
                    height: chunks[1].height + chunks[2].height,
                }
            };

            match app.current_tab {
                Tab::Connection => {
                    let source_name = app.source_connection.as_ref().map(|c| c.name.as_str()).unwrap_or("");
                    let target_name = app.target_connection.as_ref().map(|c| c.name.as_str()).unwrap_or("");
                    draw_connection_form(f, main_area, &mut app.connection_state, source_name, target_name);
                }
                Tab::SchemaDiff => draw_schema_diff(f, main_area, &mut app.schema_diff_state),
                Tab::DataSync => draw_data_sync(f, main_area, &mut app.data_sync_state),
                Tab::TableBrowser => draw_table_browser(f, main_area, &mut app.table_browser_state),
            }

            // Draw spinner at the bottom when active
            if app.spinner.active {
                draw_spinner(f, chunks[2], &app.spinner);
            }
        })?;

        // Use shorter poll time when spinner is active for smoother animation
        let poll_time = if app.spinner.active {
            std::time::Duration::from_millis(50)
        } else {
            std::time::Duration::from_millis(100)
        };

        if event::poll(poll_time)? {
            if let Event::Key(key) = event::read()? {
                // Ignore input while task is running
                if app.task_rx.is_some() {
                    continue;
                }

                // Global: Ctrl+C or Ctrl+Q to quit
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match key.code {
                        KeyCode::Char('c') | KeyCode::Char('q') => {
                            app.running = false;
                            continue;
                        }
                        _ => {}
                    }
                }

                // Global: Esc to quit (when not editing)
                if key.code == KeyCode::Esc {
                    app.running = false;
                    continue;
                }

                // Tab switching with F1-F4 (F2-F4 require both connections set)
                match key.code {
                    KeyCode::F(1) => app.current_tab = Tab::Connection,
                    KeyCode::F(2) => {
                        if app.source_connection.is_some() && app.target_connection.is_some() {
                            app.current_tab = Tab::SchemaDiff;
                        } else {
                            app.connection_state.set_status("Set source (F5) and target (F6) first", true);
                        }
                    }
                    KeyCode::F(3) => {
                        if app.source_connection.is_some() && app.target_connection.is_some() {
                            app.current_tab = Tab::DataSync;
                        } else {
                            app.connection_state.set_status("Set source (F5) and target (F6) first", true);
                        }
                    }
                    KeyCode::F(4) => {
                        if app.target_connection.is_some() {
                            app.current_tab = Tab::TableBrowser;
                        } else {
                            app.connection_state.set_status("Set target (F6) first", true);
                        }
                    }
                    _ => {
                        // Pass full KeyEvent to handlers
                        handle_tab_input(app, key).await;
                    }
                }
            }
        }
    }

    Ok(())
}

fn handle_task_result(app: &mut App, result: TaskResult) {
    match result {
        TaskResult::ConnectionTest(res) => {
            match res {
                Ok(()) => app.connection_state.set_status("Connection successful!", false),
                Err(e) => app.connection_state.set_status(&format!("Connection failed: {}", e), true),
            }
        }
        TaskResult::SchemaCompare(res) => {
            match res {
                Ok(diffs) => {
                    let count = diffs.len();
                    app.schema_diff_state.diffs = diffs;
                    app.schema_diff_state.status = format!("Found {} differences", count);
                }
                Err(e) => {
                    app.schema_diff_state.status = format!("Error: {}", e);
                }
            }
            app.schema_diff_state.loading = false;
        }
        TaskResult::LoadTables(res) => {
            match res {
                Ok(tables) => {
                    let count = tables.len();
                    match app.current_tab {
                        Tab::DataSync => {
                            app.data_sync_state.tables = tables;
                            app.data_sync_state.status = format!("Loaded {} tables", count);
                        }
                        Tab::TableBrowser => {
                            app.table_browser_state.tables = tables;
                            app.table_browser_state.status = format!("Loaded {} tables", count);
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    match app.current_tab {
                        Tab::DataSync => {
                            app.data_sync_state.status = format!("Error: {}", e);
                        }
                        Tab::TableBrowser => {
                            app.table_browser_state.status = format!("Error: {}", e);
                        }
                        _ => {}
                    }
                }
            }
        }
        TaskResult::CompareData(res) => {
            match res {
                Ok(diffs) => {
                    let count = diffs.len();
                    app.data_sync_state.diffs = diffs;
                    app.data_sync_state.status = format!("Found {} differences", count);
                }
                Err(e) => {
                    app.data_sync_state.status = format!("Error: {}", e);
                }
            }
        }
        TaskResult::LoadTableData { columns, rows, total_count } => {
            // Reset column scroll if columns changed (new table)
            if app.table_browser_state.columns != columns {
                app.table_browser_state.reset_column_scroll();
            }
            app.table_browser_state.columns = columns;
            app.table_browser_state.rows = rows;
            app.table_browser_state.total_count = total_count;
            app.table_browser_state.selected_row_index = 0;
            app.table_browser_state.status = format!(
                "Page {}/{} ({} rows)",
                app.table_browser_state.page,
                app.table_browser_state.total_pages(),
                total_count
            );
        }
        TaskResult::LoadTableDataError(e) => {
            app.table_browser_state.status = format!("Error: {}", e);
        }
    }
}

async fn handle_tab_input(app: &mut App, key: KeyEvent) {
    match app.current_tab {
        Tab::Connection => handle_connection_input(app, key).await,
        Tab::SchemaDiff => handle_schema_diff_input(app, key).await,
        Tab::DataSync => handle_data_sync_input(app, key).await,
        Tab::TableBrowser => handle_table_browser_input(app, key).await,
    }
}

async fn handle_connection_input(app: &mut App, key: KeyEvent) {
    let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        // Navigation: Up/Down to select saved connections
        // Index 0 = "New connection", Index 1+ = saved connections
        KeyCode::Up => {
            let state = &mut app.connection_state;
            let max_index = state.saved_connections.len(); // 0 is "new", 1..=len are saved
            if state.selected_index > 0 {
                state.selected_index -= 1;
                if state.selected_index == 0 {
                    state.clear();
                } else {
                    let conn = state.saved_connections[state.selected_index - 1].clone();
                    state.load_connection(&conn);
                }
            }
        }
        KeyCode::Down => {
            let state = &mut app.connection_state;
            let max_index = state.saved_connections.len(); // 0 is "new", 1..=len are saved
            if state.selected_index < max_index {
                state.selected_index += 1;
                if state.selected_index == 0 {
                    state.clear();
                } else {
                    let conn = state.saved_connections[state.selected_index - 1].clone();
                    state.load_connection(&conn);
                }
            }
        }

        // Tab: next field
        KeyCode::Tab => {
            app.connection_state.next_field();
        }
        KeyCode::BackTab => app.connection_state.prev_field(),

        // Space or Left/Right on DB type field: cycle DB type
        KeyCode::Char(' ') | KeyCode::Right if app.connection_state.focused_field == 1 => {
            app.connection_state.next_db_type();
        }
        KeyCode::Left if app.connection_state.focused_field == 1 => {
            app.connection_state.prev_db_type();
        }

        // Enter: test connection
        KeyCode::Enter => {
            app.spinner.start("Testing connection...");
            app.connection_state.set_status("Testing connection...", false);
            let config = app.connection_state.to_config();

            let (tx, rx) = tokio::sync::mpsc::channel(1);
            app.task_rx = Some(rx);

            tokio::spawn(async move {
                let result = match DbConnection::connect(&config).await {
                    Ok(conn) => {
                        match conn.test().await {
                            Ok(()) => Ok(()),
                            Err(e) => Err(e.to_string()),
                        }
                    }
                    Err(e) => Err(e.to_string()),
                };
                let _ = tx.send(TaskResult::ConnectionTest(result)).await;
            });
        }

        // Ctrl+S: save connection
        KeyCode::Char('s') if has_ctrl => {
            if app.connection_state.name.is_empty() {
                app.connection_state.set_status("Please enter a connection name", true);
            } else {
                let conn = SavedConnection {
                    name: app.connection_state.name.clone(),
                    config: app.connection_state.to_config(),
                };
                if let Err(e) = app.connection_store.save(conn) {
                    app.connection_state.set_status(&format!("Save failed: {}", e), true);
                } else {
                    app.connection_state.set_status("Connection saved!", false);
                    app.refresh_connections();
                }
            }
        }

        // Ctrl+D: delete connection
        KeyCode::Char('d') if has_ctrl => {
            let name = app.connection_state.name.clone();
            if !name.is_empty() {
                if let Err(e) = app.connection_store.delete(&name) {
                    app.connection_state.set_status(&format!("Delete failed: {}", e), true);
                } else {
                    app.connection_state.set_status("Connection deleted!", false);
                    app.connection_state.clear();
                    app.refresh_connections();
                    app.connection_state.selected_index = 0;
                }
            }
        }

        // Ctrl+N: new connection
        KeyCode::Char('n') if has_ctrl => {
            app.connection_state.clear();
        }

        // F5: set as source
        KeyCode::F(5) => {
            let state = &mut app.connection_state;
            if !state.name.is_empty() {
                let name = state.name.clone();
                let config = state.to_config();
                app.source_connection = Some(SavedConnection {
                    name: name.clone(),
                    config,
                });
                app.schema_diff_state.source_name = name.clone();
                app.data_sync_state.source_name = name.clone();
                app.connection_state.set_status(&format!("'{}' set as source", name), false);
            }
        }

        // F6: set as target
        KeyCode::F(6) => {
            let state = &mut app.connection_state;
            if !state.name.is_empty() {
                let name = state.name.clone();
                let config = state.to_config();
                app.target_connection = Some(SavedConnection {
                    name: name.clone(),
                    config,
                });
                app.schema_diff_state.target_name = name.clone();
                app.data_sync_state.target_name = name.clone();
                app.table_browser_state.connection_name = name.clone();
                app.connection_state.set_status(&format!("'{}' set as target", name), false);
            }
        }

        // Character input (only when no Ctrl modifier)
        KeyCode::Char(c) if !has_ctrl => {
            let state = &mut app.connection_state;
            match state.focused_field {
                0 => state.name.push(c),
                2 => {
                    if state.db_type == db::DbType::SQLite {
                        state.file_path.push(c);
                    } else {
                        state.host.push(c);
                    }
                }
                3 => state.port.push(c),
                4 => state.user.push(c),
                5 => state.password.push(c),
                6 => state.database.push(c),
                _ => {}
            }
        }

        // Backspace: delete character
        KeyCode::Backspace => {
            let state = &mut app.connection_state;
            match state.focused_field {
                0 => { state.name.pop(); }
                2 => {
                    if state.db_type == db::DbType::SQLite {
                        state.file_path.pop();
                    } else {
                        state.host.pop();
                    }
                }
                3 => { state.port.pop(); }
                4 => { state.user.pop(); }
                5 => { state.password.pop(); }
                6 => { state.database.pop(); }
                _ => {}
            }
        }

        _ => {}
    }
}

async fn handle_schema_diff_input(app: &mut App, key: KeyEvent) {
    let state = &mut app.schema_diff_state;

    match key.code {
        KeyCode::Up => state.prev(),
        KeyCode::Down => state.next(),

        // Enter: compare schemas
        KeyCode::Enter => {
            compare_schemas(app).await;
        }

        _ => {}
    }
}

async fn compare_schemas(app: &mut App) {
    if app.source_connection.is_none() || app.target_connection.is_none() {
        app.schema_diff_state.status = "Set source and target first (F5/F6 on Connections tab)".to_string();
        return;
    }

    app.spinner.start("Comparing schemas...");
    app.schema_diff_state.status = "Comparing schemas...".to_string();
    app.schema_diff_state.loading = true;

    let source_config = app.source_connection.as_ref().unwrap().config.clone();
    let target_config = app.target_connection.as_ref().unwrap().config.clone();

    let (tx, rx) = tokio::sync::mpsc::channel(1);
    app.task_rx = Some(rx);

    tokio::spawn(async move {
        let result = async {
            let source_conn = DbConnection::connect(&source_config).await
                .map_err(|e| e.to_string())?;
            let target_conn = DbConnection::connect(&target_config).await
                .map_err(|e| e.to_string())?;

            let source_schema = source_conn.get_schema(&source_config.database).await
                .map_err(|e| e.to_string())?;
            let target_schema = target_conn.get_schema(&target_config.database).await
                .map_err(|e| e.to_string())?;

            let diffs = db::compare_schemas(&source_schema, &target_schema, target_config.db_type);
            Ok(diffs)
        }.await;

        let _ = tx.send(TaskResult::SchemaCompare(result)).await;
    });
}

async fn handle_data_sync_input(app: &mut App, key: KeyEvent) {
    let state = &mut app.data_sync_state;
    let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Tab => state.toggle_focus(),
        KeyCode::Up => {
            if state.focus_left {
                state.prev_table();
            } else {
                state.prev_diff();
            }
        }
        KeyCode::Down => {
            if state.focus_left {
                state.next_table();
            } else {
                state.next_diff();
            }
        }

        // Ctrl+L: load tables
        KeyCode::Char('l') if has_ctrl => {
            load_data_sync_tables(app).await;
        }

        // Enter on table list: compare table data
        KeyCode::Enter => {
            if app.data_sync_state.focus_left {
                compare_table_data(app).await;
            }
        }

        _ => {}
    }
}

async fn load_data_sync_tables(app: &mut App) {
    if app.source_connection.is_none() {
        app.data_sync_state.status = "Set source connection first".to_string();
        return;
    }

    app.spinner.start("Loading tables...");
    app.data_sync_state.status = "Loading tables...".to_string();
    let config = app.source_connection.as_ref().unwrap().config.clone();

    let (tx, rx) = tokio::sync::mpsc::channel(1);
    app.task_rx = Some(rx);

    tokio::spawn(async move {
        let result = async {
            let conn = DbConnection::connect(&config).await.map_err(|e| e.to_string())?;
            let tables = conn.get_tables().await.map_err(|e| e.to_string())?;
            Ok(tables)
        }.await;

        let _ = tx.send(TaskResult::LoadTables(result)).await;
    });
}

async fn compare_table_data(app: &mut App) {
    if app.source_connection.is_none() || app.target_connection.is_none() {
        app.data_sync_state.status = "Set source and target connections first".to_string();
        return;
    }

    let table_name = match app.data_sync_state.tables.get(app.data_sync_state.selected_table_index).cloned() {
        Some(name) => name,
        None => return,
    };

    app.spinner.start(&format!("Comparing table {}...", table_name));
    app.data_sync_state.status = format!("Comparing table {}...", table_name);

    let source_config = app.source_connection.as_ref().unwrap().config.clone();
    let target_config = app.target_connection.as_ref().unwrap().config.clone();

    let (tx, rx) = tokio::sync::mpsc::channel(1);
    app.task_rx = Some(rx);

    tokio::spawn(async move {
        let result = async {
            let source_conn = DbConnection::connect(&source_config).await.map_err(|e| e.to_string())?;
            let target_conn = DbConnection::connect(&target_config).await.map_err(|e| e.to_string())?;

            let diffs = db::compare_table_data(
                &source_conn,
                &target_conn,
                &table_name,
                &source_config.database,
            ).await.map_err(|e| e.to_string())?;

            Ok(diffs)
        }.await;

        let _ = tx.send(TaskResult::CompareData(result)).await;
    });
}

async fn handle_table_browser_input(app: &mut App, key: KeyEvent) {
    let state = &mut app.table_browser_state;
    let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let has_shift = key.modifiers.contains(KeyModifiers::SHIFT);

    match key.code {
        KeyCode::Tab => state.toggle_focus(),
        KeyCode::Up => {
            if state.focus_left {
                state.prev_table();
            } else {
                state.prev_row();
            }
        }
        KeyCode::Down => {
            if state.focus_left {
                state.next_table();
            } else {
                state.next_row();
            }
        }
        KeyCode::Left => {
            if has_shift {
                // Shift+Left: scroll columns left
                app.table_browser_state.scroll_columns_left();
            } else {
                // Left: previous page
                let old_page = app.table_browser_state.page;
                app.table_browser_state.prev_page();
                if app.table_browser_state.page != old_page && !app.table_browser_state.columns.is_empty() {
                    load_table_data(app).await;
                }
            }
        }
        KeyCode::Right => {
            if has_shift {
                // Shift+Right: scroll columns right
                app.table_browser_state.scroll_columns_right();
            } else {
                // Right: next page
                let old_page = app.table_browser_state.page;
                app.table_browser_state.next_page();
                if app.table_browser_state.page != old_page && !app.table_browser_state.columns.is_empty() {
                    load_table_data(app).await;
                }
            }
        }

        // Ctrl+L: load tables
        KeyCode::Char('l') if has_ctrl => {
            load_browser_tables(app).await;
        }

        // Ctrl+R or Enter: refresh/load table data
        KeyCode::Char('r') if has_ctrl => {
            load_table_data(app).await;
        }
        KeyCode::Enter => {
            if app.table_browser_state.focus_left {
                load_table_data(app).await;
            }
        }

        _ => {}
    }
}

async fn load_browser_tables(app: &mut App) {
    if app.target_connection.is_none() {
        app.table_browser_state.status = "Set target connection first".to_string();
        return;
    }

    app.spinner.start("Loading tables...");
    app.table_browser_state.status = "Loading tables...".to_string();
    let config = app.target_connection.as_ref().unwrap().config.clone();

    let (tx, rx) = tokio::sync::mpsc::channel(1);
    app.task_rx = Some(rx);

    tokio::spawn(async move {
        let result = async {
            let conn = DbConnection::connect(&config).await.map_err(|e| e.to_string())?;
            let tables = conn.get_tables().await.map_err(|e| e.to_string())?;
            Ok(tables)
        }.await;

        let _ = tx.send(TaskResult::LoadTables(result)).await;
    });
}

async fn load_table_data(app: &mut App) {
    if app.target_connection.is_none() {
        app.table_browser_state.status = "Set target connection first".to_string();
        return;
    }

    let table_name = match app.table_browser_state.tables.get(app.table_browser_state.selected_table_index).cloned() {
        Some(name) => name,
        None => return,
    };

    app.spinner.start(&format!("Loading table {} data...", table_name));
    app.table_browser_state.status = format!("Loading {}...", table_name);

    let config = app.target_connection.as_ref().unwrap().config.clone();
    let page = app.table_browser_state.page;
    let page_size = app.table_browser_state.page_size;

    let (tx, rx) = tokio::sync::mpsc::channel(1);
    app.task_rx = Some(rx);

    tokio::spawn(async move {
        let result = async {
            let conn = DbConnection::connect(&config).await.map_err(|e| e.to_string())?;

            // Get columns
            let cols = conn.get_columns(&table_name).await.map_err(|e| e.to_string())?;
            let columns: Vec<String> = cols.iter().map(|c| c.name.clone()).collect();

            // Get row count
            let total_count = conn.get_row_count(&table_name).await.unwrap_or(0) as usize;

            // Get row data
            let rows = conn.get_table_rows(&table_name, &columns, page, page_size).await
                .map_err(|e| e.to_string())?;

            Ok::<_, String>((columns, rows, total_count))
        }.await;

        match result {
            Ok((columns, rows, total_count)) => {
                let _ = tx.send(TaskResult::LoadTableData { columns, rows, total_count }).await;
            }
            Err(e) => {
                let _ = tx.send(TaskResult::LoadTableDataError(e)).await;
            }
        }
    });
}
