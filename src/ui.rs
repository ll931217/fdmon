use crate::app::{App, InputMode, SortColumn, StatusLevel, ViewMode};
use crate::proc::FdType;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};

pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Min(0),    // Content
            Constraint::Length(2), // Footer
        ])
        .split(frame.area());

    render_header(frame, app, chunks[0]);
    render_content(frame, app, chunks[1]);
    render_footer(frame, app, chunks[2]);

    // Render modals on top
    if app.input_mode == InputMode::Search {
        render_search_input(frame, app);
    } else if app.input_mode == InputMode::KillConfirm {
        render_kill_confirm(frame, app);
    } else if app.input_mode == InputMode::UserSummary {
        render_user_summary(frame, app);
    }
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let view_indicator = match app.view {
        ViewMode::Table => "[Table] / Tree",
        ViewMode::Tree => "Table / [Tree]",
    };

    let header = Paragraph::new(format!(
        "fdmon - File Descriptor Monitor         {}",
        view_indicator
    ))
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_widget(header, area);
}

fn render_content(frame: &mut Frame, app: &App, area: Rect) {
    if app.detail_open {
        // Split into main view (left) and detail panel (right)
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);

        match app.view {
            ViewMode::Table => render_table_view(frame, app, chunks[0]),
            ViewMode::Tree => render_tree_view(frame, app, chunks[0]),
        }

        render_detail_panel(frame, app, chunks[1]);
    } else {
        match app.view {
            ViewMode::Table => render_table_view(frame, app, area),
            ViewMode::Tree => render_tree_view(frame, app, area),
        }
    }
}

fn render_table_view(frame: &mut Frame, app: &App, area: Rect) {
    let filtered = app.get_filtered_processes();

    let header_cells = ["FDs", "PID", "Owner", "Limit", "Usage%", "Command"]
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let mut style = Style::default().add_modifier(Modifier::BOLD);

            // Highlight sorted column
            let is_sorted = matches!(
                (i, app.sort_column),
                (0, SortColumn::FdCount)
                    | (1, SortColumn::Pid)
                    | (2, SortColumn::Owner)
                    | (5, SortColumn::Command)
            );

            if is_sorted {
                style = style.fg(Color::Yellow);
            }

            Cell::from(*h).style(style)
        });

    let header = Row::new(header_cells)
        .style(Style::default().fg(Color::White))
        .bottom_margin(1);

    // Build rows as Vec to allow inserting detail row
    let mut rows = Vec::new();

    for (i, p) in filtered.iter().enumerate() {
        let fd_count_str = p.fd_count.to_string();
        let pid_str = p.pid.to_string();
        let owner_str = &p.owner;
        let limit_str = p
            .fd_soft_limit
            .map(|l| l.to_string())
            .unwrap_or_else(|| "?".to_string());

        let usage_pct = if let Some(limit) = p.fd_soft_limit {
            if limit > 0 {
                (p.fd_count as f64 / limit as f64 * 100.0) as u32
            } else {
                0
            }
        } else {
            0
        };

        let usage_str = format!("{}%", usage_pct);

        // Color code based on usage
        let usage_color = if usage_pct > 80 {
            Color::Red
        } else if usage_pct > 50 {
            Color::Yellow
        } else {
            Color::White
        };

        // Calculate available width for command column
        // Fixed columns: FDs(8) + PID(8) + Owner(12) + Limit(8) + Usage%(8) = 44
        // Borders: 2, Column spacing (5 gaps × 1): 5
        // Total fixed: 51
        let cmd_width = area.width.saturating_sub(51) as usize;
        let command_str = truncate(&p.command, cmd_width.max(10));

        let cells = vec![
            Cell::from(fd_count_str),
            Cell::from(pid_str),
            Cell::from(owner_str.clone()),
            Cell::from(limit_str),
            Cell::from(usage_str).style(Style::default().fg(usage_color)),
            Cell::from(command_str),
        ];

        let mut style = Style::default();
        if i == app.selected {
            style = style.bg(Color::DarkGray).add_modifier(Modifier::BOLD);
        }

        rows.push(Row::new(cells).style(style));

        // Add placeholder CWD detail row for selected process
        // (content will be overwritten by buffer overlay below)
        if i == app.selected {
            let detail_style = Style::default().fg(Color::Gray).bg(Color::DarkGray);

            // Placeholder row — just reserves vertical space
            let empty_cells: Vec<Cell> =
                (0..6).map(|_| Cell::from("").style(detail_style)).collect();
            rows.push(Row::new(empty_cells));
        }
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),  // FDs
            Constraint::Length(8),  // PID
            Constraint::Length(12), // Owner
            Constraint::Length(8),  // Limit
            Constraint::Length(8),  // Usage%
            Constraint::Min(30),    // Command
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL));

    frame.render_widget(table, area);

    // Overlay CWD detail text directly onto the buffer (bypasses column constraints)
    if !filtered.is_empty() {
        // Y position: border(1) + header(1) + header margin(1) + selected rows + 1 for CWD row
        let cwd_y = area.y + 3 + app.selected as u16 + 1;
        let max_y = area.y + area.height.saturating_sub(1); // before bottom border

        if cwd_y < max_y {
            if let Some(p) = filtered.get(app.selected) {
                let cwd_text = format!("  └─ cwd: {}", &p.cwd);
                let style = Style::default().fg(Color::Gray).bg(Color::DarkGray);
                let x_start = area.x + 1; // after left border
                let max_width = area.width.saturating_sub(2) as usize; // between borders
                let buf = frame.buffer_mut();

                // Fill entire row with spaces to create consistent background
                let padding = " ".repeat(max_width);
                buf.set_string(x_start, cwd_y, &padding, style);

                // Then overlay the cwd text
                buf.set_string(
                    x_start,
                    cwd_y,
                    &cwd_text[..cwd_text.len().min(max_width)],
                    style,
                );
            }
        }
    }
}

fn render_tree_view(frame: &mut Frame, app: &App, area: Rect) {
    let rows = app.tree_nodes.iter().enumerate().map(|(i, node)| {
        let indent = "  ".repeat(node.depth);
        let icon = if node.has_children {
            if node.expanded {
                "▶"
            } else {
                "▷"
            }
        } else {
            "●"
        };

        let fd_info = format!("({} fds)", node.process.fd_count);
        let line = format!(
            "{}{} {}  {}  {}",
            indent, icon, node.process.pid, node.process.command, fd_info
        );

        let mut style = Style::default();
        if node.is_ancestor_only {
            style = style.fg(Color::DarkGray);
        }
        if i == app.selected {
            style = style.bg(Color::DarkGray).add_modifier(Modifier::BOLD);
        }

        Row::new(vec![Cell::from(line)]).style(style)
    });

    let table = Table::new(rows, [Constraint::Min(0)])
        .block(Block::default().borders(Borders::ALL).title("Process Tree"));

    frame.render_widget(table, area);
}

fn render_detail_panel(frame: &mut Frame, app: &App, area: Rect) {
    let title_base = match app.view {
        ViewMode::Tree => app
            .tree_nodes
            .get(app.selected)
            .map(|node| format!("FD Details: PID {}", node.process.pid))
            .unwrap_or_else(|| "FD Details".to_string()),
        ViewMode::Table => app
            .get_filtered_processes()
            .get(app.selected)
            .map(|p| format!("FD Details: PID {}", p.pid))
            .unwrap_or_else(|| "FD Details".to_string()),
    };

    // Add scroll indicator to title
    let total_fds = app.fd_details.len();
    let title = if total_fds > 0 {
        format!(
            "{} ({}/{}) [PgUp/PgDn or Ctrl+u/d to scroll]",
            title_base,
            app.detail_scroll_offset.min(total_fds.saturating_sub(1)) + 1,
            total_fds
        )
    } else {
        title_base
    };

    // Skip items based on scroll offset
    let rows = app
        .fd_details
        .iter()
        .skip(app.detail_scroll_offset)
        .map(|fd| {
            let fd_type_str = match fd.fd_type {
                FdType::Socket => "socket",
                FdType::Pipe => "pipe",
                FdType::File => "file",
                FdType::Device => "device",
                FdType::Other => "other",
            };

            let line = format!("{}  {} ({})", fd.fd_num, fd.target, fd_type_str);
            Row::new(vec![Cell::from(line)])
        });

    let table = Table::new(rows, [Constraint::Min(0)])
        .block(Block::default().borders(Borders::ALL).title(title));

    frame.render_widget(table, area);
}

fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    // System stats
    let sys_usage_pct = if app.system_stats.max_fds > 0 {
        (app.system_stats.allocated_fds as f64 / app.system_stats.max_fds as f64 * 100.0) as u32
    } else {
        0
    };

    let stats_line = format!(
        "System: {}/{} ({}%)  User({}): {} FDs",
        app.system_stats.allocated_fds,
        app.system_stats.max_fds,
        sys_usage_pct,
        app.system_stats.username,
        app.system_stats.user_fds
    );

    let stats = Paragraph::new(stats_line).style(Style::default().fg(Color::Green));
    frame.render_widget(stats, chunks[0]);

    // Key bindings or status message
    let bottom_line = if let Some((msg, level)) = &app.status_message {
        let color = match level {
            StatusLevel::Info => Color::Green,
            StatusLevel::Error => Color::Red,
        };
        Paragraph::new(msg.clone()).style(Style::default().fg(color))
    } else {
        let keys =
            "q:Quit  K:Kill  Tab:View  /:Search  u:Users  Enter:Details  ↑↓:Nav  s:Sort  +/-:Interval";
        Paragraph::new(keys).style(Style::default().fg(Color::Gray))
    };

    frame.render_widget(bottom_line, chunks[1]);
}

fn render_search_input(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 20, frame.area());

    let input = Paragraph::new(format!("Search: {}_", app.search_input))
        .style(Style::default().fg(Color::Yellow).bg(Color::Black))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Filter (Enter to apply, Esc to cancel)")
                .style(Style::default().bg(Color::Black).fg(Color::White)),
        );

    // Clear the background area first
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Black)),
        area,
    );
    frame.render_widget(input, area);
}

fn render_kill_confirm(frame: &mut Frame, app: &App) {
    let area = centered_rect(50, 30, frame.area());

    let pid = app.confirm_kill.unwrap_or(0);
    let process_name = app
        .get_filtered_processes()
        .iter()
        .find(|p| p.pid == pid)
        .map(|p| p.command.as_str())
        .unwrap_or("?");

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("Kill PID {} ({})?", pid, truncate(process_name, 30)),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "[t] SIGTERM   [k] SIGKILL   [Esc] Cancel",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Confirm Kill")
                .style(Style::default().bg(Color::Black).fg(Color::White)),
        )
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .style(Style::default().bg(Color::Black));

    // Clear the background area first
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Black)),
        area,
    );
    frame.render_widget(popup, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

fn render_user_summary(frame: &mut Frame, app: &App) {
    let area = centered_rect(70, 60, frame.area());

    // Clear the background area first
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Black)),
        area,
    );

    let title = "Per-User FD Summary (Esc to close)";

    // Calculate visible height for auto-scroll
    // area.height - 2 (borders) - 2 (header + margin)
    let visible_height = area.height.saturating_sub(4) as usize;

    // Compute scroll offset to keep selected row visible
    let scroll_offset = if !app.user_summaries.is_empty() && visible_height > 0 {
        let selected = app.user_summary_selected;
        if selected < visible_height {
            0
        } else {
            selected.saturating_sub(visible_height).saturating_add(1)
        }
    } else {
        0
    };

    // Header row
    let header_cells = ["User", "UID", "Total FDs", "Processes"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD)));

    let header = Row::new(header_cells)
        .style(Style::default().fg(Color::Yellow).bg(Color::Black))
        .bottom_margin(1);

    // Data rows with auto-scroll - pad with empty rows to fill visible area
    let mut row_vec: Vec<Row> = app
        .user_summaries
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(i, summary)| {
            let cells = vec![
                Cell::from(summary.username.clone()),
                Cell::from(summary.uid.to_string()),
                Cell::from(summary.total_fds.to_string()),
                Cell::from(summary.process_count.to_string()),
            ];

            let style = if i == app.user_summary_selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().bg(Color::Black)
            };

            Row::new(cells).style(style)
        })
        .collect();

    // Pad with empty rows to fill the visible area with black background
    let current_rows = row_vec.len();
    for _ in current_rows..visible_height {
        let empty_cells = vec![
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
        ];
        row_vec.push(Row::new(empty_cells).style(Style::default().bg(Color::Black)));
    }

    let table = Table::new(
        row_vec,
        [
            Constraint::Length(16), // User
            Constraint::Length(8),  // UID
            Constraint::Length(12), // Total FDs
            Constraint::Length(12), // Processes
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().bg(Color::Black).fg(Color::White)),
    )
    .style(Style::default().bg(Color::Black));

    frame.render_widget(table, area);
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
