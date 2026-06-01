//! Document statistics — the numbers a status bar wants.

use crate::model::Document;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Stats {
    pub words: usize,
    pub chars: usize,
    pub chars_no_spaces: usize,
    pub paragraphs: usize,
    pub reading_time_sec: usize,
}

/// Compute word/character/paragraph counts and an estimated reading time
/// (at 200 words per minute, the common average).
pub fn stats(doc: &Document) -> Stats {
    let text = doc.plain_text();
    let words = text.split_whitespace().filter(|w| !w.is_empty()).count();
    let chars = text.chars().count();
    let chars_no_spaces = text.chars().filter(|c| !c.is_whitespace()).count();
    let paragraphs = doc
        .paragraphs
        .iter()
        .filter(|p| !p.plain_text().trim().is_empty())
        .count();
    let reading_time_sec = ((words as f64 / 200.0) * 60.0).ceil() as usize;
    Stats { words, chars, chars_no_spaces, paragraphs, reading_time_sec }
}
