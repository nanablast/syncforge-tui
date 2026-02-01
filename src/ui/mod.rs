mod connection_form;
mod schema_diff;
mod data_sync;
mod table_browser;
mod spinner;

pub use connection_form::*;
pub use schema_diff::*;
pub use data_sync::*;
pub use table_browser::*;
pub use spinner::*;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Tabs},
    Frame,
};

/// Main tab
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Connection,
    SchemaDiff,
    DataSync,
    TableBrowser,
}

impl Tab {
    pub fn titles() -> Vec<&'static str> {
        vec!["F1 Connections", "F2 Schema Diff", "F3 Data Sync", "F4 Browser"]
    }

    pub fn index(&self) -> usize {
        match self {
            Tab::Connection => 0,
            Tab::SchemaDiff => 1,
            Tab::DataSync => 2,
            Tab::TableBrowser => 3,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Tab::Connection,
            1 => Tab::SchemaDiff,
            2 => Tab::DataSync,
            3 => Tab::TableBrowser,
            _ => Tab::Connection,
        }
    }

    pub fn next(&self) -> Self {
        Tab::from_index((self.index() + 1) % 4)
    }

    pub fn prev(&self) -> Self {
        Tab::from_index((self.index() + 3) % 4)
    }
}

/// Draw tab bar
pub fn draw_tabs(f: &mut Frame, area: Rect, current_tab: Tab) {
    let titles: Vec<Line> = Tab::titles()
        .iter()
        .map(|t| Line::from(Span::raw(*t)))
        .collect();

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("SyncForge Database Sync Tool"))
        .select(current_tab.index())
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, area);
}

/// Create centered rect
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
