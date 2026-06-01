//! The document model.
//!
//! A `Document` is a list of `Paragraph`s; each paragraph has a block `style`
//! (Normal, headings, quote, list item, code) and an `align`, and holds a list
//! of `Run`s — contiguous spans of text that share inline formatting (bold,
//! italic, underline, …). This is the single source of truth that every file
//! format is converted to and from, and it is exactly what crosses the wire to
//! the UI as JSON, so the front-end and the engine never disagree about what a
//! document *is*.

use serde::{Deserialize, Serialize};

fn is_false(b: &bool) -> bool {
    !*b
}

/// Inline character formatting for a run of text.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub text: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub underline: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub strike: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub code: bool,
}

impl Run {
    pub fn new(text: impl Into<String>) -> Self {
        Run { text: text.into(), ..Default::default() }
    }
    pub fn bold(mut self) -> Self { self.bold = true; self }
    pub fn italic(mut self) -> Self { self.italic = true; self }
    pub fn underline(mut self) -> Self { self.underline = true; self }
    pub fn strike(mut self) -> Self { self.strike = true; self }
    pub fn code(mut self) -> Self { self.code = true; self }
    /// True when this run carries no inline formatting.
    pub fn is_plain(&self) -> bool {
        !(self.bold || self.italic || self.underline || self.strike || self.code)
    }
}

/// The block-level role of a paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockStyle {
    Normal,
    H1,
    H2,
    H3,
    Quote,
    Bullet,
    Numbered,
    Code,
}

impl Default for BlockStyle {
    fn default() -> Self { BlockStyle::Normal }
}

/// Paragraph alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Align {
    Left,
    Center,
    Right,
    Justify,
}

impl Default for Align {
    fn default() -> Self { Align::Left }
}

/// One paragraph: a block style, an alignment, and the runs it contains.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Paragraph {
    #[serde(default)]
    pub style: BlockStyle,
    #[serde(default)]
    pub align: Align,
    #[serde(default)]
    pub runs: Vec<Run>,
}

impl Paragraph {
    pub fn new(style: BlockStyle) -> Self {
        Paragraph { style, ..Default::default() }
    }
    pub fn with_text(style: BlockStyle, text: impl Into<String>) -> Self {
        let mut p = Paragraph::new(style);
        p.runs.push(Run::new(text));
        p
    }
    pub fn push(&mut self, run: Run) {
        self.runs.push(run);
    }
    /// The paragraph's text with all formatting stripped.
    pub fn plain_text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
}

/// A whole document.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Document {
    #[serde(default)]
    pub paragraphs: Vec<Paragraph>,
}

impl Document {
    pub fn new() -> Self {
        Document::default()
    }
    pub fn push(&mut self, p: Paragraph) {
        self.paragraphs.push(p);
    }
    /// Whole-document plain text (paragraphs joined by newlines).
    pub fn plain_text(&self) -> String {
        self.paragraphs
            .iter()
            .map(|p| p.plain_text())
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn is_empty(&self) -> bool {
        self.paragraphs.is_empty()
            || self.paragraphs.iter().all(|p| p.plain_text().trim().is_empty())
    }
}
