// ferric — the Tauri shell.
//
// This binary is deliberately thin: it opens a window, serves the web UI, and
// exposes four commands that hand work straight to `ferric-core` (the Rust
// document engine). The web front-end and the engine speak the same
// `Document` JSON, so all the real logic — the model and every file format —
// lives in well-tested Rust, and this file is just the bridge.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use ferric_core::{Document, Format, Stats};
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

/// A fresh document for "New" (the friendly welcome page).
#[tauri::command]
fn new_document() -> Document {
    ferric_core::welcome()
}

/// Word / character / paragraph counts for the status bar.
#[tauri::command]
fn document_stats(doc: Document) -> Stats {
    ferric_core::stats(&doc)
}

#[derive(Serialize)]
struct Opened {
    path: String,
    doc: Document,
}

/// Show a native open dialog, read the chosen file, and parse it into a
/// `Document`. Returns `None` if the user cancels.
#[tauri::command]
fn open_document(app: AppHandle) -> Result<Option<Opened>, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter("Text documents", &["md", "markdown", "txt", "json"])
        .add_filter("Markdown", &["md", "markdown"])
        .add_filter("Plain text", &["txt"])
        .add_filter("ferric document", &["json"])
        .blocking_pick_file();

    let Some(file) = picked else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    let path_str = path.to_string_lossy().into_owned();

    let fmt = Format::from_path(&path_str).ok_or("unrecognised file type")?;
    if !fmt.can_import() {
        return Err(format!(
            "ferric can export but not yet open .{} files",
            fmt.extension()
        ));
    }
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    let doc = ferric_core::from_bytes(&data, fmt)?;
    Ok(Some(Opened { path: path_str, doc }))
}

#[derive(Serialize)]
struct Saved {
    path: String,
}

/// Save a document. If `path` is supplied and matches `format`, write there;
/// otherwise show a native save dialog. Returns `None` if the user cancels.
#[tauri::command]
fn save_document(
    app: AppHandle,
    doc: Document,
    path: Option<String>,
    format: String,
) -> Result<Option<Saved>, String> {
    let fmt = match format.as_str() {
        "markdown" => Format::Markdown,
        "rtf" => Format::Rtf,
        "docx" => Format::Docx,
        "txt" => Format::Txt,
        "json" => Format::Json,
        other => return Err(format!("unknown format: {other}")),
    };

    let target = match path {
        Some(p) if Format::from_path(&p) == Some(fmt) => std::path::PathBuf::from(p),
        _ => {
            let picked = app
                .dialog()
                .file()
                .set_file_name(format!("Untitled.{}", fmt.extension()))
                .add_filter(fmt.extension().to_uppercase(), &[fmt.extension()])
                .blocking_save_file();
            let Some(file) = picked else {
                return Ok(None);
            };
            file.into_path().map_err(|e| e.to_string())?
        }
    };

    let bytes = ferric_core::to_bytes(&doc, fmt)?;
    std::fs::write(&target, bytes).map_err(|e| e.to_string())?;
    Ok(Some(Saved {
        path: target.to_string_lossy().into_owned(),
    }))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            new_document,
            document_stats,
            open_document,
            save_document
        ])
        .run(tauri::generate_context!())
        .expect("error while running ferric");
}
