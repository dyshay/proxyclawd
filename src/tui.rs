use std::collections::HashMap;
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

/// A row in the display list, mapping to either a conversation header,
/// a normal child request, a tool-loop group header, or a tool-loop child.
#[derive(Debug, Clone)]
struct DisplayRow {
    /// Index into state.requests (None for tool-loop group headers)
    request_index: Option<usize>,
    /// Indentation level: 0 = conversation header, 1 = child/tool-loop-header, 2 = tool-loop child
    indent: u8,
    /// If this is a tool-loop group header, how many requests it contains
    tool_loop_count: Option<usize>,
    /// The conversation_id this row belongs to
    conversation_id: String,
    /// Whether this is the conversation header row
    is_conv_header: bool,
    /// A unique key for tool-loop groups to track collapse state
    tool_loop_key: Option<String>,
}

/// Build the display rows from the current app state, respecting collapse state.
fn build_display_rows(state: &AppState) -> Vec<DisplayRow> {
    if state.requests.is_empty() {
        return Vec::new();
    }

    // Group requests by conversation_id, preserving arrival order of first appearance
    let mut conv_order: Vec<String> = Vec::new();
    let mut conv_groups: HashMap<String, Vec<usize>> = HashMap::new();

    for (idx, req) in state.requests.iter().enumerate() {
        let cid = &req.conversation_id;
        if !conv_groups.contains_key(cid) {
            conv_order.push(cid.clone());
        }
        conv_groups.entry(cid.clone()).or_default().push(idx);
    }

    let mut rows = Vec::new();

    for cid in &conv_order {
        let indices = &conv_groups[cid];
        let is_collapsed = state.collapsed_conversations.contains(cid);

        // Sort by message_count within the conversation
        let mut sorted_indices = indices.clone();
        sorted_indices.sort_by_key(|&i| state.requests[i].message_count);

        // Single-request conversations: no header needed
        if sorted_indices.len() == 1 {
            rows.push(DisplayRow {
                request_index: Some(sorted_indices[0]),
                indent: 0,
                tool_loop_count: None,
                conversation_id: cid.clone(),
                is_conv_header: false,
                tool_loop_key: None,
            });
            continue;
        }

        // Conversation header (uses the first request for display info)
        rows.push(DisplayRow {
            request_index: Some(sorted_indices[0]),
            indent: 0,
            tool_loop_count: None,
            conversation_id: cid.clone(),
            is_conv_header: true,
            tool_loop_key: None,
        });

        if is_collapsed {
            continue;
        }

        // Build children, detecting contiguous tool-loop runs
        let mut i = 0;
        while i < sorted_indices.len() {
            let idx = sorted_indices[i];
            let req = &state.requests[idx];

            if req.is_tool_loop {
                // Find the run of contiguous tool-loop requests
                let run_start = i;
                while i < sorted_indices.len() && state.requests[sorted_indices[i]].is_tool_loop {
                    i += 1;
                }
                let run_len = i - run_start;
                let tool_loop_key = format!("{}-tl-{}", cid, run_start);
                let tl_collapsed = state.collapsed_conversations.contains(&tool_loop_key);

                // Tool-loop group header
                rows.push(DisplayRow {
                    request_index: Some(sorted_indices[run_start]),
                    indent: 1,
                    tool_loop_count: Some(run_len),
                    conversation_id: cid.clone(),
                    is_conv_header: false,
                    tool_loop_key: Some(tool_loop_key.clone()),
                });

                if !tl_collapsed {
                    for j in run_start..run_start + run_len {
                        rows.push(DisplayRow {
                            request_index: Some(sorted_indices[j]),
                            indent: 2,
                            tool_loop_count: None,
                            conversation_id: cid.clone(),
                            is_conv_header: false,
                            tool_loop_key: None,
                        });
                    }
                }
            } else {
                rows.push(DisplayRow {
                    request_index: Some(sorted_indices[i]),
                    indent: 1,
                    tool_loop_count: None,
                    conversation_id: cid.clone(),
                    is_conv_header: false,
                    tool_loop_key: None,
                });
                i += 1;
            }
        }
    }

    rows
}

pub async fn run_tui(mut event_rx: broadcast::Receiver<ProxyEvent>) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new();
    let mut list_state = ListState::default();
    let mut display_row_index: usize = 0;

    let result = run_tui_loop(
        &mut terminal,
        &mut state,
        &mut list_state,
        &mut display_row_index,
        &mut event_rx,
    )
    .await;

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
    display_row_index: &mut usize,
    event_rx: &mut broadcast::Receiver<ProxyEvent>,
) -> anyhow::Result<()> {
    loop {
        let rows = build_display_rows(state);

        // Sync list_state with display row index
        if !rows.is_empty() {
            *display_row_index = (*display_row_index).min(rows.len().saturating_sub(1));
            list_state.select(Some(*display_row_index));

            // Sync the underlying selected_index so prompt/response panels show the right request
            if let Some(row) = rows.get(*display_row_index) {
                if let Some(req_idx) = row.request_index {
                    state.selected_index = req_idx;
                }
            }
        }

        let rows_for_draw = rows.clone();
        terminal.draw(|f| draw_ui(f, state, list_state, &rows_for_draw))?;

        // Poll for keyboard events with a short timeout
        if crossterm::event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Up | KeyCode::Char('k') => {
                            if *display_row_index > 0 {
                                *display_row_index -= 1;
                            }
                            state.auto_select_latest = false;
                            state.response_scroll = 0;
                            state.prompt_scroll = 0;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if !rows.is_empty()
                                && *display_row_index < rows.len().saturating_sub(1)
                            {
                                *display_row_index += 1;
                            }
                            state.auto_select_latest =
                                *display_row_index == rows.len().saturating_sub(1);
                            state.response_scroll = 0;
                            state.prompt_scroll = 0;
                        }
                        KeyCode::Enter => {
                            // Toggle collapse on the selected row
                            if let Some(row) = rows.get(*display_row_index) {
                                if row.is_conv_header {
                                    let cid = row.conversation_id.clone();
                                    if !state.collapsed_conversations.remove(&cid) {
                                        state.collapsed_conversations.insert(cid);
                                    }
                                } else if let Some(ref tl_key) = row.tool_loop_key {
                                    if row.tool_loop_count.is_some() {
                                        let key = tl_key.clone();
                                        if !state.collapsed_conversations.remove(&key) {
                                            state.collapsed_conversations.insert(key);
                                        }
                                    }
                                }
                            }
                        }
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
                Ok(proxy_event) => {
                    state.apply_event(proxy_event);
                    // Auto-follow: move display row to last
                    if state.auto_select_latest {
                        let new_rows = build_display_rows(state);
                        *display_row_index = new_rows.len().saturating_sub(1);
                    }
                }
                Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    }
}

fn draw_ui(
    f: &mut ratatui::Frame,
    state: &AppState,
    list_state: &mut ListState,
    rows: &[DisplayRow],
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(40),
        ])
        .split(f.area());

    draw_request_list(f, state, list_state, rows, chunks[0]);
    draw_prompt_panel(f, state, chunks[1]);
    draw_response_panel(f, state, chunks[2]);
}

fn draw_request_list(
    f: &mut ratatui::Frame,
    state: &AppState,
    list_state: &mut ListState,
    rows: &[DisplayRow],
    area: ratatui::layout::Rect,
) {
    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| {
            let indent_str = match row.indent {
                0 => "",
                1 => "  ",
                _ => "    ",
            };

            // Tool-loop group header
            if let Some(count) = row.tool_loop_count {
                let tl_collapsed = row
                    .tool_loop_key
                    .as_ref()
                    .map(|k| state.collapsed_conversations.contains(k))
                    .unwrap_or(false);
                let arrow = if tl_collapsed { "▶" } else { "▼" };
                let line = Line::from(vec![
                    Span::raw(indent_str.to_string()),
                    Span::styled(
                        format!("{arrow} "),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!("tool loop ({count} calls)"),
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]);
                return ListItem::new(line);
            }

            // Conversation header
            if row.is_conv_header {
                let is_collapsed = state.collapsed_conversations.contains(&row.conversation_id);
                let arrow = if is_collapsed { "▶" } else { "▼" };

                // Count total requests in this conversation
                let conv_count = state
                    .requests
                    .iter()
                    .filter(|r| r.conversation_id == row.conversation_id)
                    .count();

                // Get the last request's status
                let last_status = state
                    .requests
                    .iter()
                    .filter(|r| r.conversation_id == row.conversation_id)
                    .last()
                    .map(|r| &r.status);

                let (status_icon, status_style) = match last_status {
                    Some(RequestStatus::Pending) => (".", Style::default().fg(Color::DarkGray)),
                    Some(RequestStatus::Streaming) => (
                        "●",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Some(RequestStatus::Complete) => ("✓", Style::default().fg(Color::Green)),
                    Some(RequestStatus::Error(_)) => ("!", Style::default().fg(Color::Red)),
                    None => (".", Style::default().fg(Color::DarkGray)),
                };

                let conv_short = &row.conversation_id[..8.min(row.conversation_id.len())];

                let line = Line::from(vec![
                    Span::styled(
                        format!("{arrow} "),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(format!("[{status_icon}] "), status_style),
                    Span::styled(
                        format!("conv:{conv_short} "),
                        Style::default()
                            .fg(Color::Blue)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("({conv_count} reqs)"),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]);
                return ListItem::new(line);
            }

            // Normal request row
            let req = &state.requests[row.request_index.unwrap()];
            let timestamp = req.timestamp.format("%H:%M:%S").to_string();
            let (status_icon, style) = match &req.status {
                RequestStatus::Pending => (".", Style::default().fg(Color::DarkGray)),
                RequestStatus::Streaming => (
                    "●",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
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

            let msg_count_label = format!(" [{}msg]", req.message_count);

            let line = Line::from(vec![
                Span::raw(indent_str.to_string()),
                Span::styled(format!("[{status_icon}] "), style),
                Span::styled(
                    format!("[{timestamp}] "),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    model_short.to_string(),
                    Style::default().fg(Color::Magenta),
                ),
                Span::styled(msg_count_label, Style::default().fg(Color::DarkGray)),
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
                .title_style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
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
                .title_style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
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
