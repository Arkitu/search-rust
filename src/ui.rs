use std::{io::stdout, time::Duration, path::{PathBuf, Path}, sync::{atomic::{AtomicU16, Ordering}, Arc}};
use crossterm::{terminal::{self, ClearType}, event::{self, KeyEvent, Event, KeyCode}, execute, cursor, style::{Print, Stylize}};
use tokio::sync::RwLock;
use crate::rank::{RankResult, RankSource};
use crate::rank::Ranker;
pub mod visual_pack;
use visual_pack::{VisualPack, VisualPackChars};
use dirs::home_dir;

#[derive(Clone)]
enum QuittingReason {
    Success(PathBuf),
    UserAbort
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
    result_offset: u16,
    state: Arc<RwLock<UIState>>,
    vp: VisualPack,
    results: Arc<RwLock<Vec<RankResult>>>,
    db_path: Option<String>,
    cache_path: Option<String>
}

impl UI {
    pub fn new(visual_pack: VisualPack, db_path: Option<String>, cache_path: Option<String>) -> Self {
        let input_offset = (visual_pack.get_symbol(VisualPackChars::SearchBarLeft).chars().count()+1) as u16;
        let result_offset = (visual_pack.get_symbol(VisualPackChars::ResultLeft(RankSource::ExactPath, false)).chars().count()+2) as u16;
        Self {
            input: Arc::new(RwLock::new(String::new())),
            display_input: Arc::new(RwLock::new(String::new())),
            cursor: [Arc::new(AtomicU16::new(0)), Arc::new(AtomicU16::new(0))],
            input_offset,
            result_offset,
            state: Arc::new(RwLock::new(UIState::None)),
            vp: visual_pack,
            results: Arc::new(RwLock::new(Vec::new())),
            db_path,
            cache_path
        }
    }

    pub fn default() -> Self {
        Self::new(VisualPack::ExtendedUnicode, None, None)
    }

    pub async fn run(&mut self) -> Option<PathBuf> {
        self.init();
        *self.state.write().await = UIState::Searching;

        let input_offset = self.input_offset;

        // process input
        let input = self.input.clone();
        let display_input = self.display_input.clone();
        let state = self.state.clone();
        let cursor = self.cursor.clone();
        let results = self.results.clone();
        tokio::spawn(async move {
            loop {
                Self::process_input(&input, &display_input, &state, &cursor, &results).await;
            }
        });

        // rank
        let results = self.results.clone();
        let input = self.input.clone();
        let db_path = self.db_path.clone();
        let cache_path = self.cache_path.clone();
        tokio::spawn(async move {
            let mut ranker = Ranker::new(db_path, cache_path).await;
            ranker.init();
            loop {
                Self::rank(&results, &input, &mut ranker).await
            }
        });

        // render
        let vp = self.vp.clone();
        let display_input = self.display_input.clone();
        let results = self.results.clone();
        let cursor = self.cursor.clone();
        let result_offset = self.result_offset;
        tokio::spawn(async move {
            let mut writer = Writer::new();
            loop {
                Self::render(vp, &mut writer, &display_input, &results, &cursor, input_offset, result_offset).await;
            }
        });

        loop {
            if let UIState::Quitting(qr) = (*self.state.read().await).clone() {
                Writer::clear_screen();
                match qr {
                    QuittingReason::Success(p) => return Some(p),
                    QuittingReason::UserAbort => return None
                }
            }
        }
    }

    fn init(&self) {
        terminal::enable_raw_mode().expect("Unable to enable raw mode");
        execute!(stdout(), self.vp.get_cursor_style()).expect("Unable to set cursor style");
    }

    async fn process_input(input: &Arc<RwLock<String>>, display_input: &Arc<RwLock<String>>, state: &Arc<RwLock<UIState>>, cursor: &[Arc<AtomicU16>; 2], results: &Arc<RwLock<Vec<RankResult>>>) {
        let mut display_input = display_input.write().await;
        let results = results.read().await;
        match Reader::process_keypress() {
            UserAction::Quit => {
                *state.write().await = UIState::Quitting(QuittingReason::UserAbort);
                return;
            },
            UserAction::NewChar(c) => {
                let mut input = input.write().await;
                let mut new_input = input.chars().collect::<Vec<char>>();
                new_input.insert(cursor[0].load(Ordering::Relaxed) as usize, c);
                *input = new_input.iter().collect::<String>();
                // let cursor0 = cursor[0].load(Ordering::Relaxed) as usize;
                // for (i, char) in input.chars().enumerate() {
                //     if i == cursor0 {
                //         new_input.push(c);
                //     }
                //     new_input.push(char);
                // }
                // *input = new_input;
                cursor[0].fetch_add(1, Ordering::Release);
                cursor[1].store(0, Ordering::Release);
            },
            UserAction::Move(direction) => {
                match direction {
                    Direction::Up => if cursor[1].load(Ordering::Relaxed) > 0 {
                        cursor[1].fetch_sub(1, Ordering::Release);
                    },
                    Direction::Down => {cursor[1].fetch_add(1, Ordering::Release);},
                    Direction::Left => if cursor[1].load(Ordering::Relaxed) > 0 {
                        match results[(cursor[1].load(Ordering::Relaxed)-1) as usize].path.parent() {
                            Some(target) => {
                                *input.write().await = String::new();
                                cursor[0].store(0, Ordering::Release);
                                cursor[1].store(0, Ordering::Release);
                                std::env::set_current_dir(target).expect("Can't change current dir");
                            },
                            None => {}
                        }
                    } else if cursor[0].load(Ordering::Relaxed) > 0 {
                        cursor[0].fetch_sub(1, Ordering::Release);
                    },
                    Direction::Right => {
                        if cursor[1].load(Ordering::Relaxed) > 0 {
                            let target = &results[(cursor[1].load(Ordering::Relaxed)-1) as usize].path;
                            *input.write().await = String::new();
                            cursor[0].store(0, Ordering::Release);
                            cursor[1].store(0, Ordering::Release);
                            if target.is_dir() {
                                std::env::set_current_dir(target).expect("Can't change current dir");
                            }
                        } else {
                            cursor[0].fetch_add(1, Ordering::Release);
                        }
                    }
                }
            },
            UserAction::DeleteChar => {
                if cursor[0].load(Ordering::Relaxed) > 0 {
                    let mut input = input.write().await;
                    let mut new_input = input.chars().collect::<Vec<char>>();
                    new_input.remove(cursor[0].load(Ordering::Relaxed) as usize - 1);
                    *input = new_input.iter().collect::<String>();
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
                *state.write().await = UIState::Quitting(QuittingReason::Success(results[(cursor[1].load(Ordering::Relaxed) as usize)-1].path.clone()));
                return;
            },
            UserAction::None => {}
        }

        // Check if cursor is out of bounds
        let input_len = input.read().await.len() as u16;
        if cursor[0].load(Ordering::Relaxed) >= input_len {
            cursor[0].store(input_len, Ordering::Release);
        }
        if cursor[1].load(Ordering::Relaxed) > results.len() as u16 {
            cursor[1].store(results.len() as u16, Ordering::Release);
        }

        *display_input = if cursor[1].load(Ordering::Relaxed) == 0 {
            input.read().await.to_string()
        } else {
            results[(cursor[1].load(Ordering::Relaxed)-1) as usize].path.display().to_string()
        };
    }

    async fn rank(results: &Arc<RwLock<Vec<RankResult>>>, input: &Arc<RwLock<String>>, ranker: &mut Ranker) {
        let input = input.read().await.clone();
        let result_count = terminal::size().expect("Can't get terminal size").1 as usize - 3;

        *results.write().await = ranker.get_results(&input, result_count).await;
    }

    async fn render(vp: VisualPack, writer: &mut Writer, display_input: &Arc<RwLock<String>>, results: &Arc<RwLock<Vec<RankResult>>>, cursor: &[Arc<AtomicU16>; 2], input_offset: u16, result_offset: u16) {
        let terminal_size = terminal::size().expect("Can't get terminal size");
        let result_count = terminal_size.1 as usize - 3;

        let mut output_text = format!(" {}{}{}\r\n", vp.get_colored_symbol(VisualPackChars::SearchBarLeft), display_input.read().await, vp.get_colored_symbol(VisualPackChars::SearchBarRight));

        let current_dir = std::env::current_dir().expect("Can't get current dir");
        let current_path = current_dir.to_str().unwrap_or("");

        let home_dir = match home_dir() {
            Some(p) => p.to_str().unwrap_or("").to_owned(),
            None => "".to_string()
        };

        let terminal_width = terminal_size.0;
        for (i, result) in results.read().await.iter().enumerate() {
            let symbol = vp.get_colored_symbol(VisualPackChars::ResultLeft(result.source, result.is_dir()));
            let mut path = result.path.display().to_string();
            if path.starts_with(current_path) && path != current_path && current_dir != Path::new("/") {
                path = path.replacen(current_path, ".", 1);
            } else if path.starts_with(&home_dir) && path != home_dir {
                path = path.replacen(&home_dir, "~", 1);
            }

            // If path is to long to fit on one line, replace the start with "…"
            if path.len() as u16 + result_offset > terminal_width {
                path = "…".to_string() + &path[(path.len() as u16 + result_offset - terminal_width + 1) as usize..];
            }

            let mut line = format!("\r\n {} {}", symbol, path);
            if cursor[1].load(Ordering::Relaxed) == (i as u16+1) {
                line = line.on_dark_grey().to_string();
            }
            output_text.push_str(&line);
        }

        for _ in 0..result_count-results.read().await.len() {
            output_text.push('\n');
        }

        output_text.push_str(&format!("\r\n {}", current_path).on_dark_grey().to_string());

        writer.write(&output_text, [cursor[0].load(Ordering::Relaxed)+input_offset, 0]);
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
    fn read_key() -> Option<KeyEvent> {
        if event::poll(Duration::from_millis(0)).expect("Can't poll event") {
            if let Event::Key(event) = event::read().expect("Can't read event") {
                return Some(event)
            }
        }
        None
    }

    fn process_keypress() -> UserAction {
        if let Some(key) = Self::read_key() {
            match key {
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
            }
        } else {
            UserAction::None
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
    pub fn clear_screen() {
        execute!(stdout(), terminal::Clear(ClearType::All)).expect("Can't clear screen");
        execute!(stdout(), cursor::MoveTo(0, 0)).expect("Can't move cursor to 0, 0");
    }
    pub fn write(&mut self, s: &str, cursor: [u16; 2]) {
        if s != self.last_output {
            Self::clear_screen();
            execute!(stdout(), Print(s)).expect("Can't print");
            execute!(stdout(), cursor::MoveTo(cursor[0], cursor[1])).expect("Can't move cursor");
            self.last_output = s.to_owned();
            self.cursor = cursor;
        } else if cursor != self.cursor {
            execute!(stdout(), cursor::MoveTo(cursor[0], cursor[1])).expect("Can't move cursor");
            self.cursor = cursor;
        }
    }
}
