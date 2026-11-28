//! Draws the console: status bar, unit list, log pane, command line, help.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph};

use crate::app::{App, UnitState};
use crate::core::types::{Focus, Group, Mode, Probe, Status, Stream};

const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;

/// Renders one frame.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let [bar, body, bottom] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    let [side, right] =
        Layout::horizontal([Constraint::Length(34), Constraint::Min(20)]).areas(body);

    status_bar(frame, app, bar);
    sidebar(frame, app, side);
    logs(frame, app, right);
    bottom_line(frame, app, bottom);
    if app.help {
        help(frame, frame.area());
    }
}

fn probe_span(name: &str, p: &Probe) -> Span<'static> {
    let (glyph, color) = match p {
        Probe::Unknown => ("·", DIM),
        Probe::Up => ("●", Color::Green),
        Probe::Degraded(_) => ("◐", Color::Yellow),
        Probe::Down => ("○", Color::Red),
    };
    Span::styled(format!(" {glyph} {name}"), Style::default().fg(color))
}

fn status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mode = match app.mode {
        Mode::Normal => " NORMAL ",
        Mode::Command => " COMMAND ",
        Mode::Search => " SEARCH ",
    };
    let mut spans = vec![
        Span::styled(
            " sparky ",
            Style::default().fg(Color::Black).bg(ACCENT).bold(),
        ),
        Span::styled(mode, Style::default().fg(Color::Black).bg(Color::White)),
        probe_span("engine", &app.health.engine),
        probe_span("model", &app.health.model),
        probe_span("phoenix", &app.health.phoenix),
    ];
    if let Probe::Degraded(why) = &app.health.engine {
        spans.push(Span::styled(
            format!("  {why}"),
            Style::default().fg(Color::Yellow),
        ));
    }
    if let Some(n) = &app.notice {
        spans.push(Span::styled(
            format!("  {n}"),
            Style::default().fg(Color::Magenta),
        ));
    }
    frame.render_widget(Line::from(spans), area);
}

fn status_style(s: &Status) -> Style {
    match s {
        Status::Stopped => Style::default().fg(DIM),
        Status::Starting => Style::default().fg(Color::Yellow),
        Status::Running => Style::default().fg(Color::Green),
        Status::Exited(0) => Style::default().fg(Color::Blue),
        Status::Exited(_) | Status::Failed(_) => Style::default().fg(Color::Red),
    }
}

fn sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let mut items: Vec<ListItem> = Vec::new();
    let mut selected_row = 0;
    for group in Group::ALL {
        let members: Vec<(usize, &UnitState)> = app
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.unit.group == group)
            .collect();
        if members.is_empty() {
            continue;
        }
        items.push(ListItem::new(Line::from(Span::styled(
            format!(" {}", group.title()),
            Style::default().fg(DIM).add_modifier(Modifier::BOLD),
        ))));
        for (i, u) in members {
            if i == app.selected {
                selected_row = items.len();
            }
            let port = u
                .unit
                .url
                .as_deref()
                .and_then(|u| u.rsplit(':').next())
                .filter(|p| p.chars().all(|c| c.is_ascii_digit()))
                .map_or_default(|p| format!(":{p}"));
            let name_width = 24usize.saturating_sub(port.len());
            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!("  {} ", u.status.glyph()), status_style(&u.status)),
                Span::raw(format!("{:<name_width$}", truncate(&u.unit.id, name_width))),
                Span::styled(port, Style::default().fg(DIM)),
            ])));
        }
    }
    let focused = app.focus == Focus::Units;
    let list = List::new(items)
        .block(pane(" units ", focused))
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 44, 52))
                .add_modifier(Modifier::BOLD),
        );
    let mut state = ListState::default().with_selected(Some(selected_row));
    frame.render_stateful_widget(list, area, &mut state);
}

fn logs(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Logs;
    let rows = usize::from(area.height.saturating_sub(2)).max(1);
    app.log_rows = rows;
    let search = app.search.to_lowercase();
    let hit = app.search_hit;
    let u = app.current();
    let elapsed = u
        .started_at
        .filter(|_| u.status.is_active())
        .map_or_default(|t| format!(" · {}s", t.elapsed().as_secs()));
    let title = format!(
        " {} · {}{} · {} lines{} ",
        u.unit.id,
        u.status.label(),
        elapsed,
        u.logs.len(),
        if u.follow { " · follow" } else { "" }
    );
    let top = if u.follow {
        u.logs.len().saturating_sub(rows)
    } else {
        u.scroll.min(u.logs.len().saturating_sub(rows))
    };
    let lines: Vec<Line> = u
        .logs
        .lines()
        .enumerate()
        .skip(top)
        .take(rows)
        .map(|(i, l)| {
            let mut style = match l.stream {
                Stream::Out => Style::default(),
                Stream::Err => Style::default().fg(Color::Gray),
                Stream::Meta => Style::default().fg(ACCENT).italic(),
            };
            if !search.is_empty() && l.text.to_lowercase().contains(&search) {
                style = style.fg(Color::Yellow);
                if hit == Some(i) {
                    style = style.add_modifier(Modifier::BOLD | Modifier::REVERSED);
                }
            }
            Line::from(vec![
                Span::styled(
                    l.at.format("%H:%M:%S ").to_string(),
                    Style::default().fg(DIM),
                ),
                Span::styled(l.text.as_str(), style),
            ])
        })
        .collect();
    let block = pane(&title, focused).title_bottom(
        Line::from(Span::styled(
            format!(" {} ", u.unit.hint),
            Style::default().fg(DIM),
        ))
        .right_aligned(),
    );
    let body = if u.logs.is_empty() {
        Paragraph::new(Line::from(Span::styled(
            "  nothing captured yet — press enter to start",
            Style::default().fg(DIM),
        )))
    } else {
        Paragraph::new(lines)
    };
    frame.render_widget(body.block(block), area);
}

fn bottom_line(frame: &mut Frame, app: &App, area: Rect) {
    let line = match app.mode {
        Mode::Command => Line::from(vec![
            Span::styled(":", Style::default().fg(ACCENT)),
            Span::raw(app.input.clone()),
            Span::styled("█", Style::default().fg(ACCENT)),
        ]),
        Mode::Search => Line::from(vec![
            Span::styled("/", Style::default().fg(Color::Yellow)),
            Span::raw(app.input.clone()),
            Span::styled("█", Style::default().fg(Color::Yellow)),
        ]),
        Mode::Normal => {
            let mut spans: Vec<Span> = Vec::new();
            for (k, w) in [
                ("j/k", "move"),
                ("⏎", "start/stop"),
                ("r", "restart"),
                ("l/h", "logs/units"),
                ("/", "search"),
                (":", "command"),
                ("o", "open url"),
                ("?", "help"),
                ("q", "quit"),
            ] {
                spans.extend(hint(k, w));
            }
            spans.push(Span::styled(
                format!(
                    "   engine {} · phoenix {}",
                    app.engine_url(),
                    app.phoenix_url()
                ),
                Style::default().fg(DIM),
            ));
            Line::from(spans)
        }
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn hint(key: &str, what: &str) -> [Span<'static>; 2] {
    [
        Span::styled(format!(" {key}"), Style::default().fg(ACCENT).bold()),
        Span::styled(format!(" {what}"), Style::default().fg(DIM)),
    ]
}

fn help(frame: &mut Frame, area: Rect) {
    let text = [
        ("j / k, ↑ / ↓", "move selection, scroll logs"),
        ("gg / G", "top / bottom (G on logs resumes following)"),
        ("ctrl-d / ctrl-u", "half page in logs"),
        ("enter / s", "start or stop the selected unit"),
        ("x / r", "stop / restart the selected unit"),
        ("h / l, tab", "focus units / logs"),
        ("/ then n / N", "search the selected unit's logs"),
        ("C", "clear the selected unit's logs"),
        ("o", "open the unit's URL in a browser"),
        (
            ":start x  :stop x  :restart x",
            "by unit id, e.g. :start engine",
        ),
        (
            ":<recipe> [args]",
            "run any just recipe as a task, e.g. :eval run",
        ),
        (":q", "quit; host processes are stopped, containers stay up"),
    ];
    let mut lines = vec![
        Line::from(Span::styled("keys", Style::default().fg(ACCENT).bold())),
        Line::default(),
    ];
    for (k, v) in text {
        lines.push(Line::from(vec![
            Span::styled(format!("  {k:<32}"), Style::default().fg(Color::Yellow)),
            Span::raw(v),
        ]));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "any key closes this",
        Style::default().fg(DIM),
    )));
    let width = 96.min(area.width.saturating_sub(4));
    let height = u16::try_from(lines.len() + 2)
        .unwrap_or(u16::MAX)
        .min(area.height.saturating_sub(2));
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .block(pane(" help ", true))
            .alignment(Alignment::Left),
        popup,
    );
}

fn pane(title: &str, focused: bool) -> Block<'static> {
    let border = if focused {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(DIM)
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(Span::styled(
            title.to_owned(),
            Style::default().fg(Color::White).bold(),
        ))
}

fn truncate(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(width.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
