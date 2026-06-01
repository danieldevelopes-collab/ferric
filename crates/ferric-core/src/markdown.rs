//! Markdown <-> `Document`.
//!
//! Parsing uses the battle-tested `pulldown-cmark` (a CommonMark parser);
//! serialization is hand-written from the model so we control exactly how a
//! document round-trips. Markdown has no underline, so an underlined run is
//! written as inline `<u>...</u>` (valid in CommonMark) and read back from it.

use crate::model::{BlockStyle, Document, Paragraph, Run};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Parse a Markdown string into a `Document`.
pub fn parse_markdown(input: &str) -> Document {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(input, opts);

    let mut doc = Document::new();
    let mut cur: Option<Paragraph> = None;
    let mut heading: Option<BlockStyle> = None;
    let mut in_code = false;
    let mut quote_depth: u32 = 0;
    let mut list_ordered: Vec<bool> = Vec::new();
    let (mut bold, mut italic, mut underline, mut strike) = (0u32, 0u32, 0u32, 0u32);

    fn style_now(
        heading: &Option<BlockStyle>,
        in_code: bool,
        quote_depth: u32,
        list_ordered: &[bool],
    ) -> BlockStyle {
        if in_code {
            BlockStyle::Code
        } else if let Some(h) = heading {
            *h
        } else if let Some(ordered) = list_ordered.last() {
            if *ordered { BlockStyle::Numbered } else { BlockStyle::Bullet }
        } else if quote_depth > 0 {
            BlockStyle::Quote
        } else {
            BlockStyle::Normal
        }
    }

    macro_rules! finalize {
        () => {
            if let Some(p) = cur.take() {
                if !p.runs.is_empty() {
                    doc.push(p);
                }
            }
        };
    }

    for ev in parser {
        match ev {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    finalize!();
                    heading = Some(match level {
                        HeadingLevel::H1 => BlockStyle::H1,
                        HeadingLevel::H2 => BlockStyle::H2,
                        _ => BlockStyle::H3,
                    });
                }
                Tag::Paragraph | Tag::Item => finalize!(),
                Tag::CodeBlock(_) => {
                    finalize!();
                    in_code = true;
                }
                Tag::BlockQuote(_) => quote_depth += 1,
                Tag::List(start) => list_ordered.push(start.is_some()),
                Tag::Strong => bold += 1,
                Tag::Emphasis => italic += 1,
                Tag::Strikethrough => strike += 1,
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => { finalize!(); heading = None; }
                TagEnd::Paragraph | TagEnd::Item => finalize!(),
                TagEnd::CodeBlock => { finalize!(); in_code = false; }
                TagEnd::BlockQuote(_) => quote_depth = quote_depth.saturating_sub(1),
                TagEnd::List(_) => { list_ordered.pop(); }
                TagEnd::Strong => bold = bold.saturating_sub(1),
                TagEnd::Emphasis => italic = italic.saturating_sub(1),
                TagEnd::Strikethrough => strike = strike.saturating_sub(1),
                _ => {}
            },
            Event::Text(t) => {
                let p = cur.get_or_insert_with(|| {
                    Paragraph::new(style_now(&heading, in_code, quote_depth, &list_ordered))
                });
                push_run(p, &t, bold > 0, italic > 0, underline > 0, strike > 0, false);
            }
            Event::Code(t) => {
                let p = cur.get_or_insert_with(|| {
                    Paragraph::new(style_now(&heading, in_code, quote_depth, &list_ordered))
                });
                push_run(p, &t, false, false, false, false, true);
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(p) = cur.as_mut() {
                    push_run(p, " ", bold > 0, italic > 0, underline > 0, strike > 0, false);
                }
            }
            // Markdown has no underline; we round-trip it as inline <u>...</u>.
            Event::InlineHtml(h) | Event::Html(h) => {
                underline += h.matches("<u>").count() as u32;
                underline = underline.saturating_sub(h.matches("</u>").count() as u32);
            }
            _ => {}
        }
    }
    finalize!();
    if doc.paragraphs.is_empty() {
        doc.push(Paragraph::new(BlockStyle::Normal));
    }
    doc
}

/// Append text to a paragraph, merging with the previous run when the inline
/// formatting is identical (keeps the model tidy).
fn push_run(p: &mut Paragraph, text: &str, b: bool, i: bool, u: bool, s: bool, c: bool) {
    if let Some(last) = p.runs.last_mut() {
        if last.bold == b && last.italic == i && last.underline == u
            && last.strike == s && last.code == c
        {
            last.text.push_str(text);
            return;
        }
    }
    p.runs.push(Run {
        text: text.to_string(),
        bold: b, italic: i, underline: u, strike: s, code: c,
    });
}

/// Serialize a `Document` to Markdown.
pub fn to_markdown(doc: &Document) -> String {
    let mut out = String::new();
    for (i, p) in doc.paragraphs.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        match p.style {
            BlockStyle::Code => {
                out.push_str("```\n");
                out.push_str(&p.plain_text());
                out.push_str("\n```\n");
            }
            other => {
                let prefix = match other {
                    BlockStyle::H1 => "# ",
                    BlockStyle::H2 => "## ",
                    BlockStyle::H3 => "### ",
                    BlockStyle::Quote => "> ",
                    BlockStyle::Bullet => "- ",
                    BlockStyle::Numbered => "1. ",
                    _ => "",
                };
                out.push_str(prefix);
                for run in &p.runs {
                    out.push_str(&render_run(run));
                }
                out.push('\n');
            }
        }
    }
    out
}

fn render_run(run: &Run) -> String {
    if run.text.is_empty() {
        return String::new();
    }
    if run.code {
        return format!("`{}`", run.text);
    }
    let mut text = run.text.clone();
    if run.underline {
        text = format!("<u>{}</u>", text);
    }
    if run.bold {
        text = format!("**{}**", text);
    }
    if run.italic {
        text = format!("*{}*", text);
    }
    if run.strike {
        text = format!("~~{}~~", text);
    }
    text
}
