//! `Document` -> `.docx` (Office Open XML) via the `docx-rs` crate.
//!
//! This is the headline interoperability feature: a file you can open in
//! Microsoft Word, Google Docs or LibreOffice. Headings are rendered as bold,
//! larger runs; quotes as italic; code as a monospace font; lists with a simple
//! glyph/number prefix (full Word numbering is out of scope for v1).

use crate::model::{Align, BlockStyle, Document, Paragraph, Run};
use docx_rs::{AlignmentType, Docx, Paragraph as DxPara, Run as DxRun, RunFonts};
use std::io::Cursor;

pub fn to_docx(doc: &Document) -> Result<Vec<u8>, String> {
    let mut d = Docx::new();
    for p in &doc.paragraphs {
        d = d.add_paragraph(build_para(p));
    }
    let mut buf = Vec::new();
    d.build()
        .pack(&mut Cursor::new(&mut buf))
        .map_err(|e| format!("docx pack failed: {e:?}"))?;
    Ok(buf)
}

fn build_para(p: &Paragraph) -> DxPara {
    let mut para = DxPara::new().align(match p.align {
        Align::Left => AlignmentType::Left,
        Align::Center => AlignmentType::Center,
        Align::Right => AlignmentType::Right,
        Align::Justify => AlignmentType::Justified,
    });

    let (size, heading_bold) = match p.style {
        BlockStyle::H1 => (Some(36usize), true),
        BlockStyle::H2 => (Some(30), true),
        BlockStyle::H3 => (Some(26), true),
        _ => (None, false),
    };

    let prefix = match p.style {
        BlockStyle::Bullet => "•  ",
        BlockStyle::Numbered => "1.  ",
        _ => "",
    };
    if !prefix.is_empty() {
        para = para.add_run(DxRun::new().add_text(prefix));
    }

    if p.runs.is_empty() {
        para = para.add_run(DxRun::new().add_text(""));
    }
    for run in &p.runs {
        para = para.add_run(build_run(run, size, heading_bold, p.style));
    }
    para
}

fn build_run(run: &Run, size: Option<usize>, heading_bold: bool, style: BlockStyle) -> DxRun {
    let mut r = DxRun::new().add_text(&run.text);
    if let Some(sz) = size {
        r = r.size(sz);
    }
    if run.bold || heading_bold {
        r = r.bold();
    }
    if run.italic || matches!(style, BlockStyle::Quote) {
        r = r.italic();
    }
    if run.underline {
        r = r.underline("single");
    }
    if run.strike {
        r = r.strike();
    }
    if run.code || matches!(style, BlockStyle::Code) {
        r = r.fonts(RunFonts::new().ascii("Menlo").hi_ansi("Menlo"));
    }
    r
}
