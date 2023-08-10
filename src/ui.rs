use std::{io::stdout, time::Duration};
use crossterm::{terminal::{self, ClearType}, event::{self, KeyEvent, Event, KeyCode}, execute};
use crate::error::Result;

enum UIState {
    None,
    Tapping,
    Selecting
}

pub struct UI {
    input: Input,
    state: UIState
}

fn clear_screen() -> Result<()> {
    execute!(stdout(), terminal::Clear(ClearType::All))?;
    Ok(())
}

impl UI {
    pub fn new() -> Self {
        Self {
            input: Input::new(),
            state: UIState::None
        }
    }

    fn init() -> Result<()> {
        terminal::enable_raw_mode()?;
        Ok(())
    }

    fn render(&self) {
        
    }

    pub fn run(&mut self) -> Result<()> {
        Self::init()?;
        loop {

        }
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
    Move(Direction),
    None
}

struct Input {
    input: String,
    cursor: usize
}
impl Input {
    fn new() -> Self {
        Self {
            input: String::new(),
            cursor: 0
        }
    }

    fn read_key(&self) -> Result<KeyEvent> {
        loop {
            if event::poll(Duration::from_millis(500))? {
                if let Event::Key(event) = event::read()? {
                    return Ok(event);
                }
            }
        }
    }

    fn process_keypress(&self) -> Result<UserAction> {
        return Ok(match self.read_key()? {
            KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: event::KeyModifiers::CONTROL,
                ..
            } => UserAction::Quit,

            KeyEvent { code: KeyCode::Char(c), .. } => UserAction::NewChar(c),

            KeyEvent { code: KeyCode::Left, .. } => UserAction::Move(Direction::Left),
            KeyEvent { code: KeyCode::Right, .. } => UserAction::Move(Direction::Right),
            KeyEvent { code: KeyCode::Up, .. } => UserAction::Move(Direction::Up),
            KeyEvent { code: KeyCode::Down, .. } => UserAction::Move(Direction::Down),

            KeyEvent { code: KeyCode::Enter, .. } => UserAction::Move(Direction::Down),

            _ => UserAction::None
        })
    }
}