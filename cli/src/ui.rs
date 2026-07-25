use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Tabs, Wrap};
use ratatui::Frame;

use crate::app::{App, Mode};

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(9),
            Constraint::Length(3),
            Constraint::Length(6),
            Constraint::Length(2),
        ])
        .split(frame.area());

    let selected = match app.mode {
        Mode::Send => 0,
        Mode::Receive => 1,
        Mode::Chat => 2,
    };
    let tabs = Tabs::new(["1 Send", "2 Receive", "3 Chat"])
        .block(
            Block::default()
                .title(" Sonic Share ")
                .borders(Borders::ALL),
        )
        .select(selected)
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider(" | ");
    frame.render_widget(tabs, sections[0]);

    match app.mode {
        Mode::Send => draw_send(frame, app, sections[1]),
        Mode::Receive => draw_receive(frame, app, sections[1]),
        Mode::Chat => draw_chat(frame, app, sections[1]),
    }

    let input_title = match app.mode {
        Mode::Send => " File path ",
        Mode::Chat => " Message ",
        Mode::Receive => " Action ",
    };
    let input = if app.mode == Mode::Receive {
        if app.receiving {
            "Press Enter to stop listening"
        } else {
            "Press Enter to start listening"
        }
    } else {
        app.input.as_str()
    };
    frame.render_widget(
        Paragraph::new(input).block(
            Block::default()
                .title(input_title)
                .title_style(if app.editing {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                })
                .borders(Borders::ALL),
        ),
        sections[2],
    );
    if app.editing && app.mode != Mode::Receive {
        let cursor_x = sections[2]
            .x
            .saturating_add(1)
            .saturating_add(app.input.chars().count() as u16)
            .min(sections[2].right().saturating_sub(2));
        frame.set_cursor_position((cursor_x, sections[2].y + 1));
    }

    let logs = app
        .logs
        .iter()
        .rev()
        .take(4)
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    frame.render_widget(
        Paragraph::new(logs)
            .block(
                Block::default()
                    .title(" Status / Log ")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true }),
        sections[3],
    );

    let help = if app.editing {
        "Enter: send/action  Backspace: edit  Esc: stop editing  Tab: next tab"
    } else {
        "1/2/3 or Tab: switch  Enter: edit/action  r: chat listener  q or Esc: quit"
    };
    frame.render_widget(
        Paragraph::new(help)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray)),
        sections[4],
    );
}

fn draw_send(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(area);
    let content = vec![
        Line::from(Span::styled(
            "Acoustic file transfer",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Enter a local file path below. Reading, FEC encoding, and playback run in the background."),
        Line::from("The default output device transmits the generated acoustic packets."),
        Line::from(""),
        Line::from(if app.editing {
            "Input active"
        } else {
            "Press Enter to edit the path"
        }),
    ];
    frame.render_widget(
        Paragraph::new(content)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        rows[0],
    );
    let (label, ratio) = app.tx_progress.map_or_else(
        || ("Transmission idle".to_owned(), 0.0),
        |(current, total)| {
            (
                format!("Packet {current}/{total}"),
                if total == 0 {
                    0.0
                } else {
                    current as f64 / total as f64
                },
            )
        },
    );
    frame.render_widget(
        Gauge::default()
            .block(
                Block::default()
                    .title(" TX progress ")
                    .borders(Borders::ALL),
            )
            .gauge_style(Style::default().fg(Color::Cyan))
            .label(label)
            .ratio(ratio.clamp(0.0, 1.0)),
        rows[1],
    );
}

fn draw_receive(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Min(3),
        ])
        .split(area);
    let device = app.input_device.as_deref().unwrap_or("Default input");
    let status = if app.receiving {
        "LISTENING"
    } else {
        "STOPPED"
    };
    frame.render_widget(
        Paragraph::new(format!("Status: {status}\nDevice: {device}"))
            .block(Block::default().title(" Receiver ").borders(Borders::ALL)),
        rows[0],
    );
    frame.render_widget(
        Gauge::default()
            .block(
                Block::default()
                    .title(" Input level ")
                    .borders(Borders::ALL),
            )
            .gauge_style(Style::default().fg(Color::Green))
            .ratio(f64::from(app.input_level.clamp(0.0, 1.0))),
        rows[1],
    );
    let progress = match app.transfer_progress {
        Some((completed, total)) if total > 0 => format!("Transfer: {completed}/{total} groups"),
        Some(_) => "Transfer detected; awaiting data".to_owned(),
        None => "Transfer: idle".to_owned(),
    };
    let received = app.received_path.as_ref().map_or_else(
        || "Received file: none".to_owned(),
        |path| format!("Received file: {}", path.display()),
    );
    frame.render_widget(
        Paragraph::new(format!("{progress}\n{received}"))
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        rows[2],
    );
}

fn draw_chat(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                if app.receiving {
                    "LISTENING"
                } else {
                    "NOT LISTENING"
                },
                Style::default().fg(if app.receiving {
                    Color::Green
                } else {
                    Color::Yellow
                }),
            ),
            Span::raw(format!("  input level: {:.0}%", app.input_level * 100.0)),
        ]),
        Line::from(""),
    ];
    if app.messages.is_empty() {
        lines.push(Line::from(
            "No messages yet. Chat messages are never saved to disk.",
        ));
    } else {
        lines.extend(
            app.messages
                .iter()
                .rev()
                .take(area.height.saturating_sub(4) as usize)
                .rev()
                .map(|message| {
                    let (prefix, color) = if message.incoming {
                        ("peer > ", Color::Green)
                    } else {
                        ("you  > ", Color::Cyan)
                    };
                    Line::from(vec![
                        Span::styled(prefix, Style::default().fg(color)),
                        Span::raw(&message.text),
                    ])
                })
                .collect::<Vec<_>>(),
        );
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" In-memory chat ")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;

    #[test]
    fn all_modes_render_on_common_terminal_sizes() {
        for (width, height) in [(100, 30), (60, 20)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).expect("test terminal");
            let mut app = App::default();
            for mode in [Mode::Send, Mode::Receive, Mode::Chat] {
                app.mode = mode;
                terminal
                    .draw(|frame| draw(frame, &app))
                    .expect("render mode");
            }
        }
    }
}
