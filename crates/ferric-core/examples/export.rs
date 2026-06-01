//! Use `ferric-core` as a library to convert one document into every format
//! ferric can write. Run with:
//!
//!     cargo run -p ferric-core --example export
//!
//! It writes the sample files into your temp directory and prints the paths —
//! handy for sanity-checking that the PDF really is a PDF and the .odt really
//! opens in LibreOffice.

use ferric_core::{to_bytes, welcome, Format};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = welcome();
    let dir = std::env::temp_dir();

    let targets = [
        (Format::Markdown, "ferric_sample.md"),
        (Format::Rtf, "ferric_sample.rtf"),
        (Format::Docx, "ferric_sample.docx"),
        (Format::Odt, "ferric_sample.odt"),
        (Format::Pdf, "ferric_sample.pdf"),
        (Format::Txt, "ferric_sample.txt"),
        (Format::Json, "ferric_sample.json"),
    ];

    for (fmt, name) in targets {
        let path = dir.join(name);
        let bytes = to_bytes(&doc, fmt)?;
        std::fs::write(&path, &bytes)?;
        println!("wrote {:>22}  ({} bytes)", path.display(), bytes.len());
    }
    Ok(())
}
