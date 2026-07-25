use std::collections::VecDeque;
use std::path::PathBuf;

use crate::runtime::RuntimeEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Send,
    Receive,
    Chat,
}

impl Mode {
    fn next(self) -> Self {
        match self {
            Self::Send => Self::Receive,
            Self::Receive => Self::Chat,
            Self::Chat => Self::Send,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Backspace,
    Enter,
    Esc,
    Tab,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Intent {
    SendFile(PathBuf),
    SendChat(String),
    StartReceive,
    StopReceive,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub incoming: bool,
    pub text: String,
}

pub struct App {
    pub mode: Mode,
    pub editing: bool,
    pub input: String,
    pub receiving: bool,
    pub input_device: Option<String>,
    pub input_level: f32,
    pub transfer_progress: Option<(u32, u32)>,
    pub tx_progress: Option<(usize, usize)>,
    pub received_path: Option<PathBuf>,
    pub messages: Vec<ChatMessage>,
    pub logs: VecDeque<String>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            mode: Mode::Send,
            editing: true,
            input: String::new(),
            receiving: false,
            input_device: None,
            input_level: 0.0,
            transfer_progress: None,
            tx_progress: None,
            received_path: None,
            messages: Vec::new(),
            logs: VecDeque::from(["Ready".to_owned()]),
        }
    }
}

impl App {
    pub fn handle_key(&mut self, key: Key) -> Option<Intent> {
        match key {
            Key::Tab => {
                self.mode = self.mode.next();
                self.editing = self.mode != Mode::Receive;
            }
            Key::Esc if self.editing => self.editing = false,
            Key::Esc | Key::Char('q') if !self.editing => return Some(Intent::Quit),
            Key::Char('1' | '2' | '3') if !self.editing => {
                self.mode = match key {
                    Key::Char('1') => Mode::Send,
                    Key::Char('2') => Mode::Receive,
                    _ => Mode::Chat,
                };
            }
            Key::Enter if !self.editing && self.mode != Mode::Receive => self.editing = true,
            Key::Enter if self.mode == Mode::Receive => {
                return Some(if self.receiving {
                    Intent::StopReceive
                } else {
                    Intent::StartReceive
                });
            }
            Key::Char('r') if !self.editing && self.mode == Mode::Chat => {
                return Some(if self.receiving {
                    Intent::StopReceive
                } else {
                    Intent::StartReceive
                });
            }
            Key::Enter if self.editing => return self.submit(),
            Key::Backspace if self.editing => {
                self.input.pop();
            }
            Key::Char(character) if self.editing && !character.is_control() => {
                self.input.push(character);
            }
            _ => {}
        }
        None
    }

    pub fn update(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::Status(message) => self.log(message),
            RuntimeEvent::Error(message) => self.log(format!("Error: {message}")),
            RuntimeEvent::RxStarted {
                device,
                sample_rate,
            } => {
                self.receiving = true;
                self.input_device = Some(device.clone());
                self.log(format!("Listening on {device} at {sample_rate} Hz"));
            }
            RuntimeEvent::RxStopped => {
                self.receiving = false;
                self.input_level = 0.0;
                self.log("Listening stopped".to_owned());
            }
            RuntimeEvent::InputLevel(level) => self.input_level = level,
            RuntimeEvent::TransferStarted { name, size } => {
                self.transfer_progress = Some((0, 0));
                self.log(format!("Receiving {name} ({size} bytes)"));
            }
            RuntimeEvent::TransferProgress { completed, total } => {
                self.transfer_progress = Some((completed, total));
            }
            RuntimeEvent::FileReceived(path) => {
                self.received_path = Some(path.clone());
                self.transfer_progress = None;
                self.log(format!("Saved {}", path.display()));
            }
            RuntimeEvent::ChatReceived(text) => self.messages.push(ChatMessage {
                incoming: true,
                text,
            }),
            RuntimeEvent::TxStarted => {
                self.tx_progress = Some((0, 0));
                self.log("Acoustic transmission started".to_owned());
            }
            RuntimeEvent::TxProgress { current, total } => {
                self.tx_progress = Some((current, total));
            }
            RuntimeEvent::TxFinished => {
                self.tx_progress = None;
                self.log("Acoustic transmission stopped".to_owned());
            }
        }
    }

    fn submit(&mut self) -> Option<Intent> {
        let value = self.input.trim().to_owned();
        if value.is_empty() {
            self.log("Input is empty".to_owned());
            return None;
        }
        self.input.clear();
        match self.mode {
            Mode::Send => Some(Intent::SendFile(PathBuf::from(value))),
            Mode::Chat => {
                self.messages.push(ChatMessage {
                    incoming: false,
                    text: value.clone(),
                });
                Some(Intent::SendChat(value))
            }
            Mode::Receive => None,
        }
    }

    fn log(&mut self, message: String) {
        if self.logs.len() == 100 {
            self.logs.pop_front();
        }
        self.logs.push_back(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_and_navigation_produce_expected_intents() {
        let mut app = App::default();
        for character in "/tmp/a.txt".chars() {
            app.handle_key(Key::Char(character));
        }
        assert_eq!(
            app.handle_key(Key::Enter),
            Some(Intent::SendFile(PathBuf::from("/tmp/a.txt")))
        );
        app.handle_key(Key::Tab);
        assert_eq!(app.mode, Mode::Receive);
        assert_eq!(app.handle_key(Key::Enter), Some(Intent::StartReceive));
    }

    #[test]
    fn chat_is_added_to_history_without_a_file_intent() {
        let mut app = App {
            mode: Mode::Chat,
            input: "hello".to_owned(),
            ..App::default()
        };
        assert_eq!(
            app.handle_key(Key::Enter),
            Some(Intent::SendChat("hello".to_owned()))
        );
        assert_eq!(app.messages[0].text, "hello");
    }

    #[test]
    fn chat_listener_can_be_toggled_outside_input_mode() {
        let mut app = App {
            mode: Mode::Chat,
            editing: false,
            ..App::default()
        };
        assert_eq!(app.handle_key(Key::Char('r')), Some(Intent::StartReceive));
        app.receiving = true;
        assert_eq!(app.handle_key(Key::Char('r')), Some(Intent::StopReceive));
    }
}
