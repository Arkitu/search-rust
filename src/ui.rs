use std::{io::stdout, time::Duration, path::{PathBuf, Path}};
use crossterm::{terminal::{self, ClearType}, event::{self, KeyEvent, Event, KeyCode}, execute, cursor, style::{Print, Stylize}};
use crate::{error::Result, rank::RankResult};
use crate::rank::get_results;
pub mod visual_pack;
use visual_pack::{VisualPack, VisualPackChars};

enum UIState {
    None,
    Searching,
    Quitting
}

pub struct UI {
    input: String,
    display_input: String,
    output: Option<PathBuf>,
    cursor: [usize; 2],
    state: UIState,
    vp: VisualPack,
    results: Vec<RankResult>
}

impl UI {
    pub fn new(visual_pack: VisualPack) -> Self {
        Self {
            input: String::new(),
            display_input: String::new(),
            output: None,
            cursor: [visual_pack.get_symbol(VisualPackChars::SearchBarLeft).chars().count(), 0],
            state: UIState::None,
            vp: visual_pack,
            results: Vec::new()
        }
    }

    pub fn default() -> Self {
        Self::new(VisualPack::ExtendedUnicode)
    }

    pub fn run(&mut self) -> Result<Option<PathBuf>> {
        self.init()?;
        self.state = UIState::Searching;
        loop {
            self.render()?;
            if let UIState::Quitting = self.state {
                Writer::clear_screen()?;
                break;
            }
        }
        Ok(self.output.clone())
    }

    fn init(&self) -> Result<()> {
        terminal::enable_raw_mode()?;
        execute!(stdout(), self.vp.get_cursor_style())?;
        Ok(())
    }

    fn render(&mut self) -> Result<()> {
        let input_offset = self.vp.get_symbol(VisualPackChars::SearchBarLeft).chars().count();
        match Reader::process_keypress()? {
            UserAction::Quit => {
                self.state = UIState::Quitting;
                return Ok(());
            },
            UserAction::NewChar(c) => {
                self.input.insert(self.cursor[0]-input_offset, c);
                self.cursor[0] += 1;
                self.cursor[1] = 0;
            },
            UserAction::Move(direction) => {
                match direction {
                    Direction::Up => if self.cursor[1] != 0 {
                        self.cursor[1] -= 1
                    },
                    Direction::Down => self.cursor[1] += 1,
                    Direction::Left => if self.cursor[0] > input_offset {
                            self.cursor[0] -= 1
                    },
                    Direction::Right => {
                        if self.cursor[1] != 0 {
                            self.input = self.display_input.clone();
                            self.cursor[0] = self.input.len()+input_offset;
                            self.cursor[1] = 0;
                        } else {
                            self.cursor[0] += 1
                        }
                    }
                }
            },
            UserAction::DeleteChar => {
                if self.cursor[0] > input_offset {
                    self.input.remove(self.cursor[0] - 1 - input_offset);
                    self.cursor[0] -= 1;
                }
                self.cursor[1] = 0;
            },
            UserAction::NextResult => {
                self.cursor[1] += 1;
                if self.cursor[1] > self.results.len() {
                    self.cursor[1] = 1;
                }
            },
            UserAction::GotoResult => {
                if self.cursor[1] == 0 {
                    self.cursor[1] = 1;
                }
                self.output = Some(self.results[self.cursor[1]-1].path.clone());
                self.state = UIState::Quitting;
                return Ok(());
            }
            UserAction::None => {}
        }

        let result_count = terminal::size()?.1 as usize - 2;

        self.results = get_results(&self.input, result_count)?;

        // Check if cursor is out of bounds
        if self.cursor[0] >= (self.input.len()+input_offset) {
            self.cursor[0] = self.input.len()+input_offset;
        }
        if self.cursor[1] > self.results.len() {
            self.cursor[1] = self.results.len();
        }

        self.display_input = if self.cursor[1] == 0 {
            self.input.to_string()
        } else {
            self.results[self.cursor[1]-1].path.display().to_string()
        };

        let mut output_text = format!("{}{}{}\r\n", self.vp.get_symbol(VisualPackChars::SearchBarLeft), self.display_input, self.vp.get_symbol(VisualPackChars::SearchBarRight));

        let current_path = Path::new(".").canonicalize()?;
        let current_path = current_path.to_str().unwrap_or("");
        for (i, result) in self.results.iter().enumerate() {
            let symbol = self.vp.get_symbol(result.result_type.into());
            let mut path = result.path.display().to_string();
            if path.starts_with(current_path) {
                path = path.replacen(current_path, ".", 1);
            }
            let mut line = format!("\r\n {} {}", symbol, path);
            if self.cursor[1] == (i+1) {
                line = line.on_white().to_string();
            }
            output_text.push_str(&line);
        }

        Writer::write(&output_text, [self.cursor[0], 0])?;

        Ok(())
    }
}

impl Drop for UI {
    fn drop(&mut self) {
        terminal::disable_raw_mode().expect("Unable to disable raw mode");
        execute!(stdout(), cursor::SetCursorStyle::DefaultUserShape).expect("Unable to turn the cursor back to normal");
    }
}

enum Direction {
    Up,
    Down,
    Left,
    Right
}
enum UserAction {
    Quit,
    NewChar(char),
    DeleteChar,
    Move(Direction),
    NextResult,
    GotoResult,
    None
}

struct Reader;
impl Reader {
    fn read_key() -> Result<KeyEvent> {
        loop {
            if event::poll(Duration::from_millis(500))? {
                if let Event::Key(event) = event::read()? {
                    return Ok(event);
                }
            }
        }
    }

    fn process_keypress() -> Result<UserAction> {
        return Ok(match Self::read_key()? {
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: event::KeyModifiers::CONTROL,
                ..
            } => UserAction::Quit,

            KeyEvent { code: KeyCode::Char(c), .. } => UserAction::NewChar(c),

            KeyEvent { code: KeyCode::Left, .. } => UserAction::Move(Direction::Left),
            KeyEvent { code: KeyCode::Right, .. } => UserAction::Move(Direction::Right),
            KeyEvent { code: KeyCode::Up, .. } => UserAction::Move(Direction::Up),
            KeyEvent { code: KeyCode::Down, .. } => UserAction::Move(Direction::Down),

            KeyEvent { code: KeyCode::Enter, ..} => UserAction::GotoResult,

            KeyEvent { code: KeyCode::Backspace, .. } => UserAction::DeleteChar,

            KeyEvent { code: KeyCode::Tab, .. } => UserAction::NextResult,
            _ => UserAction::None
        })
    }
}

struct Writer;
impl Writer {
    fn clear_screen() -> Result<()> {
        execute!(stdout(), terminal::Clear(ClearType::All))?;
        execute!(stdout(), cursor::MoveTo(0, 0))?;
        Ok(())
    }
    fn write(s: &str, cursor: [usize; 2]) -> Result<()> {
        Self::clear_screen()?;
        execute!(stdout(), Print(s))?;
        execute!(stdout(), cursor::MoveTo(cursor[0] as u16, cursor[1] as u16))?;
        Ok(())
    }
}
