//! `Document` -> RTF (Rich Text Format).
//!
//! RTF is a plain-text format that Word, Pages and TextEdit all open, which
//! makes it a great, dependency-free "rich" export. We emit a minimal but
//! correct RTF 1.0 document: a font table, per-paragraph alignment and heading
//! sizing, and per-run bold/italic/underline/strike, with proper escaping of
//! `\\ { }` and non-ASCII characters.

use crate::model::{Align, BlockStyle, Document, Run};

pub fn to_rtf(doc: &Document) -> String {
    let mut s = String::from(
        "{\\rtf1\\ansi\\ansicpg1252\\deff0{\\fonttbl{\\f0\\fswiss Helvetica;}{\\f1\\fmodern Menlo;}}\n",
    );
    for p in &doc.paragraphs {
        s.push_str("\\pard");
        s.push_str(match p.align {
            Align::Left => "\\ql",
            Align::Center => "\\qc",
            Align::Right => "\\qr",
            Align::Justify => "\\qj",
        });
        // heading sizing (RTF font size is in half-points)
        let (size, heading_bold) = match p.style {
            BlockStyle::H1 => (Some(36), true),
            BlockStyle::H2 => (Some(30), true),
            BlockStyle::H3 => (Some(26), true),
            _ => (None, false),
        };
        if let Some(sz) = size {
            s.push_str(&format!("\\fs{}", sz));
        } else {
            s.push_str("\\fs24");
        }
        if matches!(p.style, BlockStyle::Quote) {
            s.push_str("\\li360\\ri360\\i");
        }
        let prefix = match p.style {
            BlockStyle::Bullet => "\\u8226? ",
            BlockStyle::Numbered => "",
            _ => "",
        };
        s.push(' ');
        s.push_str(prefix);
        for run in &p.runs {
            push_run(&mut s, run, heading_bold, matches!(p.style, BlockStyle::Code));
        }
        s.push_str("\\par\n");
    }
    s.push('}');
    s
}

fn push_run(s: &mut String, run: &Run, force_bold: bool, force_mono: bool) {
    let mono = run.code || force_mono;
    s.push('{');
    if mono {
        s.push_str("\\f1 ");
    }
    if let Some(sz) = run.size {
        s.push_str(&format!("\\fs{} ", (sz as u32) * 2)); // RTF size is half-points
    }
    if run.bold || force_bold {
        s.push_str("\\b ");
    }
    if run.italic {
        s.push_str("\\i ");
    }
    if run.underline {
        s.push_str("\\ul ");
    }
    if run.strike {
        s.push_str("\\strike ");
    }
    s.push_str(&escape(&run.text));
    s.push('}');
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '\n' => out.push_str("\\line "),
            c if (c as u32) < 128 => out.push(c),
            c => {
                // RTF unicode escape: \uN? with a 16-bit signed code unit
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{}?", *unit as i16));
                }
            }
        }
    }
    out
}
