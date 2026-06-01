//! # ferric-core
//!
//! The document engine behind **ferric**, a Rust word processor. Everything
//! interesting and testable lives here, independent of any UI: the formatted
//! [`Document`] model and conversions to/from Markdown, RTF, `.docx`, plain
//! text and JSON. The Tauri layer is a thin shell that calls into this crate;
//! the web front-end exchanges the same [`Document`] as JSON, so the editor and
//! the engine can never disagree about what a document is.

pub mod docx;
pub mod markdown;
pub mod model;
pub mod odt;
pub mod pdf;
pub mod rtf;
pub mod stats;

pub use docx::to_docx;
pub use markdown::{parse_markdown, to_markdown};
pub use model::{Align, BlockStyle, Document, Paragraph, Run};
pub use odt::to_odt;
pub use pdf::to_pdf;
pub use rtf::to_rtf;
pub use stats::{stats, Stats};

/// A file format ferric can read and/or write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Rtf,
    Docx,
    Odt,
    Pdf,
    Txt,
    Json,
}

impl Format {
    /// Guess a format from a file path's extension.
    pub fn from_path(path: &str) -> Option<Format> {
        let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        match ext.as_str() {
            "md" | "markdown" | "mkd" => Some(Format::Markdown),
            "rtf" => Some(Format::Rtf),
            "docx" => Some(Format::Docx),
            "odt" => Some(Format::Odt),
            "pdf" => Some(Format::Pdf),
            "txt" | "text" => Some(Format::Txt),
            "json" | "ferric" => Some(Format::Json),
            _ => None,
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            Format::Markdown => "md",
            Format::Rtf => "rtf",
            Format::Docx => "docx",
            Format::Odt => "odt",
            Format::Pdf => "pdf",
            Format::Txt => "txt",
            Format::Json => "json",
        }
    }
    /// Whether ferric can currently *import* this format (v1 imports text-like
    /// formats; RTF and .docx are export-only for now — stated honestly).
    pub fn can_import(self) -> bool {
        matches!(self, Format::Markdown | Format::Txt | Format::Json)
    }
}

/// Serialize a document to bytes in the given format.
pub fn to_bytes(doc: &Document, fmt: Format) -> Result<Vec<u8>, String> {
    Ok(match fmt {
        Format::Markdown => to_markdown(doc).into_bytes(),
        Format::Rtf => to_rtf(doc).into_bytes(),
        Format::Docx => to_docx(doc)?,
        Format::Odt => to_odt(doc)?,
        Format::Pdf => to_pdf(doc)?,
        Format::Txt => doc.plain_text().into_bytes(),
        Format::Json => serde_json::to_vec_pretty(doc).map_err(|e| e.to_string())?,
    })
}

/// Parse bytes into a document. Returns an error for formats ferric can't yet
/// import, rather than guessing.
pub fn from_bytes(data: &[u8], fmt: Format) -> Result<Document, String> {
    match fmt {
        Format::Markdown => Ok(parse_markdown(&String::from_utf8_lossy(data))),
        Format::Txt => Ok(plain_to_doc(&String::from_utf8_lossy(data))),
        Format::Json => serde_json::from_slice(data).map_err(|e| e.to_string()),
        Format::Rtf => Err("importing RTF is not supported yet".into()),
        Format::Docx => Err("importing .docx is not supported yet".into()),
        Format::Odt => Err("importing .odt is not supported yet".into()),
        Format::Pdf => Err("PDF is an export-only format".into()),
    }
}

fn plain_to_doc(s: &str) -> Document {
    let mut d = Document::new();
    for line in s.split('\n') {
        let line = line.trim_end_matches('\r');
        let mut p = Paragraph::new(BlockStyle::Normal);
        if !line.is_empty() {
            p.runs.push(Run::new(line));
        }
        d.push(p);
    }
    if d.paragraphs.is_empty() {
        d.push(Paragraph::new(BlockStyle::Normal));
    }
    d
}

/// A friendly starter document, shown on first launch.
pub fn welcome() -> Document {
    let mut d = Document::new();
    d.push(Paragraph::with_text(BlockStyle::H1, "Welcome to ferric"));
    let mut intro = Paragraph::new(BlockStyle::Normal);
    intro.push(Run::new("A small word processor whose engine is written in "));
    intro.push(Run::new("Rust").bold());
    intro.push(Run::new(". Type here — use the ribbon for "));
    intro.push(Run::new("bold").bold());
    intro.push(Run::new(", "));
    intro.push(Run::new("italic").italic());
    intro.push(Run::new(" and "));
    intro.push(Run::new("underline").underline());
    intro.push(Run::new("."));
    d.push(intro);
    d.push(Paragraph::with_text(BlockStyle::H2, "What it can do"));
    d.push(Paragraph::with_text(BlockStyle::Bullet, "Headings, quotes, lists and alignment"));
    d.push(Paragraph::with_text(BlockStyle::Bullet, "Live word and character counts"));
    d.push(Paragraph::with_text(BlockStyle::Bullet, "Save as Markdown, RTF, plain text — or a real .docx Word can open"));
    d.push(Paragraph::with_text(BlockStyle::Quote, "The interesting parts live in Rust; the page you are reading is just glass over the engine."));
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Document {
        let mut d = Document::new();
        d.push(Paragraph::with_text(BlockStyle::H1, "Title"));
        let mut p = Paragraph::new(BlockStyle::Normal);
        p.push(Run::new("Hello "));
        p.push(Run::new("bold").bold());
        p.push(Run::new(" and "));
        p.push(Run::new("italic").italic());
        d.push(p);
        d.push(Paragraph::with_text(BlockStyle::Bullet, "one"));
        d.push(Paragraph::with_text(BlockStyle::Bullet, "two"));
        d.push(Paragraph::with_text(BlockStyle::Quote, "a quote"));
        d
    }

    #[test]
    fn markdown_round_trip_preserves_structure() {
        let doc = sample();
        let md = to_markdown(&doc);
        let back = parse_markdown(&md);
        assert_eq!(back.plain_text(), doc.plain_text());
        assert_eq!(back.paragraphs[0].style, BlockStyle::H1);
        assert!(back.paragraphs.iter().any(|p| p.style == BlockStyle::Bullet));
        assert!(back.paragraphs.iter().any(|p| p.style == BlockStyle::Quote));
        // bold + italic survive the round trip
        let normal = &back.paragraphs[1];
        assert!(normal.runs.iter().any(|r| r.bold && r.text.contains("bold")));
        assert!(normal.runs.iter().any(|r| r.italic && r.text.contains("italic")));
    }

    #[test]
    fn markdown_emits_expected_markers() {
        let md = to_markdown(&sample());
        assert!(md.contains("# Title"));
        assert!(md.contains("**bold**"));
        assert!(md.contains("*italic*"));
        assert!(md.contains("- one"));
        assert!(md.contains("> a quote"));
    }

    #[test]
    fn underline_round_trips_via_html_span() {
        let mut d = Document::new();
        let mut p = Paragraph::new(BlockStyle::Normal);
        p.push(Run::new("under").underline());
        d.push(p);
        let md = to_markdown(&d);
        assert!(md.contains("<u>under</u>"));
        let back = parse_markdown(&md);
        assert!(back.paragraphs[0].runs.iter().any(|r| r.underline && r.text == "under"));
    }

    #[test]
    fn json_round_trips_exactly() {
        let doc = sample();
        let bytes = to_bytes(&doc, Format::Json).unwrap();
        let back = from_bytes(&bytes, Format::Json).unwrap();
        assert_eq!(doc, back);
    }

    #[test]
    fn stats_are_correct() {
        let mut d = Document::new();
        d.push(Paragraph::with_text(BlockStyle::Normal, "the quick brown fox"));
        d.push(Paragraph::with_text(BlockStyle::Normal, "jumps over"));
        let s = stats(&d);
        assert_eq!(s.words, 6);
        assert_eq!(s.paragraphs, 2);
        assert!(s.chars > s.chars_no_spaces);
    }

    #[test]
    fn rtf_is_well_formed() {
        let rtf = to_rtf(&sample());
        assert!(rtf.starts_with("{\\rtf1"));
        assert!(rtf.trim_end().ends_with('}'));
        assert!(rtf.contains("\\b "));
    }

    #[test]
    fn docx_is_a_zip() {
        let bytes = to_docx(&sample()).unwrap();
        assert!(bytes.len() > 100);
        assert_eq!(&bytes[0..2], b"PK"); // .docx is a zip archive
    }

    #[test]
    fn pdf_is_a_pdf() {
        let bytes = to_bytes(&sample(), Format::Pdf).unwrap();
        assert!(bytes.len() > 200);
        assert_eq!(&bytes[0..5], b"%PDF-"); // real PDF header
    }

    #[test]
    fn odt_is_opendocument() {
        let bytes = to_bytes(&sample(), Format::Odt).unwrap();
        assert_eq!(&bytes[0..2], b"PK"); // .odt is a zip archive
        // the mimetype is stored uncompressed, so it appears verbatim in the zip
        let needle = b"application/vnd.oasis.opendocument.text";
        assert!(bytes.windows(needle.len()).any(|w| w == needle));
    }

    #[test]
    fn font_and_size_round_trip_and_export() {
        let mut d = Document::new();
        let mut p = Paragraph::new(BlockStyle::Normal);
        let mut r = Run::new("styled");
        r.font = Some("Georgia".into());
        r.size = Some(18);
        p.push(r);
        d.push(p);
        // survive JSON exactly
        let back = from_bytes(&to_bytes(&d, Format::Json).unwrap(), Format::Json).unwrap();
        assert_eq!(d, back);
        assert_eq!(back.paragraphs[0].runs[0].font.as_deref(), Some("Georgia"));
        assert_eq!(back.paragraphs[0].runs[0].size, Some(18));
        // and export cleanly to every binary format
        for fmt in [Format::Docx, Format::Pdf, Format::Odt, Format::Rtf] {
            assert!(to_bytes(&d, fmt).is_ok(), "export to {fmt:?} failed");
        }
    }

    #[test]
    fn txt_import_export() {
        let d = plain_to_doc("line one\nline two");
        assert_eq!(d.paragraphs.len(), 2);
        let bytes = to_bytes(&d, Format::Txt).unwrap();
        assert_eq!(String::from_utf8(bytes).unwrap(), "line one\nline two");
    }

    #[test]
    fn format_from_path_works() {
        assert_eq!(Format::from_path("a.docx"), Some(Format::Docx));
        assert_eq!(Format::from_path("a.MD"), Some(Format::Markdown));
        assert_eq!(Format::from_path("a.unknown"), None);
        assert!(!Format::Docx.can_import());
        assert!(Format::Markdown.can_import());
    }
}
