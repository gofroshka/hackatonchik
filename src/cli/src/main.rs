use std::error::Error;
use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::Duration;

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use sonic_share_cli::app::{App, Intent, Key, Mode};
use sonic_share_cli::runtime::{Runtime, RuntimeEvent};

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(error);
        }
        match Terminal::new(CrosstermBackend::new(stdout)) {
            Ok(terminal) => Ok(Self { terminal }),
            Err(error) => {
                let _ = disable_raw_mode();
                let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
                Err(error)
            }
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen, Show);
        let _ = self.terminal.show_cursor();
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut session = TerminalSession::enter()?;
    let runtime = Runtime::new(PathBuf::from("received"));
    let mut app = App::default();
    let mut resume_receive_after_tx = false;

    loop {
        while let Ok(runtime_event) = runtime.try_event() {
            match &runtime_event {
                RuntimeEvent::TxStarted if app.receiving => {
                    runtime.stop_receive().map_err(io::Error::other)?;
                    resume_receive_after_tx = true;
                }
                RuntimeEvent::TxFinished if resume_receive_after_tx => {
                    runtime.start_receive().map_err(io::Error::other)?;
                    resume_receive_after_tx = false;
                }
                _ => {}
            }
            app.update(runtime_event);
        }
        session
            .terminal
            .draw(|frame| sonic_share_cli::ui::draw(frame, &app))?;

        if !event::poll(Duration::from_millis(50))? {
            continue;
        }
        let Event::Key(event) = event::read()? else {
            continue;
        };
        if event.kind != KeyEventKind::Press {
            continue;
        }
        let key = match event.code {
            KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => Key::Esc,
            KeyCode::Char(character) => Key::Char(character),
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Enter => Key::Enter,
            KeyCode::Esc => Key::Esc,
            KeyCode::Tab => Key::Tab,
            _ => continue,
        };
        let previous_mode = app.mode;
        let intent = app.handle_key(key);
        if app.mode != previous_mode {
            if app.mode == Mode::Chat && !app.receiving {
                if let Err(error) = runtime.start_receive() {
                    app.update(RuntimeEvent::Error(error));
                }
            } else if previous_mode == Mode::Chat && app.mode == Mode::Send && app.receiving {
                if let Err(error) = runtime.stop_receive() {
                    app.update(RuntimeEvent::Error(error));
                }
            }
        }
        let Some(intent) = intent else { continue };
        let result = match intent {
            Intent::SendFile(path) => runtime.send_file(path),
            Intent::SendChat(text) => runtime.send_chat(text),
            Intent::StartReceive => runtime.start_receive(),
            Intent::StopReceive => runtime.stop_receive(),
            Intent::Quit => break,
        };
        if let Err(error) = result {
            app.update(RuntimeEvent::Error(error));
        }
    }
    Ok(())
}
