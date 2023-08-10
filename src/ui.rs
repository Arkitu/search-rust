use std::{io::stdout, time::Duration, result};
use crossterm::{terminal::{self, ClearType}, event::{self, KeyEvent, Event, KeyCode}, execute, cursor, style};
use crate::error::Result;
use crate::rank::get_results;

enum UIState {
    None,
    Searching,
    Quitting
}

pub struct UI {
    input: String,
    cursor: [usize; 2],
    state: UIState
}

impl UI {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            cursor: [2, 0],
            state: UIState::None
        }
    }

    fn init() -> Result<()> {
        terminal::enable_raw_mode()?;
        Ok(())
    }

    fn render(&mut self) -> Result<()> {
        match Reader::process_keypress()? {
            UserAction::Quit => {
                self.state = UIState::Quitting;
                return Ok(());
            },
            UserAction::NewChar(c) => {
                self.input.insert(self.cursor[0]-2, c);
                self.cursor[0] += 1;
            },
            UserAction::Move(direction) => {
                match direction {
                    Direction::Up => if self.cursor[1] != 0 {
                        self.cursor[1] -= 1
                    },
                    Direction::Down => self.cursor[1] += 1,
                    Direction::Left => if self.cursor[0] > 2 {
                        self.cursor[0] -= 1
                    },
                    Direction::Right => self.cursor[0] += 1
                }
            },
            UserAction::DeleteChar => {
                if self.cursor[0] > 2 {
                    self.input.remove(self.cursor[0] - 3);
                    self.cursor[0] -= 1;
                }
            },
            UserAction::None => {}
        }

        // Check if cursor is out of bounds
        if self.cursor[0] >= (self.input.len()+2) {
            self.cursor[0] = self.input.len()+2;
        }
        if self.cursor[1] > 0 {
            self.cursor[1] = 0;
        }

        let result_count = terminal::size()?.1 as usize - 2;

        let results = get_results(&self.input, result_count)?;

        let mut output_text = format!("[ {} ]\r\n", self.input);

        for result in results {
            output_text.push_str(&format!("\r\n > {}", result));
        }

        Writer::write(&output_text, self.cursor)?;

        Ok(())
    }

    pub fn run(&mut self) -> Result<()> {
        Self::init()?;
        self.state = UIState::Searching;
        loop {
            self.render()?;
            if let UIState::Quitting = self.state {
                Writer::clear_screen()?;
                break;
            }
        }
        Ok(())
    }
}

impl Drop for UI {
    fn drop(&mut self) {
        terminal::disable_raw_mode().expect("Unable to disable raw mode")
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

            KeyEvent { code: KeyCode::Enter, ..} => UserAction::Move(Direction::Down),

            KeyEvent { code: KeyCode::Backspace, .. } => UserAction::DeleteChar,
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
        execute!(stdout(), style::Print(s))?;
        execute!(stdout(), cursor::MoveTo(cursor[0] as u16, cursor[1] as u16))?;
        Ok(())
    }
}