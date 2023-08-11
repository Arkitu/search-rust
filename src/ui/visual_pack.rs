use crossterm::cursor;

use crate::rank::ResultType;

pub enum VisualPackChars {
    ResultTypeDir,
    ResultTypeFile,
    SearchBarLeft,
    SearchBarRight
}
impl From<ResultType> for VisualPackChars {
    fn from(result_type: ResultType) -> Self {
        match result_type {
            ResultType::Dir => Self::ResultTypeDir,
            ResultType::File => Self::ResultTypeFile
        }
    }
}

pub enum VisualPack {
    ExtendedUnicode,
    CommonUnicode,
    Ascii
}

impl VisualPack {
    pub fn get_symbol(&self, symbol: VisualPackChars) -> &'static str {
        match self {
            VisualPack::ExtendedUnicode => match symbol {
                VisualPackChars::ResultTypeDir => "֎",
                VisualPackChars::ResultTypeFile => "۞",
                VisualPackChars::SearchBarLeft => "ᗧ ",
                VisualPackChars::SearchBarRight => " ᗤ"
            },
            VisualPack::CommonUnicode => match symbol {
                VisualPackChars::ResultTypeDir => "▸",
                VisualPackChars::ResultTypeFile => "▪",
                VisualPackChars::SearchBarLeft => "[",
                VisualPackChars::SearchBarRight => " ]"
            },
            VisualPack::Ascii => match symbol {
                VisualPackChars::ResultTypeDir => ">",
                VisualPackChars::ResultTypeFile => "*",
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
}