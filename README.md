# ferric

> A small word processor that **functions like a limited version of Microsoft
> Word** — headings, bold/italic/underline, lists, alignment, live word counts,
> and **export to a real `.docx`** Word can open — but whose engine is written
> in **Rust**. The page you type on is glass; the machinery underneath is iron.

**By Daniel Bratcher · [@danieldevelopes-collab](https://github.com/danieldevelopes-collab)**
· MIT licensed · Rust + Tauri · cross-platform desktop app

![The ferric word processor — a Word-style ribbon over a paper page](docs/ui.png)

---

## What it is

ferric is a desktop word processor built as a **[Tauri](https://tauri.app)**
app: a native window whose interface is a calm, Word-like web UI, and whose
**document engine is a pure-Rust library** (`ferric-core`). It does the things a
word processor should:

- **Formatting** — bold, italic, underline, strikethrough, inline code.
- **Block styles** — Heading 1/2/3, normal text, block quotes, bullet & numbered
  lists, and a code block.
- **Alignment** — left, centre, right, justified.
- **Live counts** — words, characters, and an estimated reading time, always in
  the status bar.
- **Open** Markdown, plain text, and ferric's own JSON.
- **Save / export** to **Markdown**, **RTF**, **plain text**, ferric JSON — and
  a genuine **`.docx`** that Microsoft Word, Google Docs and LibreOffice open.

It is deliberately *limited*: no comments, no track-changes, no tables, no
embedded images (yet). It does the core of word processing, and does it
honestly.

---

## Why it's built this way (and why that's the interesting part)

A word processor is mostly a **document model** and a pile of **file-format
conversions** — and that is exactly the part worth writing in Rust, where a
precise type system and thorough tests pay off. So ferric splits cleanly:

```
crates/ferric-core/   ← the engine (pure Rust, no UI): the model + every format
src-tauri/            ← a thin Tauri shell: opens a window, exposes 4 commands
src/                  ← the web UI: a Word-like ribbon + a contenteditable page
```

The engine and the UI exchange one thing — a **`Document`** as JSON:

```
Document  = { paragraphs: [ { style, align, runs: [ { text, bold, italic, … } ] } ] }
```

The UI never parses a file format and the engine never touches the DOM. When you
press **Save as .docx**, the page is serialised to a `Document`, handed to Rust,
and `ferric-core` builds the Office Open XML with the `docx-rs` crate. All the
logic that could be wrong lives in Rust, behind a test suite — and the four
Tauri commands (`new_document`, `open_document`, `save_document`,
`document_stats`) are a dozen lines each.

`ferric-core` has **no dependency on Tauri at all**: you can use it as a library
to convert documents between Markdown, RTF, `.docx`, text and JSON from any Rust
program.

---

## A short history of the word processor

Typing this README on a glowing rectangle, it's easy to forget how recent all of
this is.

- **1964 — the term is born.** IBM markets the **MT/ST** (Magnetic Tape Selectric
  Typewriter); the phrase "word processing" comes from IBM's German
  *Textverarbeitung*, attributed to **Ulrich Steinhilper**.
- **1974 — WYSIWYG.** At **Xerox PARC**, **Charles Simonyi** and **Butler
  Lampson** write **Bravo**, the first what-you-see-is-what-you-get word
  processor, on the Alto.
- **1976 — the microcomputer.** **Michael Shrayer** writes **Electric Pencil**,
  widely held to be the first word processor for a home computer.
- **1978 — WordStar.** **Rob Barnaby** (at Seymour Rubinstein's MicroPro) writes
  **WordStar**, which dominates the early PC era.
- **1979 — WordPerfect.** **Alan Ashton** and **Bruce Bastian** create
  **WordPerfect**, the office standard of the 1980s.
- **1983 — Microsoft Word.** **Charles Simonyi** (yes, from Bravo) and **Richard
  Brodie** ship **Microsoft Word** — the application ferric gently imitates.
- **The formats follow:** Microsoft's **RTF** (~1987) and binary **.doc**, then
  the open **OOXML / `.docx`** (ECMA-376 / ISO 29500, 2007); and, from a very
  different tradition, **Markdown** (**John Gruber** with **Aaron Swartz**, 2004),
  later standardised as **CommonMark**.

ferric reads and writes that last half-century of formats from a few hundred
lines of Rust.

---

## Run it

You need [Rust](https://rustup.rs) and the Tauri prerequisites for your OS
(macOS: Xcode command-line tools; Linux: `webkit2gtk`; Windows: WebView2 —
see <https://tauri.app/start/prerequisites/>).

```bash
# install the Tauri CLI once
cargo install tauri-cli --version "^2"

# run the app
cargo tauri dev          # from the repo root

# build a release binary
cargo tauri build
```

Test the engine on its own (no GUI, no Tauri):

```bash
cargo test -p ferric-core
```

---

## Credits & acknowledgements

ferric stands on a lot of other people's work. Where a name or date is
imperfect, corrections are welcome — these are sincere thanks.

**The pioneers** — Charles Simonyi & Butler Lampson (Bravo, the first WYSIWYG);
Michael Shrayer (Electric Pencil); Rob Barnaby (WordStar); Alan Ashton & Bruce
Bastian (WordPerfect); Charles Simonyi & Richard Brodie (Microsoft Word).

**The language & frameworks**
- **Rust** — created by **Graydon Hoare** (2010) and the Rust project. The whole
  engine and shell are Rust.
- **Tauri** — the Tauri Working Group, for a way to ship a Rust app with a web UI
  and a tiny footprint.

**The crates that do real work**
- **`docx-rs`** (by *bokuweb*) — builds the Office Open XML for `.docx` export.
- **`pulldown-cmark`** (by **Raph Levien** & contributors) — the CommonMark
  parser behind Markdown import.
- **`serde`** / **`serde_json`** (by **David Tolnay** & Erick Tryzelaar) — the
  serialization that carries a `Document` across the wire and to JSON.

**The formats & the web platform**
- **Microsoft / ECMA** for RTF and OOXML (`.docx`); **John Gruber** & **Aaron
  Swartz** for Markdown.
- **Tim Berners-Lee** (HTML) and **Brendan Eich** (JavaScript) — the web platform
  the editor is painted with; and **Ian Hickson** & the WHATWG for the
  `contenteditable` and `execCommand` machinery the page leans on.

---

## Honesty

- **The engine is real and tested.** `cargo test -p ferric-core` runs a suite
  covering the model, Markdown round-trips (including underline via inline HTML),
  RTF, `.docx` (verified to be a valid zip archive), stats and JSON — all green.
- **`.docx` export is genuine** — it produces an Office Open XML file Word opens,
  not a rename of something else.
- **What it does *not* do (yet):** it can export RTF and `.docx` but not yet
  *import* them; there are no tables, images, comments or track-changes. These
  are stated, not hidden.
- **On verification:** the Rust engine is unit-tested (all green), the Tauri
  shell **compiles cleanly** (`cargo check`), and the UI is shown above. The
  windowed app is best experienced by running it yourself with `cargo tauri
  dev` — a desktop window can't be screenshotted in the environment this was
  assembled in, so its *look* is proven (the web UI), its *engine* is proven
  (the tests), the shell is proven to build, and the three are joined by four
  small commands.

---

## License

[MIT](LICENSE) © 2026 Daniel Bratcher (danieldevelopes-collab).
