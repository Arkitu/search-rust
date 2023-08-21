use std::{io::stdout, time::Duration, path::{PathBuf, Path}, thread, sync::{atomic::{AtomicU16, Ordering}, Arc, RwLock}};
use crossterm::{terminal::{self, ClearType}, event::{self, KeyEvent, Event, KeyCode}, execute, cursor, style::{Print, Stylize}};
use crate::{error::{Result, Error}, rank::RankResult};
use crate::rank::Ranker;
pub mod visual_pack;
use visual_pack::{VisualPack, VisualPackChars};
use dirs::home_dir;

#[derive(Clone)]
enum QuittingReason {
    Success(PathBuf),
    UserAbort,
    Error(Arc<Error>)
}

#[derive(Clone)]
enum UIState {
    None,
    Searching,
    // Path chosen
    Quitting(QuittingReason)
}

pub struct UI {
    input: Arc<RwLock<String>>,
    display_input: Arc<RwLock<String>>,
    cursor: [Arc<AtomicU16>; 2],
    input_offset: u16,
    state: Arc<RwLock<UIState>>,
    vp: VisualPack,
    results: Arc<RwLock<Vec<RankResult>>>
}

impl UI {
    pub fn new(visual_pack: VisualPack) -> Result<Self> {
        let input_offset = (visual_pack.get_symbol(VisualPackChars::SearchBarLeft).chars().count()+1) as u16;
        Ok(Self {
            input: Arc::new(RwLock::new(String::new())),
            display_input: Arc::new(RwLock::new(String::new())),
            cursor: [Arc::new(AtomicU16::new(0)), Arc::new(AtomicU16::new(0))],
            input_offset,
            state: Arc::new(RwLock::new(UIState::None)),
            vp: visual_pack,
            results: Arc::new(RwLock::new(Vec::new()))
        })
    }

    pub fn default() -> Result<Self> {
        Self::new(VisualPack::ExtendedUnicode)
    }

    pub fn run(&mut self) -> Result<Option<PathBuf>> {
        self.init()?;
        *self.state.write()? = UIState::Searching;

        let input_offset = self.input_offset;

        // process input
        let input = self.input.clone();
        let display_input = self.display_input.clone();
        let state = self.state.clone();
        let cursor = self.cursor.clone();
        let results = self.results.clone();
        thread::spawn(move || {
            loop {
                if let Err(e) = Self::process_input(&input, &display_input, &state, &cursor, &results) {
                    *state.write().unwrap() = UIState::Quitting(QuittingReason::Error(Arc::new(e)));
                }
            }
        });

        // rank
        let state = self.state.clone();
        let results = self.results.clone();
        let input = self.input.clone();
        thread::spawn(move || {
            let mut ranker = Ranker::new().unwrap();
            ranker.init().unwrap();
            loop {
                if let Err(e) = Self::rank(&results, &input, &mut ranker) {
                    *state.write().unwrap() = UIState::Quitting(QuittingReason::Error(Arc::new(e)));
                }
            }
        });

        // render
        let state = self.state.clone();
        let vp = self.vp.clone();
        let display_input = self.display_input.clone();
        let results = self.results.clone();
        let cursor = self.cursor.clone();
        thread::spawn(move || {
            let mut writer = Writer::new();
            loop {
                if let Err(e) = Self::render(vp, &mut writer, &display_input, &results, &cursor, input_offset) {
                    *state.write().unwrap() = UIState::Quitting(QuittingReason::Error(Arc::new(e)));
                }
            }
        });

        loop {
            if let UIState::Quitting(qr) = (*self.state.read()?).clone() {
                Writer::clear_screen()?;
                match qr {
                    QuittingReason::Success(p) => return Ok(Some(p)),
                    QuittingReason::UserAbort => return Ok(None),
                    QuittingReason::Error(e) => return Err(e.into())
                }
            }
        }
    }

    fn init(&self) -> Result<()> {
        terminal::enable_raw_mode()?;
        execute!(stdout(), self.vp.get_cursor_style())?;
        Ok(())
    }

    fn process_input(input: &Arc<RwLock<String>>, display_input: &Arc<RwLock<String>>, state: &Arc<RwLock<UIState>>, cursor: &[Arc<AtomicU16>; 2], results: &Arc<RwLock<Vec<RankResult>>>) -> Result<()> {
        let mut display_input = display_input.write()?;
        let results = results.read()?;
        match Reader::process_keypress()? {
            UserAction::Quit => {
                *state.write()? = UIState::Quitting(QuittingReason::UserAbort);
                return Ok(());
            },
            UserAction::NewChar(c) => {
                input.write()?.insert(cursor[0].load(Ordering::Relaxed) as usize, c);
                cursor[0].fetch_add(1, Ordering::Release);
                cursor[1].store(0, Ordering::Release);
            },
            UserAction::Move(direction) => {
                match direction {
                    Direction::Up => if cursor[1].load(Ordering::Relaxed) > 0 {
                        cursor[1].fetch_sub(1, Ordering::Release);
                    },
                    Direction::Down => {cursor[1].fetch_add(1, Ordering::Release);},
                    Direction::Left => if cursor[0].load(Ordering::Relaxed) > 0 {
                            cursor[0].fetch_sub(1, Ordering::Release);
                    },
                    Direction::Right => {
                        if cursor[1].load(Ordering::Relaxed) != 0 {
                            *input.write()? = display_input.clone();
                            cursor[0].store(input.read()?.len() as u16, Ordering::Release);
                            cursor[1].store(0, Ordering::Release);
                        } else {
                            cursor[0].fetch_add(1, Ordering::Release);
                        }
                    }
                }
            },
            UserAction::DeleteChar => {
                if cursor[0].load(Ordering::Relaxed) > 0 {
                    input.write()?.remove((cursor[0].load(Ordering::Relaxed) - 1) as usize);
                    cursor[0].fetch_sub(1, Ordering::Release);
                }
                cursor[1].store(0, Ordering::Release);
            },
            UserAction::NextResult => {
                cursor[1].fetch_add(1, Ordering::Release);
                if cursor[1].load(Ordering::Relaxed) as usize > results.len() {
                    cursor[1].store(1, Ordering::Release);
                }
            },
            UserAction::GotoResult => {
                if cursor[1].load(Ordering::Relaxed) == 0 {
                    cursor[1].store(1, Ordering::Release);
                }
                *state.write()? = UIState::Quitting(QuittingReason::Success(results[(cursor[1].load(Ordering::Relaxed) as usize)-1].path.clone()));
                return Ok(());
            },
            UserAction::None => {}
        }

        // Check if cursor is out of bounds
        let input_len = input.read()?.len() as u16;
        if cursor[0].load(Ordering::Relaxed) >= input_len {
            cursor[0].store(input_len, Ordering::Release);
        }
        if cursor[1].load(Ordering::Relaxed) > results.len() as u16 {
            cursor[1].store(results.len() as u16, Ordering::Release);
        }

        *display_input = if cursor[1].load(Ordering::Relaxed) == 0 {
            input.read()?.to_string()
        } else {
            results[(cursor[1].load(Ordering::Relaxed)-1) as usize].path.display().to_string()
        };

        Ok(())
    }

    fn rank(results: &Arc<RwLock<Vec<RankResult>>>, input: &Arc<RwLock<String>>, ranker: &mut Ranker) -> Result<()> {
        let input = input.read()?.clone();
        let result_count = terminal::size()?.1 as usize - 3;

        *results.write()? = ranker.get_results(&input, result_count)?;
        Ok(())
    }

    fn render(vp: VisualPack, writer: &mut Writer, display_input: &Arc<RwLock<String>>, results: &Arc<RwLock<Vec<RankResult>>>, cursor: &[Arc<AtomicU16>; 2], input_offset: u16) -> Result<()> {
        let result_count = terminal::size()?.1 as usize - 3;

        let mut output_text = format!(" {}{}{}\r\n", vp.get_colored_symbol(VisualPackChars::SearchBarLeft), display_input.read()?, vp.get_colored_symbol(VisualPackChars::SearchBarRight));

        let current_path = Path::new(".").canonicalize()?;
        let current_path = current_path.to_str().unwrap_or("");

        let home_dir = match home_dir() {
            Some(p) => p.to_str().unwrap_or("").to_owned(),
            None => "".to_string()
        };

        for (i, result) in results.read()?.iter().enumerate() {
            let symbol = vp.get_colored_symbol(VisualPackChars::ResultLeft(result.source, result.is_dir()));
            let mut path = result.path.display().to_string();
            if path.starts_with(current_path) && path != current_path {
                path = path.replacen(current_path, ".", 1);
            } else if path.starts_with(&home_dir) && path != home_dir {
                path = path.replacen(&home_dir, "~", 1);
            }
            let mut line = format!("\r\n {} {}", symbol, path);
            if cursor[1].load(Ordering::Relaxed) == (i as u16+1) {
                line = line.on_dark_grey().to_string();
            }
            output_text.push_str(&line);
        }

        for _ in 0..result_count-results.read()?.len() {
            output_text.push('\n');
        }

        output_text.push_str(&format!("\r\n {}", current_path).on_dark_grey().to_string());

        writer.write(&output_text, [cursor[0].load(Ordering::Relaxed)+input_offset, 0])?;

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
    fn read_key() -> Result<Option<KeyEvent>> {
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(event) = event::read()? {
                return Ok(Some(event))
            }
        }
        Ok(None)
    }

    fn process_keypress() -> Result<UserAction> {
        if let Some(key) = Self::read_key()? {
            Ok(match key {
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
        } else {
            Ok(UserAction::None)
        }
    }
}

struct Writer {
    last_output: String,
    cursor: [u16; 2]
}
impl Writer {
    pub fn new() -> Self {
        Self {
            last_output: String::new(),
            cursor: [0, 0]
        }
    }
    pub fn clear_screen() -> Result<()> {
        execute!(stdout(), terminal::Clear(ClearType::All))?;
        execute!(stdout(), cursor::MoveTo(0, 0))?;
        Ok(())
    }
    pub fn write(&mut self, s: &str, cursor: [u16; 2]) -> Result<()> {
        if s != self.last_output {
            Self::clear_screen()?;
            execute!(stdout(), Print(s))?;
            execute!(stdout(), cursor::MoveTo(cursor[0], cursor[1]))?;
            self.last_output = s.to_owned();
            self.cursor = cursor;
        } else if cursor != self.cursor {
            execute!(stdout(), cursor::MoveTo(cursor[0], cursor[1]))?;
            self.cursor = cursor;
        }
        Ok(())
    }
}
