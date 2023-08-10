use std::io::stdout;
use crossterm::{terminal::{self, ClearType}, event, execute};
use crate::error::Result;

enum UIState {
    None,
    Tapping,
    Selecting
}

pub struct UI {
    input: String,
    cursor: usize,
    state: UIState
}

fn clear_screen() -> Result<()> {
    execute!(stdout(), terminal::Clear(ClearType::All))?;
    Ok(())
}

impl UI {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            cursor: 0,
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