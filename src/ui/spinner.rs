use std::time::{Duration, Instant};

/// Block-style spinner frames (rolling animation)
const BLOCK_FRAMES: &[&str] = &[
    "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█", "▉", "▊", "▋", "▌", "▍", "▎", "▏", " ",
];

/// Bouncing bar animation
const BAR_FRAMES: &[&str] = &[
    "[■□□□□□□□]",
    "[□■□□□□□□]",
    "[□□■□□□□□]",
    "[□□□■□□□□]",
    "[□□□□■□□□]",
    "[□□□□□■□□]",
    "[□□□□□□■□]",
    "[□□□□□□□■]",
    "[□□□□□□■□]",
    "[□□□□□■□□]",
    "[□□□□■□□□]",
    "[□□□□□□□□]",
    "[□□□■□□□□]",
    "[□□■□□□□□]",
    "[□■□□□□□□]",
];

/// Braille dots spinner
const DOTS_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Growing blocks
const GROW_FRAMES: &[&str] = &["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█", "▇", "▆", "▅", "▄", "▃", "▂"];

/// Spinner style
#[derive(Debug, Clone, Copy, Default)]
pub enum SpinnerStyle {
    #[default]
    Block,
    Bar,
    Dots,
    Grow,
}

impl SpinnerStyle {
    fn frames(&self) -> &'static [&'static str] {
        match self {
            SpinnerStyle::Block => BLOCK_FRAMES,
            SpinnerStyle::Bar => BAR_FRAMES,
            SpinnerStyle::Dots => DOTS_FRAMES,
            SpinnerStyle::Grow => GROW_FRAMES,
        }
    }
}

/// Async spinner state
#[derive(Debug, Clone)]
pub struct Spinner {
    pub active: bool,
    pub message: String,
    pub style: SpinnerStyle,
    frame_index: usize,
    last_update: Instant,
    frame_duration: Duration,
}

impl Default for Spinner {
    fn default() -> Self {
        Self {
            active: false,
            message: String::new(),
            style: SpinnerStyle::Bar,
            frame_index: 0,
            last_update: Instant::now(),
            frame_duration: Duration::from_millis(80),
        }
    }
}

impl Spinner {
    pub fn new(style: SpinnerStyle) -> Self {
        Self {
            style,
            ..Default::default()
        }
    }

    /// Start the spinner with a message
    pub fn start(&mut self, message: &str) {
        self.active = true;
        self.message = message.to_string();
        self.frame_index = 0;
        self.last_update = Instant::now();
    }

    /// Stop the spinner
    pub fn stop(&mut self) {
        self.active = false;
        self.message.clear();
    }

    /// Update the spinner frame (call this on each tick)
    pub fn tick(&mut self) {
        if !self.active {
            return;
        }

        let now = Instant::now();
        if now.duration_since(self.last_update) >= self.frame_duration {
            let frames = self.style.frames();
            self.frame_index = (self.frame_index + 1) % frames.len();
            self.last_update = now;
        }
    }

    /// Get current frame character
    pub fn frame(&self) -> &'static str {
        let frames = self.style.frames();
        frames[self.frame_index % frames.len()]
    }

    /// Get display string with spinner and message
    pub fn display(&self) -> String {
        if !self.active {
            return String::new();
        }
        format!("{} {}", self.frame(), self.message)
    }
}

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Draw spinner widget
pub fn draw_spinner(f: &mut Frame, area: Rect, spinner: &Spinner) {
    if !spinner.active {
        return;
    }

    let line = Line::from(vec![
        Span::styled(
            spinner.frame(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(&spinner.message, Style::default().fg(Color::Yellow)),
    ]);

    let paragraph = Paragraph::new(line);
    f.render_widget(paragraph, area);
}

/// Draw a progress bar with percentage
pub fn draw_progress_bar(f: &mut Frame, area: Rect, progress: f64, message: &str) {
    let width = area.width.saturating_sub(2) as usize;
    let filled = ((progress * width as f64) as usize).min(width);
    let empty = width.saturating_sub(filled);

    let bar = format!(
        "[{}{}] {:.0}% {}",
        "█".repeat(filled),
        "░".repeat(empty),
        progress * 100.0,
        message
    );

    let paragraph = Paragraph::new(bar).style(Style::default().fg(Color::Cyan));
    f.render_widget(paragraph, area);
}
