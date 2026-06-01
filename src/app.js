/* ferric — front-end logic
   Daniel Bratcher
   Maps the JSON document model to an editable page and back, drives the
   toolbar, tracks the dirty flag, and talks to the Rust backend over Tauri
   when present (with a full in-browser fallback when it is not). */

(function () {
  "use strict";

  /* ---------------------------------------------------------------- *
   * Backend bridge
   * ---------------------------------------------------------------- */
  var TAURI = (typeof window !== "undefined" && window.__TAURI__) ? window.__TAURI__ : null;
  var HAS_TAURI = !!(TAURI && TAURI.core && typeof TAURI.core.invoke === "function");

  function invoke(name, args) {
    return TAURI.core.invoke(name, args);
  }

  /* ---------------------------------------------------------------- *
   * Element handles (every id/class below exists in index.html)
   * ---------------------------------------------------------------- */
  var editor       = document.getElementById("editor");
  var canvas       = document.getElementById("canvas");
  var styleSelect  = document.getElementById("style-select");
  var fontSelect   = document.getElementById("font-select");
  var sizeSelect   = document.getElementById("size-select");

  var btnNew       = document.getElementById("btn-new");
  var btnOpen      = document.getElementById("btn-open");
  var btnSave      = document.getElementById("btn-save");
  var btnSaveAs    = document.getElementById("btn-saveas");
  var btnPrint     = document.getElementById("btn-print");
  var saveAsList   = document.getElementById("saveas-list");

  var statWords    = document.getElementById("stat-words");
  var statChars    = document.getElementById("stat-chars");
  var statReading  = document.getElementById("stat-reading");

  var saveIndicator = document.getElementById("save-indicator");
  var saveText      = document.getElementById("save-text");
  var toastRegion   = document.getElementById("toast-region");

  /* Inline-format toggle buttons keyed by their execCommand. "code" is a
     custom command handled separately (no native execCommand for it). */
  var formatButtons = {
    bold:          document.getElementById("btn-bold"),
    italic:        document.getElementById("btn-italic"),
    underline:     document.getElementById("btn-underline"),
    strikeThrough: document.getElementById("btn-strike"),
    code:          document.getElementById("btn-code")
  };

  var listButtons = {
    insertUnorderedList: document.getElementById("btn-ul"),
    insertOrderedList:   document.getElementById("btn-ol")
  };

  var alignButtons = {
    justifyLeft:   document.getElementById("btn-align-left"),
    justifyCenter: document.getElementById("btn-align-center"),
    justifyRight:  document.getElementById("btn-align-right"),
    justifyFull:   document.getElementById("btn-align-justify")
  };

  /* ---------------------------------------------------------------- *
   * App state
   * ---------------------------------------------------------------- */
  var currentPath = null;   // last saved file path (for plain Save)
  var dirty = false;        // unsaved changes?
  var suppressInput = false; // ignore input events while we rebuild the DOM

  /* ---------------------------------------------------------------- *
   * Model <-> DOM mapping tables
   * ---------------------------------------------------------------- */
  var STYLE_TO_TAG = {
    Normal: "p", H1: "h1", H2: "h2", H3: "h3",
    Quote: "blockquote", Code: "pre"
    // Bullet / Numbered are emitted as <li> within <ul>/<ol> (handled below).
  };

  var TAG_TO_STYLE = {
    P: "Normal", DIV: "Normal",
    H1: "H1", H2: "H2", H3: "H3",
    BLOCKQUOTE: "Quote", PRE: "Code"
  };

  var ALIGN_TO_CSS = { Left: "left", Center: "center", Right: "right", Justify: "justify" };

  /* Body text default (points). Runs at this size carry no explicit size and
     serialize without a `size` field; likewise the default/inherited family
     omits `font`. The font-size <select> lists these point values. */
  var DEFAULT_SIZE_PT = 12;

  /* Offline font families offered in the ribbon (must match the <select>). The
     map normalizes a CSS family token back to one of these clean names. */
  var FONT_FAMILIES = [
    "Georgia", "Times New Roman", "Helvetica", "Arial",
    "Courier New", "Verdana", "Garamond"
  ];

  /* The paragraph-style <select> values map onto formatBlock tags. */
  var SELECT_TO_STYLE = {
    p: "Normal", h1: "H1", h2: "H2", h3: "H3", blockquote: "Quote", pre: "Code"
  };

  /* ================================================================ *
   * Model -> DOM : render a Document into the editable page
   * ================================================================ */
  function renderDocument(doc) {
    var paras = (doc && doc.paragraphs) ? doc.paragraphs : [];
    var html = "";
    var i = 0;

    while (i < paras.length) {
      var p = paras[i] || {};
      var style = p.style || "Normal";

      if (style === "Bullet" || style === "Numbered") {
        // Coalesce a run of consecutive list items of the same kind.
        var listTag = (style === "Bullet") ? "ul" : "ol";
        html += "<" + listTag + ">";
        while (i < paras.length && paras[i] && paras[i].style === style) {
          html += "<li" + alignAttr(paras[i].align) + ">" + runsToHtml(paras[i].runs) + "</li>";
          i++;
        }
        html += "</" + listTag + ">";
      } else {
        var tag = STYLE_TO_TAG[style] || "p";
        html += "<" + tag + alignAttr(p.align) + ">" + runsToHtml(p.runs) + "</" + tag + ">";
        i++;
      }
    }

    if (html === "") {
      html = "<p><br></p>"; // never leave the editor empty
    }

    suppressInput = true;
    editor.innerHTML = html;
    suppressInput = false;
  }

  function alignAttr(align) {
    var css = ALIGN_TO_CSS[align];
    if (!css || css === "left") return ""; // Left is the default
    return ' style="text-align:' + css + '"';
  }

  function runsToHtml(runs) {
    if (!runs || !runs.length) return "<br>";
    var out = "";
    for (var i = 0; i < runs.length; i++) {
      var r = runs[i] || {};
      var text = escapeHtml(r.text != null ? r.text : "");
      if (text === "") continue;
      var open = "", close = "";
      // A font/size span wraps the outside so inline tags inherit it.
      var styleCss = runStyleCss(r);
      if (styleCss) { open += '<span style="' + styleCss + '">'; close = "</span>" + close; }
      // Order is cosmetic; serialization detects formats regardless of nesting.
      if (r.bold)      { open += "<strong>"; close = "</strong>" + close; }
      if (r.italic)    { open += "<em>";     close = "</em>" + close; }
      if (r.underline) { open += "<u>";      close = "</u>" + close; }
      if (r.strike)    { open += "<s>";      close = "</s>" + close; }
      if (r.code)      { open += "<code>";   close = "</code>" + close; }
      out += open + text + close;
    }
    return out === "" ? "<br>" : out;
  }

  /* Build the inline style for a run's font/size (empty when both default). */
  function runStyleCss(r) {
    var css = "";
    if (r.font) css += "font-family:" + cssFontStack(r.font) + ";";
    if (r.size) css += "font-size:" + r.size + "pt;";
    return css;
  }

  /* A named family plus a sensible generic fallback, so missing fonts still
     render reasonably offline. The name is escaped for the style attribute. */
  function cssFontStack(name) {
    var quoted = "'" + String(name).replace(/'/g, "") + "'";
    var generic = "sans-serif";
    if (name === "Georgia" || name === "Times New Roman" || name === "Garamond") generic = "serif";
    else if (name === "Courier New") generic = "monospace";
    return quoted + ", " + generic;
  }

  function escapeHtml(s) {
    return s.replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;");
  }

  /* ================================================================ *
   * DOM -> Model : serialize the page into a Document
   * ================================================================ */
  function serializeDocument() {
    var paragraphs = [];
    var blocks = editor.childNodes;

    for (var i = 0; i < blocks.length; i++) {
      var node = blocks[i];
      if (node.nodeType !== 1) {
        // Stray top-level text (e.g. after odd edits) -> a Normal paragraph.
        if (node.nodeType === 3 && node.nodeValue.trim() !== "") {
          paragraphs.push({ style: "Normal", align: "Left", runs: [{ text: node.nodeValue }] });
        }
        continue;
      }

      var tag = node.tagName;

      if (tag === "UL" || tag === "OL") {
        var listStyle = (tag === "UL") ? "Bullet" : "Numbered";
        var items = node.children;
        for (var j = 0; j < items.length; j++) {
          if (items[j].tagName !== "LI") continue;
          paragraphs.push(blockToParagraph(items[j], listStyle));
        }
      } else {
        var style = TAG_TO_STYLE[tag] || "Normal";
        paragraphs.push(blockToParagraph(node, style));
      }
    }

    if (paragraphs.length === 0) {
      paragraphs.push({ style: "Normal", align: "Left", runs: [] });
    }
    return { paragraphs: paragraphs };
  }

  function blockToParagraph(el, style) {
    var base = { bold: false, italic: false, underline: false, strike: false, code: false, font: null, size: null };
    var runs = coalesce(collectRuns(el, base));
    return { style: style, align: alignOf(el), runs: runs };
  }

  function alignOf(el) {
    var ta = el.style && el.style.textAlign;
    if (!ta) {
      // Fall back to computed style (covers execCommand justify* which may set it).
      try { ta = window.getComputedStyle(el).textAlign; } catch (e) { ta = ""; }
    }
    ta = (ta || "").toLowerCase();
    if (ta === "center") return "Center";
    if (ta === "right")  return "Right";
    if (ta === "justify") return "Justify";
    if (ta === "start" || ta === "left" || ta === "") return "Left";
    return "Left";
  }

  /* Walk inline descendants, accumulating active formats, into flat runs. */
  function collectRuns(node, fmt) {
    var runs = [];
    var kids = node.childNodes;
    for (var i = 0; i < kids.length; i++) {
      var child = kids[i];
      if (child.nodeType === 3) {
        if (child.nodeValue !== "") {
          runs.push(makeRun(child.nodeValue, fmt));
        }
      } else if (child.nodeType === 1) {
        var t = child.tagName;
        if (t === "BR") {
          // Soft break within a block -> newline inside the run stream.
          runs.push(makeRun("\n", fmt));
          continue;
        }
        var efont = fontFamilyOf(child);
        var esize = fontSizeOf(child);
        var next = {
          bold:      fmt.bold      || t === "STRONG" || t === "B" || isBoldStyle(child),
          italic:    fmt.italic    || t === "EM" || t === "I" || isItalicStyle(child),
          underline: fmt.underline || t === "U" || hasDecoration(child, "underline"),
          strike:    fmt.strike    || t === "S" || t === "STRIKE" || t === "DEL" || hasDecoration(child, "line-through"),
          code:      fmt.code      || t === "CODE" || t === "TT" || t === "KBD" || t === "SAMP",
          // Inner explicit styles override an outer span's; otherwise inherit.
          font:      (efont != null) ? efont : fmt.font,
          size:      (esize != null) ? esize : fmt.size
        };
        runs = runs.concat(collectRuns(child, next));
      }
    }
    return runs;
  }

  function makeRun(text, fmt) {
    var r = { text: text };
    if (fmt.bold)      r.bold = true;
    if (fmt.italic)    r.italic = true;
    if (fmt.underline) r.underline = true;
    if (fmt.strike)    r.strike = true;
    if (fmt.code)      r.code = true;
    if (fmt.font)      r.font = fmt.font;
    if (fmt.size != null && fmt.size !== DEFAULT_SIZE_PT) r.size = fmt.size;
    return r;
  }

  function isBoldStyle(el) {
    var w = el.style && el.style.fontWeight;
    if (!w) return false;
    return w === "bold" || w === "bolder" || (parseInt(w, 10) >= 600);
  }
  function isItalicStyle(el) {
    var s = el.style && el.style.fontStyle;
    return s === "italic" || s === "oblique";
  }
  function hasDecoration(el, kind) {
    var d = el.style && (el.style.textDecorationLine || el.style.textDecoration);
    return !!d && d.indexOf(kind) !== -1;
  }

  /* Read an element's explicit inline font-family, normalized to one of the
     ribbon's clean family names. Returns "" to mean "Default" (clear the
     inherited family), or null when the element sets no family at all. */
  function fontFamilyOf(el) {
    var ff = el.style && el.style.fontFamily;
    if (!ff) return null;
    var first = ff.split(",")[0].replace(/['"]/g, "").trim();
    if (first === "" || /^(inherit|initial|default)$/i.test(first)) return "";
    for (var i = 0; i < FONT_FAMILIES.length; i++) {
      if (FONT_FAMILIES[i].toLowerCase() === first.toLowerCase()) return FONT_FAMILIES[i];
    }
    return first; // an unlisted but explicit family — keep what the user has
  }

  /* Read an element's explicit inline font-size and return whole points, or
     null when none is set. Supports pt directly and px (96px = 72pt). */
  function fontSizeOf(el) {
    var fs = el.style && el.style.fontSize;
    if (!fs) return null;
    var m = /^([\d.]+)(pt|px)?$/.exec(fs.trim());
    if (!m) return null;
    var n = parseFloat(m[1]);
    if (!isFinite(n)) return null;
    if (m[2] === "px") n = n * 72 / 96;
    return Math.round(n);
  }

  /* Merge adjacent runs that carry identical formatting. */
  function coalesce(runs) {
    var out = [];
    for (var i = 0; i < runs.length; i++) {
      var r = runs[i];
      var last = out[out.length - 1];
      if (last && sameFmt(last, r)) {
        last.text += r.text;
      } else {
        out.push({
          text: r.text,
          bold: !!r.bold, italic: !!r.italic, underline: !!r.underline,
          strike: !!r.strike, code: !!r.code,
          font: r.font || "", size: (r.size != null ? r.size : null)
        });
      }
    }
    // Strip default flags/values to match the wire format (defaults omitted).
    for (var k = 0; k < out.length; k++) {
      var o = out[k];
      if (!o.bold) delete o.bold;
      if (!o.italic) delete o.italic;
      if (!o.underline) delete o.underline;
      if (!o.strike) delete o.strike;
      if (!o.code) delete o.code;
      if (!o.font) delete o.font;
      if (o.size == null || o.size === DEFAULT_SIZE_PT) delete o.size;
    }
    return out;
  }

  function sameFmt(a, b) {
    return !!a.bold === !!b.bold &&
           !!a.italic === !!b.italic &&
           !!a.underline === !!b.underline &&
           !!a.strike === !!b.strike &&
           !!a.code === !!b.code &&
           (a.font || "") === (b.font || "") &&
           (a.size != null ? a.size : null) === (b.size != null ? b.size : null);
  }

  /* ================================================================ *
   * Formatting commands
   * ================================================================ */
  function exec(cmd, value) {
    editor.focus();
    try {
      document.execCommand(cmd, false, value);
    } catch (e) { /* webview always supports these; ignore otherwise */ }
    markDirty();
    refreshToolbar();
  }

  /* Inline code has no native execCommand: wrap/unwrap the selection in
     <code> ourselves, then let serialization pick it up. */
  function toggleInlineCode() {
    editor.focus();
    var sel = window.getSelection();
    if (!sel || sel.rangeCount === 0) return;
    var range = sel.getRangeAt(0);

    var existing = closestWithin(sel.anchorNode, "CODE");
    if (existing) {
      unwrap(existing);
    } else if (!range.collapsed) {
      var code = document.createElement("code");
      try {
        code.appendChild(range.extractContents());
        range.insertNode(code);
        // Reselect the wrapped content.
        var nr = document.createRange();
        nr.selectNodeContents(code);
        sel.removeAllRanges();
        sel.addRange(nr);
      } catch (e) { /* complex selection spanning blocks — ignore */ }
    }
    markDirty();
    refreshToolbar();
  }

  function closestWithin(node, tagName) {
    while (node && node !== editor) {
      if (node.nodeType === 1 && node.tagName === tagName) return node;
      node = node.parentNode;
    }
    return null;
  }

  function unwrap(el) {
    var parent = el.parentNode;
    if (!parent) return;
    while (el.firstChild) parent.insertBefore(el.firstChild, el);
    parent.removeChild(el);
  }

  function applyBlockStyle(selectValue) {
    var tag = "<" + selectValue + ">"; // p, h1, h2, h3, blockquote, pre
    exec("formatBlock", tag);
  }

  /* Font family / size apply as our own command: wrap the current selection in
     a <span> carrying the chosen inline style. execCommand fontName/fontSize
     are too crude (legacy <font>, 1–7 size buckets), so we own this. An empty
     family value means "Default" — we strip any family the selection carries. */
  function applyFont(family) {
    // Always strip inner font-family so the chosen family applies uniformly;
    // an empty family ("Default") leaves the span with no family at all.
    wrapSelectionStyle(function (span) {
      if (family) span.style.fontFamily = cssFontStack(family);
      else clearStyleProp(span, "font-family");
    }, "font-family");
  }

  function applySize(pt) {
    // Strip inner font-size overrides so the new size wins across the selection.
    wrapSelectionStyle(function (span) {
      span.style.fontSize = pt + "pt";
    }, "font-size");
  }

  /* Wrap the selection in a styled span. `apply` sets the new style on the
     span; `clearProp` (optional) names a property to also strip from any
     descendant spans so an inner override does not win over the new value. */
  function wrapSelectionStyle(apply, clearProp) {
    editor.focus();
    var sel = window.getSelection();
    if (!sel || sel.rangeCount === 0) return;
    var range = sel.getRangeAt(0);
    if (range.collapsed) return; // nothing selected — no-op

    var span = document.createElement("span");
    try {
      var frag = range.extractContents();
      if (clearProp) stripStyleProp(frag, clearProp);
      span.appendChild(frag);
      apply(span);
      // If "Default" with no other style left, unwrap to avoid an empty span.
      if (!span.getAttribute("style")) {
        var holder = document.createDocumentFragment();
        while (span.firstChild) holder.appendChild(span.firstChild);
        range.insertNode(holder);
      } else {
        range.insertNode(span);
        var nr = document.createRange();
        nr.selectNodeContents(span);
        sel.removeAllRanges();
        sel.addRange(nr);
      }
    } catch (e) { /* selection spanning blocks — ignore */ }
    markDirty();
    refreshStats();
    refreshToolbar();
  }

  function clearStyleProp(el, prop) {
    if (el.style) el.style.removeProperty(prop);
  }

  /* Remove a style property from every element inside a fragment/subtree. */
  function stripStyleProp(root, prop) {
    var els = (root.querySelectorAll) ? root.querySelectorAll("*") : [];
    for (var i = 0; i < els.length; i++) {
      if (els[i].style) els[i].style.removeProperty(prop);
    }
  }

  /* ================================================================ *
   * Toolbar active-state sync
   * ================================================================ */
  function refreshToolbar() {
    // Inline formats via queryCommandState.
    setPressed(formatButtons.bold,          queryState("bold"));
    setPressed(formatButtons.italic,        queryState("italic"));
    setPressed(formatButtons.underline,     queryState("underline"));
    setPressed(formatButtons.strikeThrough, queryState("strikeThrough"));
    setPressed(formatButtons.code,          !!closestWithin(currentAnchor(), "CODE"));

    // Lists.
    setPressed(listButtons.insertUnorderedList, queryState("insertUnorderedList"));
    setPressed(listButtons.insertOrderedList,   queryState("insertOrderedList"));

    // Alignment — exactly one is active; default to left.
    var aLeft = queryState("justifyLeft");
    var aCenter = queryState("justifyCenter");
    var aRight = queryState("justifyRight");
    var aFull = queryState("justifyFull");
    if (!aCenter && !aRight && !aFull) aLeft = true;
    setPressed(alignButtons.justifyLeft, aLeft && !aCenter && !aRight && !aFull);
    setPressed(alignButtons.justifyCenter, aCenter);
    setPressed(alignButtons.justifyRight, aRight);
    setPressed(alignButtons.justifyFull, aFull);

    // Paragraph-style dropdown reflects the block at the caret.
    syncStyleSelect();

    // Font family / size dropdowns reflect the run at the caret.
    syncFontControls();
  }

  function syncFontControls() {
    var node = currentAnchor();
    var el = (node && node.nodeType === 3) ? node.parentNode : node;

    // Family: first explicit inline family walking up to the editor, else Default.
    var fam = "";
    var n = el;
    while (n && n !== editor && n.nodeType === 1) {
      var f = fontFamilyOf(n);
      if (f != null) { fam = f; break; }
      n = n.parentNode;
    }
    if (fontSelect.value !== fam) fontSelect.value = fam;

    // Size: explicit inline size if any, else fall back to the computed size
    // rounded to points; clamp onto the nearest offered option.
    var sz = null;
    n = el;
    while (n && n !== editor && n.nodeType === 1) {
      var s = fontSizeOf(n);
      if (s != null) { sz = s; break; }
      n = n.parentNode;
    }
    if (sz == null && el && el.nodeType === 1) {
      try {
        var px = parseFloat(window.getComputedStyle(el).fontSize);
        if (isFinite(px)) sz = Math.round(px * 72 / 96);
      } catch (e) { /* ignore */ }
    }
    var want = nearestSizeOption(sz);
    if (want != null && sizeSelect.value !== String(want)) sizeSelect.value = String(want);
  }

  /* Snap a point size onto the closest value present in the size <select>. */
  function nearestSizeOption(pt) {
    if (pt == null) return null;
    var best = null, bestD = Infinity;
    var opts = sizeSelect.options;
    for (var i = 0; i < opts.length; i++) {
      var v = parseInt(opts[i].value, 10);
      var d = Math.abs(v - pt);
      if (d < bestD) { bestD = d; best = v; }
    }
    return best;
  }

  function queryState(cmd) {
    try { return document.queryCommandState(cmd); } catch (e) { return false; }
  }

  function setPressed(btn, on) {
    if (!btn) return;
    btn.setAttribute("aria-pressed", on ? "true" : "false");
  }

  function currentAnchor() {
    var sel = window.getSelection();
    return (sel && sel.anchorNode) ? sel.anchorNode : null;
  }

  function syncStyleSelect() {
    var node = currentAnchor();
    var block = blockAncestor(node);
    var value = "p";
    if (block) {
      var t = block.tagName;
      if (t === "H1") value = "h1";
      else if (t === "H2") value = "h2";
      else if (t === "H3") value = "h3";
      else if (t === "BLOCKQUOTE") value = "blockquote";
      else if (t === "PRE") value = "pre";
      else value = "p";
    }
    if (styleSelect.value !== value) styleSelect.value = value;
  }

  function blockAncestor(node) {
    while (node && node !== editor) {
      if (node.nodeType === 1 && /^(P|H1|H2|H3|BLOCKQUOTE|PRE|LI|DIV)$/.test(node.tagName)) {
        return node;
      }
      node = node.parentNode;
    }
    return null;
  }

  /* ================================================================ *
   * Counts & status bar
   * ================================================================ */
  function updateCountsLocal() {
    var text = editor.innerText || "";
    var words = text.split(/\s+/).filter(function (w) { return w.length > 0; }).length;
    var chars = text.replace(/\r/g, "").length;
    statWords.textContent = String(words);
    statChars.textContent = String(chars);
    statReading.textContent = formatReading(Math.ceil((words / 200) * 60));
  }

  function formatReading(sec) {
    if (!sec || sec < 1) return "0 sec read";
    if (sec < 60) return sec + " sec read";
    var m = Math.round(sec / 60);
    return m + " min read";
  }

  function applyStats(s) {
    if (!s) return;
    statWords.textContent = String(s.words);
    statChars.textContent = String(s.chars);
    statReading.textContent = formatReading(s.reading_time_sec);
  }

  function refreshStats() {
    updateCountsLocal(); // instant, responsive
    if (HAS_TAURI) {
      invoke("document_stats", { doc: serializeDocument() })
        .then(applyStats)
        .catch(function () { /* keep local counts */ });
    }
  }

  /* ================================================================ *
   * Dirty flag / save indicator
   * ================================================================ */
  function markDirty() {
    if (!dirty) {
      dirty = true;
      saveIndicator.classList.remove("is-clean");
      saveIndicator.classList.add("is-dirty");
      saveText.textContent = "Edited";
    }
  }

  function markClean() {
    dirty = false;
    saveIndicator.classList.remove("is-dirty");
    saveIndicator.classList.add("is-clean");
    saveText.textContent = "Saved";
  }

  /* ================================================================ *
   * File operations
   * ================================================================ */
  function doNew() {
    if (HAS_TAURI) {
      invoke("new_document")
        .then(function (doc) { loadDoc(doc, null); })
        .catch(function (e) { toast("Could not create a new document"); console.error(e); });
    } else {
      loadDoc(welcomeDocument(), null);
    }
  }

  function doOpen() {
    if (!HAS_TAURI) { toast("Run the ferric desktop app to open and save files"); return; }
    invoke("open_document")
      .then(function (res) {
        if (!res) return; // user cancelled
        loadDoc(res.doc, res.path);
        toast("Opened " + baseName(res.path));
      })
      .catch(function (e) { toast("Could not open the document"); console.error(e); });
  }

  function doSave(format) {
    if (!HAS_TAURI) { toast("Run the ferric desktop app to open and save files"); return; }
    var doc = serializeDocument();
    // Plain Save reuses the current path & infers format from extension; an
    // explicit "Save as" passes a concrete format and forces the dialog.
    var fmt = format || formatFromPath(currentPath) || "markdown";
    var pathArg = format ? null : currentPath;
    invoke("save_document", { doc: doc, path: pathArg, format: fmt })
      .then(function (res) {
        if (res && res.path) currentPath = res.path;
        markClean();
        toast("Saved " + (currentPath ? baseName(currentPath) : ""));
      })
      .catch(function (e) { toast("Could not save the document"); console.error(e); });
  }

  /* Print uses the browser print pipeline; the @media print stylesheet hides
     all chrome so only the page prints (and OS "Save as PDF" is WYSIWYG).
     Works the same in the desktop webview and a plain browser. */
  function doPrint() {
    try { editor.blur(); } catch (e) { /* ignore */ }
    window.print();
  }

  function loadDoc(doc, path) {
    renderDocument(doc);
    currentPath = path || null;
    markClean();
    refreshStats();
    refreshToolbar();
    placeCaretAtStart();
  }

  function formatFromPath(path) {
    var ext = (path || "").split(".").pop().toLowerCase();
    if (ext === "md" || ext === "markdown") return "markdown";
    if (ext === "rtf") return "rtf";
    if (ext === "docx") return "docx";
    if (ext === "pdf") return "pdf";
    if (ext === "odt") return "odt";
    if (ext === "txt" || ext === "text") return "txt";
    if (ext === "json") return "json";
    return null;
  }

  function baseName(path) {
    if (!path) return "";
    var parts = path.split(/[\\/]/);
    return parts[parts.length - 1] || path;
  }

  /* ================================================================ *
   * Toast notifications
   * ================================================================ */
  var toastTimer = null;
  function toast(msg) {
    var el = document.createElement("div");
    el.className = "toast";
    el.textContent = msg;
    toastRegion.appendChild(el);
    // force reflow so the transition runs
    void el.offsetWidth;
    el.classList.add("show");
    if (toastTimer) clearTimeout(toastTimer);
    setTimeout(function () {
      el.classList.remove("show");
      setTimeout(function () { if (el.parentNode) el.parentNode.removeChild(el); }, 220);
    }, 2600);
  }

  /* ================================================================ *
   * Save-as menu
   * ================================================================ */
  function openSaveMenu() {
    saveAsList.hidden = false;
    btnSaveAs.setAttribute("aria-expanded", "true");
    document.addEventListener("mousedown", onMenuOutside, true);
    document.addEventListener("keydown", onMenuEsc, true);
  }
  function closeSaveMenu() {
    saveAsList.hidden = true;
    btnSaveAs.setAttribute("aria-expanded", "false");
    document.removeEventListener("mousedown", onMenuOutside, true);
    document.removeEventListener("keydown", onMenuEsc, true);
  }
  function onMenuOutside(e) {
    if (!document.getElementById("menu-saveas").contains(e.target)) closeSaveMenu();
  }
  function onMenuEsc(e) {
    if (e.key === "Escape") { closeSaveMenu(); btnSaveAs.focus(); }
  }

  /* ================================================================ *
   * Caret helpers
   * ================================================================ */
  function placeCaretAtStart() {
    try {
      var sel = window.getSelection();
      var range = document.createRange();
      range.selectNodeContents(editor);
      range.collapse(true);
      sel.removeAllRanges();
      sel.addRange(range);
    } catch (e) { /* non-fatal */ }
  }

  /* ================================================================ *
   * Wiring
   * ================================================================ */
  function init() {
    // File buttons.
    btnNew.addEventListener("click", doNew);
    btnOpen.addEventListener("click", doOpen);
    btnSave.addEventListener("click", function () { doSave(null); });

    // Save-as menu.
    btnSaveAs.addEventListener("click", function (e) {
      e.stopPropagation();
      if (saveAsList.hidden) openSaveMenu(); else closeSaveMenu();
    });
    var items = saveAsList.querySelectorAll(".menu-item");
    for (var m = 0; m < items.length; m++) {
      items[m].addEventListener("click", function (e) {
        var fmt = e.currentTarget.getAttribute("data-format");
        closeSaveMenu();
        doSave(fmt);
      });
    }

    // Inline-format toggles (B/I/U/S + code).
    Object.keys(formatButtons).forEach(function (cmd) {
      var btn = formatButtons[cmd];
      if (!btn) return;
      btn.addEventListener("click", function () {
        if (cmd === "code") toggleInlineCode();
        else exec(cmd);
      });
    });

    // List & alignment buttons (all use data-cmd execCommand names).
    [listButtons, alignButtons].forEach(function (group) {
      Object.keys(group).forEach(function (cmd) {
        var btn = group[cmd];
        if (btn) btn.addEventListener("click", function () { exec(cmd); });
      });
    });

    // Paragraph-style dropdown.
    styleSelect.addEventListener("change", function () {
      applyBlockStyle(styleSelect.value);
    });

    // Font family / size dropdowns.
    fontSelect.addEventListener("change", function () {
      applyFont(fontSelect.value);
    });
    sizeSelect.addEventListener("change", function () {
      applySize(parseInt(sizeSelect.value, 10));
    });

    // Print.
    btnPrint.addEventListener("click", doPrint);

    // Editing events.
    editor.addEventListener("input", function () {
      if (suppressInput) return;
      markDirty();
      refreshStats();
    });
    editor.addEventListener("keyup", refreshToolbar);
    editor.addEventListener("mouseup", refreshToolbar);
    editor.addEventListener("focus", refreshToolbar);
    document.addEventListener("selectionchange", function () {
      if (document.activeElement === editor) refreshToolbar();
    });

    // Keyboard shortcuts.
    document.addEventListener("keydown", onKeydown, true);

    // Warn before discarding unsaved work (desktop close / refresh).
    window.addEventListener("beforeunload", function (e) {
      if (dirty) { e.preventDefault(); e.returnValue = ""; }
    });

    // Initial document: backend welcome doc if available, else the built-in
    // welcome so the page looks alive on its own (and in static screenshots).
    if (HAS_TAURI) {
      invoke("new_document")
        .then(function (doc) { loadDoc(doc, null); })
        .catch(function () { loadDoc(welcomeDocument(), null); });
    } else {
      loadDoc(welcomeDocument(), null);
    }
  }

  function onKeydown(e) {
    var mod = e.metaKey || e.ctrlKey;
    if (!mod) return;
    var k = e.key.toLowerCase();

    if (k === "b") { e.preventDefault(); exec("bold"); }
    else if (k === "i") { e.preventDefault(); exec("italic"); }
    else if (k === "u") { e.preventDefault(); exec("underline"); }
    else if (k === "s") { e.preventDefault(); doSave(null); }
    else if (k === "o") { e.preventDefault(); doOpen(); }
    else if (k === "p") { e.preventDefault(); doPrint(); }
  }

  /* ================================================================ *
   * Built-in welcome document (fallback + a realistic first screen)
   * ================================================================ */
  function welcomeDocument() {
    return {
      paragraphs: [
        { style: "H1", align: "Left", runs: [{ text: "Welcome to ferric" }] },
        { style: "Normal", align: "Left", runs: [
          { text: "ferric is a calm, fast word processor. Start typing to make it your own — or format text as " },
          { text: "bold", bold: true },
          { text: ", " },
          { text: "italic", italic: true },
          { text: ", and " },
          { text: "underlined", underline: true },
          { text: " using the toolbar above." }
        ] },
        { style: "H2", align: "Left", runs: [{ text: "A few things you can do" }] },
        { style: "Bullet", align: "Left", runs: [
          { text: "Write headings, quotes, lists, and " },
          { text: "code", code: true },
          { text: " blocks." }
        ] },
        { style: "Bullet", align: "Left", runs: [
          { text: "Save to Markdown, RTF, " },
          { text: "Word (.docx)", bold: true },
          { text: ", or plain text." }
        ] },
        { style: "Bullet", align: "Left", runs: [
          { text: "Watch your word count update live in the status bar." }
        ] },
        { style: "Quote", align: "Left", runs: [
          { text: "Good writing is rewriting. ferric keeps out of your way so you can do both." }
        ] },
        { style: "Normal", align: "Left", runs: [
          { text: "Pick a " },
          { text: "font family", font: "Garamond" },
          { text: " and " },
          { text: "size", size: 18 },
          { text: " from the ribbon to shape your type." }
        ] },
        { style: "Normal", align: "Left", runs: [
          { text: "When you are ready, choose " },
          { text: "Save as", italic: true },
          { text: " to export to Markdown, RTF, Word, " },
          { text: "PDF", bold: true },
          { text: ", or OpenDocument — or " },
          { text: "Print", bold: true },
          { text: " straight from your desktop." }
        ] }
      ]
    };
  }

  /* Boot when the DOM is ready. */
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
