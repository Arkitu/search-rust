use crossterm::{cursor, style::{Stylize, StyledContent}};

use crate::rank::RankSource;

#[derive(Copy, Clone)]
pub enum VisualPackChars {
    /// (source, is_dir)
    ResultLeft(RankSource, bool),
    SearchBarLeft,
    SearchBarRight
}

#[derive(Copy, Clone)]
pub enum VisualPack {
    ExtendedUnicode,
    CommonUnicode,
    Ascii
}

impl VisualPack {
    pub fn get_symbol(&self, symbol: VisualPackChars) -> &'static str {
        match self {
            VisualPack::ExtendedUnicode => match symbol {
                VisualPackChars::ResultLeft(_, d) => if d {"֎"} else {"۞"},
                VisualPackChars::SearchBarLeft => "ᗧ ",
                VisualPackChars::SearchBarRight => " ᗤ"
            },
            VisualPack::CommonUnicode => match symbol {
                VisualPackChars::ResultLeft(_, d) => if d {"▸"} else {"▪"},
                VisualPackChars::SearchBarLeft => "[",
                VisualPackChars::SearchBarRight => " ]"
            },
            VisualPack::Ascii => match symbol {
                VisualPackChars::ResultLeft(_, d) => if d {">"} else {"*"},
                VisualPackChars::SearchBarLeft => "[",
                VisualPackChars::SearchBarRight => " ]"
            }
        }
    }
    pub fn get_cursor_style(&self) -> cursor::SetCursorStyle {
        match self {
            VisualPack::ExtendedUnicode => cursor::SetCursorStyle::BlinkingBar,
            VisualPack::CommonUnicode => cursor::SetCursorStyle::BlinkingBlock,
            VisualPack::Ascii => cursor::SetCursorStyle::BlinkingBlock,
            _ => cursor::SetCursorStyle::DefaultUserShape
        }
    }
    pub fn get_colored_symbol(&self, symbol: VisualPackChars) -> StyledContent<&str> {
        let s = self.get_symbol(symbol);
        match self {
            VisualPack::ExtendedUnicode => match symbol {
                VisualPackChars::SearchBarLeft | VisualPackChars::SearchBarRight => s.bold(),
                VisualPackChars::ResultLeft(so, _) => match so {
                    RankSource::ExactPath => s.green(),
                    RankSource::StartLikePath => s.blue(),
                    RankSource::InDir => s.dark_blue(),
                    RankSource::Semantic => s.yellow()
                }
            },
            _ => s.stylize()
        }
    }
}
