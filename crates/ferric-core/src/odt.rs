//! `Document` -> `.odt` (OpenDocument Text) — the format LibreOffice Writer,
//! OpenOffice and Google Docs use.
//!
//! An `.odt` is a zip archive of XML. We build it directly: a `mimetype` entry
//! (stored uncompressed and first, as the spec requires), a manifest, a minimal
//! `styles.xml`, and a `content.xml` whose `<office:automatic-styles>` are
//! generated on the fly — one paragraph style per (block, alignment) and one
//! text style per unique run formatting (bold / italic / underline / strike /
//! code / font / size), referenced from the body.

use crate::model::{Align, BlockStyle, Document, Paragraph, Run};
use std::collections::HashMap;
use std::io::Write;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

pub fn to_odt(doc: &Document) -> Result<Vec<u8>, String> {
    let content = content_xml(doc);
    let styles = STYLES_XML;
    let manifest = MANIFEST_XML;

    let mut buf = Vec::new();
    {
        let mut zw = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        // mimetype MUST be first and stored (uncompressed) for ODF detection.
        zw.start_file(
            "mimetype",
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )
        .map_err(|e| e.to_string())?;
        zw.write_all(b"application/vnd.oasis.opendocument.text")
            .map_err(|e| e.to_string())?;

        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, data) in [
            ("META-INF/manifest.xml", manifest),
            ("styles.xml", styles),
            ("content.xml", content.as_str()),
        ] {
            zw.start_file(name, opts).map_err(|e| e.to_string())?;
            zw.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
        }
        zw.finish().map_err(|e| e.to_string())?;
    }
    Ok(buf)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn align_attr(a: Align) -> &'static str {
    match a {
        Align::Left => "start",
        Align::Right => "end",
        Align::Center => "center",
        Align::Justify => "justify",
    }
}

fn content_xml(doc: &Document) -> String {
    let mut para_styles: HashMap<String, String> = HashMap::new();
    let mut text_styles: HashMap<String, String> = HashMap::new();
    let mut style_defs = String::new();
    let mut body = String::new();

    // group consecutive list items into one <text:list>
    let mut open_list: Option<BlockStyle> = None;

    for p in &doc.paragraphs {
        let is_list = matches!(p.style, BlockStyle::Bullet | BlockStyle::Numbered);
        if open_list.is_some() && open_list != Some(p.style) {
            body.push_str("</text:list>");
            open_list = None;
        }
        if is_list && open_list.is_none() {
            body.push_str("<text:list>");
            open_list = Some(p.style);
        }

        let pname = para_style_name(p, &mut para_styles, &mut style_defs);
        let inner = paragraph_inner(p, &mut text_styles, &mut style_defs);

        let block = match p.style {
            BlockStyle::H1 => format!(
                "<text:h text:style-name=\"{pname}\" text:outline-level=\"1\">{inner}</text:h>"
            ),
            BlockStyle::H2 => format!(
                "<text:h text:style-name=\"{pname}\" text:outline-level=\"2\">{inner}</text:h>"
            ),
            BlockStyle::H3 => format!(
                "<text:h text:style-name=\"{pname}\" text:outline-level=\"3\">{inner}</text:h>"
            ),
            _ => format!("<text:p text:style-name=\"{pname}\">{inner}</text:p>"),
        };

        if is_list {
            body.push_str(&format!("<text:list-item>{block}</text:list-item>"));
        } else {
            body.push_str(&block);
        }
    }
    if open_list.is_some() {
        body.push_str("</text:list>");
    }

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<office:document-content \
xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" \
xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" \
xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\" \
office:version=\"1.3\">\
<office:automatic-styles>{style_defs}</office:automatic-styles>\
<office:body><office:text>{body}</office:text></office:body>\
</office:document-content>"
    )
}

fn para_style_name(
    p: &Paragraph,
    styles: &mut HashMap<String, String>,
    defs: &mut String,
) -> String {
    let (size, bold, italic, ml) = match p.style {
        BlockStyle::H1 => (Some("24pt"), true, false, None),
        BlockStyle::H2 => (Some("18pt"), true, false, None),
        BlockStyle::H3 => (Some("14pt"), true, false, None),
        BlockStyle::Quote => (None, false, true, Some("0.5cm")),
        _ => (None, false, false, None),
    };
    let key = format!("{:?}-{}", p.style, align_attr(p.align));
    if let Some(n) = styles.get(&key) {
        return n.clone();
    }
    let name = format!("P{}", styles.len() + 1);
    let mut tp = String::new();
    if let Some(s) = size {
        tp.push_str(&format!(" fo:font-size=\"{s}\""));
    }
    if bold {
        tp.push_str(" fo:font-weight=\"bold\"");
    }
    if italic {
        tp.push_str(" fo:font-style=\"italic\"");
    }
    let ml_attr = ml.map(|m| format!(" fo:margin-left=\"{m}\"")).unwrap_or_default();
    defs.push_str(&format!(
        "<style:style style:name=\"{name}\" style:family=\"paragraph\">\
<style:paragraph-properties fo:text-align=\"{}\"{ml_attr}/>\
{}\
</style:style>",
        align_attr(p.align),
        if tp.is_empty() {
            String::new()
        } else {
            format!("<style:text-properties{tp}/>")
        }
    ));
    styles.insert(key, name.clone());
    name
}

fn paragraph_inner(
    p: &Paragraph,
    styles: &mut HashMap<String, String>,
    defs: &mut String,
) -> String {
    let mut out = String::new();
    for run in &p.runs {
        if run.text.is_empty() {
            continue;
        }
        let tname = text_style_name(run, p.style, styles, defs);
        out.push_str(&format!(
            "<text:span text:style-name=\"{tname}\">{}</text:span>",
            esc(&run.text)
        ));
    }
    out
}

fn text_style_name(
    run: &Run,
    block: BlockStyle,
    styles: &mut HashMap<String, String>,
    defs: &mut String,
) -> String {
    let code = run.code || matches!(block, BlockStyle::Code);
    let key = format!(
        "{}{}{}{}{}{}{}",
        run.bold as u8,
        run.italic as u8,
        run.underline as u8,
        run.strike as u8,
        code as u8,
        run.font.clone().unwrap_or_default(),
        run.size.map(|s| s.to_string()).unwrap_or_default()
    );
    if let Some(n) = styles.get(&key) {
        return n.clone();
    }
    let name = format!("T{}", styles.len() + 1);
    let mut tp = String::new();
    if run.bold {
        tp.push_str(" fo:font-weight=\"bold\"");
    }
    if run.italic {
        tp.push_str(" fo:font-style=\"italic\"");
    }
    if run.underline {
        tp.push_str(" style:text-underline-style=\"solid\" style:text-underline-width=\"auto\" style:text-underline-color=\"font-color\"");
    }
    if run.strike {
        tp.push_str(" style:text-line-through-style=\"solid\"");
    }
    if let Some(font) = &run.font {
        tp.push_str(&format!(" style:font-name=\"{}\"", esc(font)));
    } else if code {
        tp.push_str(" style:font-name=\"Liberation Mono\"");
    }
    if let Some(sz) = run.size {
        tp.push_str(&format!(" fo:font-size=\"{sz}pt\""));
    }
    defs.push_str(&format!(
        "<style:style style:name=\"{name}\" style:family=\"text\"><style:text-properties{tp}/></style:style>"
    ));
    styles.insert(key, name.clone());
    name
}

const STYLES_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<office:document-styles \
xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" \
xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\" \
office:version=\"1.3\">\
<office:styles>\
<style:default-style style:family=\"paragraph\">\
<style:text-properties style:font-name=\"Liberation Serif\" fo:font-size=\"11pt\"/>\
</style:default-style>\
</office:styles>\
</office:document-styles>";

const MANIFEST_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<manifest:manifest xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\" manifest:version=\"1.3\">\
<manifest:file-entry manifest:full-path=\"/\" manifest:version=\"1.3\" manifest:media-type=\"application/vnd.oasis.opendocument.text\"/>\
<manifest:file-entry manifest:full-path=\"content.xml\" manifest:media-type=\"text/xml\"/>\
<manifest:file-entry manifest:full-path=\"styles.xml\" manifest:media-type=\"text/xml\"/>\
</manifest:manifest>";
