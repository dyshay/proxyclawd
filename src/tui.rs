use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Terminal;
use tokio::sync::broadcast;

use crate::state::{AppState, ProxyEvent, RequestStatus};

pub async fn run_tui(mut event_rx: broadcast::Receiver<ProxyEvent>) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new();
    let mut list_state = ListState::default();

    let result = run_tui_loop(&mut terminal, &mut state, &mut list_state, &mut event_rx).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_tui_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
    list_state: &mut ListState,
    event_rx: &mut broadcast::Receiver<ProxyEvent>,
) -> anyhow::Result<()> {
    loop {
        // Sync list_state with app state
        if !state.requests.is_empty() {
            list_state.select(Some(state.selected_index));
        }

        terminal.draw(|f| draw_ui(f, state, list_state))?;

        // Poll for keyboard events with a short timeout
        if crossterm::event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Up | KeyCode::Char('k') => state.select_previous(),
                        KeyCode::Down | KeyCode::Char('j') => state.select_next(),
                        KeyCode::PageDown => state.scroll_response_down(10),
                        KeyCode::PageUp => state.scroll_response_up(10),
                        _ => {}
                    }
                }
            }
        }

        // Drain all pending proxy events
        loop {
            match event_rx.try_recv() {
                Ok(proxy_event) => state.apply_event(proxy_event),
                Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    }
}

fn draw_ui(f: &mut ratatui::Frame, state: &AppState, list_state: &mut ListState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(40),
        ])
        .split(f.area());

    draw_request_list(f, state, list_state, chunks[0]);
    draw_prompt_panel(f, state, chunks[1]);
    draw_response_panel(f, state, chunks[2]);
}

fn draw_request_list(
    f: &mut ratatui::Frame,
    state: &AppState,
    list_state: &mut ListState,
    area: ratatui::layout::Rect,
) {
    let items: Vec<ListItem> = state
        .requests
        .iter()
        .map(|req| {
            let timestamp = req.timestamp.format("%H:%M:%S").to_string();
            let (status_icon, style) = match &req.status {
                RequestStatus::Pending => (".", Style::default().fg(Color::DarkGray)),
                RequestStatus::Streaming => {
                    ("●", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
                }
                RequestStatus::Complete => ("✓", Style::default().fg(Color::Green)),
                RequestStatus::Error(_) => ("!", Style::default().fg(Color::Red)),
            };

            let model_short = if req.model.len() > 20 {
                &req.model[..20]
            } else {
                &req.model
            };

            let status_text = match &req.status {
                RequestStatus::Streaming => "  streaming...".to_string(),
                RequestStatus::Error(e) => format!("  err: {}", truncate(e, 30)),
                _ => String::new(),
            };

            let line = Line::from(vec![
                Span::styled(format!("[{status_icon}] "), style),
                Span::styled(
                    format!("[{timestamp}] "),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{} {} ", req.method, req.path),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    model_short.to_string(),
                    Style::default().fg(Color::Magenta),
                ),
                Span::styled(status_text, style),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Requests ")
                .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, list_state);
}

fn draw_prompt_panel(f: &mut ratatui::Frame, state: &AppState, area: ratatui::layout::Rect) {
    let content = if let Some(req) = state.selected_request() {
        let mut text = String::new();
        text.push_str("── user ──\n");
        text.push_str(&req.prompt_text);
        if let Some(system) = &req.system_prompt {
            text.push_str("\n\n── system ──\n");
            text.push_str(system);
        }
        text
    } else {
        "No request selected".to_string()
    };

    let paragraph = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Prompt ")
                .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        )
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false })
        .scroll((state.prompt_scroll, 0));

    f.render_widget(paragraph, area);
}

fn draw_response_panel(f: &mut ratatui::Frame, state: &AppState, area: ratatui::layout::Rect) {
    let (content, title) = if let Some(req) = state.selected_request() {
        let title = match &req.status {
            RequestStatus::Streaming => " Response (live) ● ",
            RequestStatus::Complete => " Response (complete) ",
            RequestStatus::Error(_) => " Response (error) ",
            RequestStatus::Pending => " Response (waiting...) ",
        };
        (req.response_text.clone(), title)
    } else {
        ("No request selected".to_string(), " Response ")
    };

    let title_style = if state
        .selected_request()
        .map(|r| r.status == RequestStatus::Streaming)
        .unwrap_or(false)
    {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    };

    let paragraph = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .title_style(title_style),
        )
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false })
        .scroll((state.response_scroll, 0));

    f.render_widget(paragraph, area);
}

fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        &s[..s.floor_char_boundary(max_len)]
    }
}
