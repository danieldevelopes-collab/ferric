//! `Document` -> PDF, laid out with the pure-Rust `printpdf` crate.
//!
//! This is a real, paginated text layout — not a screenshot. It word-wraps each
//! paragraph to the page width, honours per-run font family / weight / slant and
//! per-run or per-heading size, indents quotes, prefixes list items, and starts
//! a fresh page when it runs out of room. Wrapping widths come from the standard
//! Helvetica metric table (good enough for clean line breaks across the built-in
//! PDF fonts). For pixel-perfect output the app also offers the system Print /
//! "Save as PDF" path, which renders the page exactly as you see it.

use crate::model::{Align, BlockStyle, Document, Paragraph};
use printpdf::{BuiltinFont, IndirectFontRef, Mm, PdfDocument, PdfLayerReference};

const PAGE_W: f64 = 215.9; // US Letter, millimetres
const PAGE_H: f64 = 279.4;
const MARGIN: f64 = 20.0;
const PT_TO_MM: f64 = 0.352_778;

#[inline]
fn mm(v: f64) -> Mm {
    Mm(v as f32)
}

struct Fonts {
    sans: [IndirectFontRef; 4], // [reg, bold, italic, bolditalic]
    serif: [IndirectFontRef; 4],
    mono: [IndirectFontRef; 2], // [reg, bold]
}

impl Fonts {
    fn pick(&self, family: &str, code: bool, bold: bool, italic: bool) -> &IndirectFontRef {
        let f = family.to_ascii_lowercase();
        let idx = (bold as usize) | ((italic as usize) << 1);
        if code || f.contains("courier") || f.contains("mono") || f.contains("menlo")
            || f.contains("consolas")
        {
            &self.mono[bold as usize]
        } else if f.contains("times") || f.contains("georgia") || f.contains("garamond")
            || f.contains("serif")
        {
            &self.serif[idx]
        } else {
            &self.sans[idx]
        }
    }
}

pub fn to_pdf(doc: &Document) -> Result<Vec<u8>, String> {
    let (pdf, page1, layer1) =
        PdfDocument::new("ferric document", mm(PAGE_W), mm(PAGE_H), "Layer 1");
    let add = |f: BuiltinFont| pdf.add_builtin_font(f).map_err(|e| e.to_string());
    let fonts = Fonts {
        sans: [
            add(BuiltinFont::Helvetica)?, add(BuiltinFont::HelveticaBold)?,
            add(BuiltinFont::HelveticaOblique)?, add(BuiltinFont::HelveticaBoldOblique)?,
        ],
        serif: [
            add(BuiltinFont::TimesRoman)?, add(BuiltinFont::TimesBold)?,
            add(BuiltinFont::TimesItalic)?, add(BuiltinFont::TimesBoldItalic)?,
        ],
        mono: [add(BuiltinFont::Courier)?, add(BuiltinFont::CourierBold)?],
    };

    let mut layer = pdf.get_page(page1).get_layer(layer1);
    let mut top = MARGIN; // distance from the top of the current page, mm

    for p in &doc.paragraphs {
        layout_paragraph(&pdf, &mut layer, &fonts, p, &mut top);
    }

    let mut bytes = Vec::new();
    {
        let mut buf = std::io::BufWriter::new(&mut bytes);
        pdf.save(&mut buf).map_err(|e| e.to_string())?;
    }
    Ok(bytes)
}

fn layout_paragraph(
    pdf: &printpdf::PdfDocumentReference,
    layer: &mut PdfLayerReference,
    fonts: &Fonts,
    p: &Paragraph,
    top: &mut f64,
) {
    let size = effective_size(p);
    let line_h = size * PT_TO_MM * 1.42;
    let heading_bold = matches!(p.style, BlockStyle::H1 | BlockStyle::H2 | BlockStyle::H3);
    let indent = if matches!(p.style, BlockStyle::Quote) { 8.0 } else { 0.0 };
    let left = MARGIN + indent;
    let right = PAGE_W - MARGIN;

    // space before headings
    if heading_bold {
        *top += size * PT_TO_MM * 0.5;
    }

    // build the token stream (word, font, width-in-mm), incl. list prefix
    let mut tokens: Vec<(String, IndirectFontRef, f64, bool)> = Vec::new(); // text, font, width, is_space
    let prefix = match p.style {
        BlockStyle::Bullet => Some("•  ".to_string()),
        BlockStyle::Numbered => Some("1.  ".to_string()),
        _ => None,
    };
    if let Some(pre) = prefix {
        let font = fonts.pick("", false, false, false).clone();
        let w = text_width(&pre, &"", false, size);
        tokens.push((pre, font, w, false));
    }
    for run in &p.runs {
        let fam = run.font.clone().unwrap_or_default();
        let bold = run.bold || heading_bold;
        let italic = run.italic || matches!(p.style, BlockStyle::Quote);
        let font = fonts.pick(&fam, run.code, bold, italic).clone();
        push_words(&mut tokens, &run.text, &font, &fam, run.code, size);
    }
    if tokens.is_empty() {
        *top += line_h; // empty paragraph -> blank line
        return;
    }

    // lay out tokens into wrapped lines, then place each line honoring alignment
    let mut line: Vec<(String, IndirectFontRef, f64, bool)> = Vec::new();
    let mut line_w = 0.0;
    let flush = |layer: &mut PdfLayerReference,
                 line: &mut Vec<(String, IndirectFontRef, f64, bool)>,
                 line_w: f64,
                 top: &mut f64| {
        if *top + line_h > PAGE_H - MARGIN {
            let (np, nl) = pdf.add_page(mm(PAGE_W), mm(PAGE_H), "Layer");
            *layer = pdf.get_page(np).get_layer(nl);
            *top = MARGIN;
        }
        let baseline = PAGE_H - (*top + size * PT_TO_MM);
        let mut x = match p.align {
            Align::Right => right - line_w,
            Align::Center => left + (right - left - line_w) / 2.0,
            _ => left,
        };
        for (txt, font, w, _sp) in line.iter() {
            layer.use_text(txt.clone(), size as f32, mm(x), mm(baseline), font);
            x += w;
        }
        line.clear();
        *top += line_h;
    };

    for tok in tokens.into_iter() {
        let is_space = tok.3;
        if !is_space && !line.is_empty() && line_w + tok.2 > (right - left) {
            // wrap before this word; drop a trailing space if present
            if let Some((_, _, w, true)) = line.last() {
                line_w -= *w;
                line.pop();
            }
            flush(layer, &mut line, line_w, top);
            line_w = 0.0;
            if is_space { continue; }
        }
        line_w += tok.2;
        line.push(tok);
    }
    if !line.is_empty() {
        flush(layer, &mut line, line_w, top);
    }
    *top += line_h * 0.35; // paragraph spacing
}

fn effective_size(p: &Paragraph) -> f64 {
    let style_size = match p.style {
        BlockStyle::H1 => 24.0,
        BlockStyle::H2 => 18.0,
        BlockStyle::H3 => 14.0,
        BlockStyle::Code => 10.5,
        _ => 11.0,
    };
    let run_max = p.runs.iter().filter_map(|r| r.size).max();
    match run_max {
        Some(s) if !matches!(p.style, BlockStyle::H1 | BlockStyle::H2 | BlockStyle::H3) => {
            (s as f64).clamp(6.0, 96.0)
        }
        _ => style_size,
    }
}

fn push_words(
    tokens: &mut Vec<(String, IndirectFontRef, f64, bool)>,
    text: &str,
    font: &IndirectFontRef,
    family: &str,
    code: bool,
    size: f64,
) {
    let mut word = String::new();
    let flush_word = |w: &mut String, toks: &mut Vec<(String, IndirectFontRef, f64, bool)>| {
        if !w.is_empty() {
            let width = text_width(w, family, code, size);
            toks.push((std::mem::take(w), font.clone(), width, false));
        }
    };
    for ch in text.chars() {
        if ch == ' ' || ch == '\t' || ch == '\n' {
            flush_word(&mut word, tokens);
            let sp = if ch == '\t' { "    ".to_string() } else { " ".to_string() };
            let width = text_width(&sp, family, code, size);
            tokens.push((sp, font.clone(), width, true));
        } else {
            word.push(ch);
        }
    }
    flush_word(&mut word, tokens);
}

/// Width of `text` in millimetres at `size` points, using Helvetica metrics
/// (or a fixed advance for monospace families).
fn text_width(text: &str, family: &str, code: bool, size: f64) -> f64 {
    let mono = code
        || {
            let f = family.to_ascii_lowercase();
            f.contains("courier") || f.contains("mono") || f.contains("menlo") || f.contains("consolas")
        };
    let mut units = 0u32;
    for ch in text.chars() {
        let c = ch as u32;
        units += if mono {
            600
        } else if (32..=126).contains(&c) {
            HELVETICA_WIDTHS[(c - 32) as usize] as u32
        } else {
            556
        };
    }
    (units as f64 / 1000.0) * size * PT_TO_MM
}

// Standard Helvetica advance widths (1/1000 em) for ASCII 32..=126.
const HELVETICA_WIDTHS: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, // 32-47
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, // 48-63
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, // 64-79
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, // 80-95
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, // 96-111
    556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584, // 112-126
];
