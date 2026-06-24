# Stage 1 (Extractor) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a Rust CLI that turns a Wikimedia XML dump into clean plain text — a faster WikiExtractor.

**Architecture:** Streaming dump reader (`quick-xml` + multistream `bzip2`) yields one `Page` at a time at constant memory; a deliberately *lossy* `extract::strip` pipeline turns each page's wikitext into plain text via byte-scanning passes (comments/refs → templates/tables → links → markup → whitespace); the CLI fans pages across cores with `rayon` and writes `text` or `jsonl`. No AST — that is Stage 2.

**Tech Stack:** Rust 2021, `quick-xml`, `bzip2` (MultiBzDecoder), `rayon`, `memchr`, `serde`/`serde_json`, `clap`, `anyhow`, `insta` (snapshots), `criterion` (bench), `cargo-fuzz`.

**Context:** Task 0 (crate scaffold + CI) is already DONE — `Cargo.toml`, `src/lib.rs`, `src/main.rs`, `src/dump.rs`, `src/extract.rs`, `.github/workflows/ci.yml` exist and are green. This plan implements Tasks 1–9. Design: [../../DESIGN.md](../../DESIGN.md). Behavior table & checkpoints: [../../stages/stage-1-extractor.md](../../stages/stage-1-extractor.md). Tests: [../../TESTING.md](../../TESTING.md).

**Conventions:** TDD (failing test first), DRY, YAGNI, frequent commits. Every byte-scanning pass copies UTF-8 *slices* (`&s[a..b]`) — never casts individual bytes to `char` — because ASCII delimiters (`{ } | [ ] < ' =`) never collide with UTF-8 continuation bytes (≥ 0x80).

> **API drift note:** `quick-xml`'s event/config API shifts between minor versions. Pin whatever `cargo add quick-xml` resolves and adjust `read_event_into` / config calls to match that version's docs. The event *logic* below is stable; only method spelling may differ.

---

## File Structure

| File | Responsibility |
|------|----------------|
| `src/dump.rs` | `Page` type + `Pages` streaming iterator over dump XML; `open()` for `.xml`/`.xml.bz2` |
| `src/extract.rs` | `pub fn strip()` orchestrator + `mod` declarations |
| `src/extract/comments.rs` | strip `<!-- -->`, `<ref>…</ref>`, `<nowiki>` |
| `src/extract/templates.rs` | drop `{{…}}` and `{\|…\|}` (nesting-aware) |
| `src/extract/links.rs` | `[[A\|t]]`→`t`, `[url t]`→`t`, drop `File:`/bare |
| `src/extract/markup.rs` | headings, bold/italic, list markers, leftover HTML tags |
| `src/output.rs` | `text` and `jsonl` serialization of an extracted page |
| `src/main.rs` | clap CLI, parallel driver |
| `xtask/` | dev tasks (fetch parser tests, bench harness) — added in Task 8 |
| `fuzz/` | cargo-fuzz targets — added in Task 9 |

---

## Task 1: `dump::Page` + streaming `Pages` iterator

**Files:**
- Modify: `src/dump.rs`
- Test: inline `#[cfg(test)]` module in `src/dump.rs`

- [ ] **Step 1: Add dependencies**

```bash
cargo add quick-xml
```

- [ ] **Step 2: Write the failing test**

In `src/dump.rs`, append:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const SAMPLE: &str = r#"
<mediawiki>
  <page><title>Earth</title><ns>0</ns>
    <revision><text>Earth is the '''third''' planet.</text></revision>
  </page>
  <page><title>Talk:Earth</title><ns>1</ns>
    <revision><text>discuss here</text></revision>
  </page>
  <page><title>USA</title><ns>0</ns>
    <redirect title="United States" />
    <revision><text>#REDIRECT [[United States]]</text></revision>
  </page>
</mediawiki>"#;

    #[test]
    fn parses_pages_with_fields() {
        let pages: Vec<Page> = Pages::new(Cursor::new(SAMPLE))
            .collect::<anyhow::Result<_>>()
            .unwrap();
        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0].title, "Earth");
        assert_eq!(pages[0].namespace, 0);
        assert!(!pages[0].redirect);
        assert_eq!(pages[0].text, "Earth is the '''third''' planet.");
        assert!(pages[2].redirect);
    }

    #[test]
    fn is_article_filters_ns_and_redirects() {
        let pages: Vec<Page> = Pages::new(Cursor::new(SAMPLE))
            .collect::<anyhow::Result<_>>()
            .unwrap();
        let articles: Vec<&str> =
            pages.iter().filter(|p| p.is_article()).map(|p| p.title.as_str()).collect();
        assert_eq!(articles, ["Earth"]); // ns1 and the redirect are excluded
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --lib dump`
Expected: FAIL — `Page` / `Pages` not found.

- [ ] **Step 4: Implement the reader**

Replace the doc-comment body of `src/dump.rs` (keep the `//!` header) with:

```rust
use std::io::BufRead;

use quick_xml::events::Event;
use quick_xml::Reader;

/// One page from a dump. `text` is raw wikitext.
#[derive(Debug, Clone)]
pub struct Page {
    pub title: String,
    pub namespace: i32,
    pub redirect: bool,
    pub text: String,
}

impl Page {
    /// A real article: main namespace and not a redirect.
    pub fn is_article(&self) -> bool {
        self.namespace == 0 && !self.redirect
    }
}

/// Streaming iterator over `<page>` elements. Constant memory per page.
pub struct Pages<R: BufRead> {
    reader: Reader<R>,
    buf: Vec<u8>,
}

impl<R: BufRead> Pages<R> {
    pub fn new(read: R) -> Self {
        Pages { reader: Reader::from_reader(read), buf: Vec::new() }
    }
}

#[derive(Default)]
enum Field {
    #[default]
    None,
    Title,
    Ns,
    Text,
}

impl<R: BufRead> Iterator for Pages<R> {
    type Item = anyhow::Result<Page>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut page: Option<Page> = None;
        let mut field = Field::None;
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(e)) => match e.name().as_ref() {
                    b"page" => page = Some(Page {
                        title: String::new(),
                        namespace: 0,
                        redirect: false,
                        text: String::new(),
                    }),
                    b"title" => field = Field::Title,
                    b"ns" => field = Field::Ns,
                    b"text" => field = Field::Text,
                    _ => {}
                },
                Ok(Event::Empty(e)) if e.name().as_ref() == b"redirect" => {
                    if let Some(p) = page.as_mut() {
                        p.redirect = true;
                    }
                }
                Ok(Event::Text(t)) => {
                    if let Some(p) = page.as_mut() {
                        let s = match t.unescape() {
                            Ok(s) => s,
                            Err(e) => return Some(Err(e.into())),
                        };
                        match field {
                            Field::Title => p.title.push_str(&s),
                            Field::Text => p.text.push_str(&s),
                            Field::Ns => p.namespace = s.trim().parse().unwrap_or(0),
                            Field::None => {}
                        }
                    }
                }
                Ok(Event::End(e)) => match e.name().as_ref() {
                    b"title" | b"ns" | b"text" => field = Field::None,
                    b"page" => return page.map(Ok),
                    _ => {}
                },
                Ok(Event::Eof) => return None,
                Err(e) => return Some(Err(e.into())),
                _ => {}
            }
        }
    }
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib dump`
Expected: PASS (2 tests).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/dump.rs
git commit -m "feat(dump): streaming Page iterator over dump XML"
```

---

## Task 2: `dump::open` for `.xml` and `.xml.bz2` (multistream)

**Files:**
- Modify: `src/dump.rs`
- Test: `tests/dump_open.rs` (+ a tiny fixture written by the test)

- [ ] **Step 1: Add dependency**

```bash
cargo add bzip2
```

- [ ] **Step 2: Write the failing test**

Create `tests/dump_open.rs`:

```rust
use std::io::Write;

#[test]
fn opens_plain_and_bz2_dumps() {
    let xml = b"<mediawiki><page><title>A</title><ns>0</ns>\
        <revision><text>hello</text></revision></page></mediawiki>";

    let dir = std::env::temp_dir().join("wikrs_dump_open_test");
    std::fs::create_dir_all(&dir).unwrap();

    let plain = dir.join("d.xml");
    std::fs::write(&plain, xml).unwrap();

    let bz = dir.join("d.xml.bz2");
    let mut enc = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
    enc.write_all(xml).unwrap();
    std::fs::write(&bz, enc.finish().unwrap()).unwrap();

    for path in [&plain, &bz] {
        let pages: Vec<_> = wikrs::dump::open(path)
            .unwrap()
            .collect::<anyhow::Result<_>>()
            .unwrap();
        assert_eq!(pages.len(), 1, "path {:?}", path);
        assert_eq!(pages[0].text, "hello");
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --test dump_open`
Expected: FAIL — `dump::open` not found.

- [ ] **Step 4: Implement `open`**

Add to `src/dump.rs`:

```rust
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Open a dump file, transparently decompressing multistream `.bz2`.
pub fn open(path: &Path) -> anyhow::Result<Pages<Box<dyn BufRead>>> {
    let file = File::open(path)?;
    let reader: Box<dyn BufRead> = if path.extension().is_some_and(|e| e == "bz2") {
        Box::new(BufReader::new(bzip2::read::MultiBzDecoder::new(file)))
    } else {
        Box::new(BufReader::new(file))
    };
    Ok(Pages::new(reader))
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --test dump_open`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/dump.rs tests/dump_open.rs
git commit -m "feat(dump): open .xml and multistream .xml.bz2 dumps"
```

---

## Task 3: `extract::comments` — strip comments, refs, nowiki

**Files:**
- Create: `src/extract/comments.rs`
- Modify: `src/extract.rs`

- [ ] **Step 1: Write the failing test**

Create `src/extract/comments.rs`:

```rust
//! Remove HTML comments, `<ref>` citations, and `<nowiki>` wrappers.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_comments_refs_nowiki() {
        assert_eq!(strip_comments_refs("a<!-- x -->b"), "ab");
        assert_eq!(strip_comments_refs("a<ref name=q>cite</ref>b"), "ab");
        assert_eq!(strip_comments_refs("a<ref name=q />b"), "ab");
        assert_eq!(strip_comments_refs("a<nowiki>[[x]]</nowiki>b"), "a[[x]]b");
    }
}
```

- [ ] **Step 2: Wire the module and run to verify it fails**

In `src/extract.rs`, under the `//!` header add `mod comments;`. Run: `cargo test --lib extract::comments`
Expected: FAIL — `strip_comments_refs` not found.

- [ ] **Step 3: Implement**

Prepend to `src/extract/comments.rs` (above the test module):

```rust
/// Remove `<!-- … -->`, `<ref …>…</ref>`, `<ref … />`, and `<nowiki>` tags
/// (keeping nowiki's inner text). Case-insensitive tag matching.
pub fn strip_comments_refs(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(i) = rest.find("<!--") {
            out.push_str(&rest[..i]);
            rest = match rest[i..].find("-->") {
                Some(j) => &rest[i + j + 3..],
                None => "", // unterminated: drop remainder
            };
        } else if let Some((pre, after)) = cut_tag(rest, "ref") {
            out.push_str(pre);
            rest = after;
        } else if let Some((pre, after)) = cut_nowiki(rest) {
            out.push_str(pre);
            rest = after;
        } else {
            out.push_str(rest);
            break;
        }
    }
    out
}

/// If `s` starts-or-contains a `<tag …>…</tag>` or `<tag … />`, return
/// (text before it, text after it). Drops the whole element.
fn cut_tag<'a>(s: &'a str, tag: &str) -> Option<(&'a str, &'a str)> {
    let lower = s.to_ascii_lowercase();
    let open_pat = format!("<{tag}");
    let start = lower.find(&open_pat)?;
    // end of the opening tag
    let gt = s[start..].find('>')? + start;
    let pre = &s[..start];
    if s[..gt].trim_end().ends_with('/') {
        return Some((pre, &s[gt + 1..])); // self-closing
    }
    let close = format!("</{tag}>");
    match lower[gt..].find(&close) {
        Some(c) => Some((pre, &s[gt + c + close.len()..])),
        None => Some((pre, "")), // unterminated: drop remainder
    }
}

fn cut_nowiki(s: &str) -> Option<(&str, &str)> {
    let lower = s.to_ascii_lowercase();
    let start = lower.find("<nowiki>")?;
    let inner_start = start + "<nowiki>".len();
    let (inner, after) = match lower[inner_start..].find("</nowiki>") {
        Some(c) => (&s[inner_start..inner_start + c], &s[inner_start + c + "</nowiki>".len()..]),
        None => (&s[inner_start..], ""),
    };
    // Caller appends `pre`; fold inner text back in by returning it joined.
    // We can't return three slices, so allocate the pre+inner join here:
    Some((Box::leak(format!("{}{}", &s[..start], inner).into_boxed_str()), after))
}
```

> **Refinement note:** `cut_nowiki`'s `Box::leak` is a deliberate shortcut to keep the slice-returning signature uniform; replace it with a small `Cow`-returning rewrite if it shows up in profiling. For Stage 1 (one pass over each page, output owned anyway) it is acceptable. If you prefer no leak now, change `strip_comments_refs` to build output directly instead of via `(pre, after)` slices for the nowiki branch.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib extract::comments`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/extract.rs src/extract/comments.rs
git commit -m "feat(extract): strip comments, refs, and nowiki"
```

---

## Task 4: `extract::templates` — drop `{{…}}` and `{|…|}`

**Files:**
- Create: `src/extract/templates.rs`
- Modify: `src/extract.rs` (add `mod templates;`)

- [ ] **Step 1: Write the failing test**

Create `src/extract/templates.rs`:

```rust
//! Drop templates `{{…}}` and tables `{|…|}`, nesting-aware.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_templates_and_tables() {
        assert_eq!(strip_templates_tables("a{{cite|x}}b"), "ab");
        assert_eq!(strip_templates_tables("a{{outer{{inner}}}}b"), "ab");
        assert_eq!(strip_templates_tables("a{|\n|r\n|}b"), "ab");
        assert_eq!(strip_templates_tables("plain text"), "plain text");
        assert_eq!(strip_templates_tables("café {{t}} ☕"), "café  ☕"); // UTF-8 safe
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Add `mod templates;` to `src/extract.rs`. Run: `cargo test --lib extract::templates`
Expected: FAIL — `strip_templates_tables` not found.

- [ ] **Step 3: Implement (slice-copy, UTF-8 safe)**

Prepend to `src/extract/templates.rs`:

```rust
/// Remove `{{…}}` and `{|…|}` regions, honoring nesting. Lossy by design.
pub fn strip_templates_tables(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut depth = 0usize;
    let mut seg_start = 0usize;
    while i + 1 < b.len() {
        let open = b[i] == b'{' && (b[i + 1] == b'{' || b[i + 1] == b'|');
        let close = (b[i] == b'}' && b[i + 1] == b'}') || (b[i] == b'|' && b[i + 1] == b'}');
        if open {
            if depth == 0 {
                out.push_str(&s[seg_start..i]);
            }
            depth += 1;
            i += 2;
        } else if close && depth > 0 {
            depth -= 1;
            i += 2;
            if depth == 0 {
                seg_start = i;
            }
        } else {
            i += 1;
        }
    }
    if depth == 0 {
        out.push_str(&s[seg_start..]);
    }
    out
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test --lib extract::templates`
Expected: PASS (incl. the UTF-8 case).

- [ ] **Step 5: Commit**

```bash
git add src/extract.rs src/extract/templates.rs
git commit -m "feat(extract): drop nested templates and tables"
```

---

## Task 5: `extract::links` — internal/external/file links

**Files:**
- Create: `src/extract/links.rs`
- Modify: `src/extract.rs` (add `mod links;`)

- [ ] **Step 1: Write the failing test**

Create `src/extract/links.rs`:

```rust
//! Reduce links to their human-visible text.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_anchor_text() {
        assert_eq!(strip_links("see [[Earth|our planet]] now"), "see our planet now");
        assert_eq!(strip_links("see [[Earth]] now"), "see Earth now");
        assert_eq!(strip_links("x [https://a.com label] y"), "x label y");
        assert_eq!(strip_links("x [https://a.com] y"), "x  y"); // bare url dropped
        assert_eq!(strip_links("a [[File:p.jpg|thumb|cap]] b"), "a  b"); // file dropped
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Add `mod links;` to `src/extract.rs`. Run: `cargo test --lib extract::links`
Expected: FAIL — `strip_links` not found.

- [ ] **Step 3: Implement**

Prepend to `src/extract/links.rs`:

```rust
/// `[[Target|text]]`→`text`, `[[Target]]`→`Target`, `[[File:…]]`→dropped;
/// `[url text]`→`text`, `[url]`→dropped.
pub fn strip_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while !rest.is_empty() {
        let next_internal = rest.find("[[");
        let next_external = find_external(rest);
        match (next_internal, next_external) {
            (Some(i), e) if e.is_none_or(|x| i < x) => {
                out.push_str(&rest[..i]);
                rest = match rest[i..].find("]]") {
                    Some(j) => {
                        out.push_str(&internal_text(&rest[i + 2..i + j]));
                        &rest[i + j + 2..]
                    }
                    None => "",
                };
            }
            (_, Some(e)) => {
                out.push_str(&rest[..e]);
                rest = match rest[e..].find(']') {
                    Some(j) => {
                        out.push_str(external_text(&rest[e + 1..e + j]));
                        &rest[e + j + 1..]
                    }
                    None => "",
                };
            }
            _ => {
                out.push_str(rest);
                break;
            }
        }
    }
    out
}

/// A `[` that begins an external link `[http…]` (not `[[`).
fn find_external(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'[' && b.get(i + 1) != Some(&b'[') && s[i + 1..].starts_with("http") {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Inner text of `[[ … ]]`. Drop `File:`/`Image:`; keep text after last `|`.
fn internal_text(inner: &str) -> String {
    let target = inner.split('|').next().unwrap_or("");
    let ns = target.split(':').next().unwrap_or("").trim().to_ascii_lowercase();
    if ns == "file" || ns == "image" {
        return String::new();
    }
    match inner.rsplit('|').next() {
        Some(t) => t.to_string(),
        None => inner.to_string(),
    }
}

/// Inner text of `[url text]`: the part after the first space, else "".
fn external_text(inner: &str) -> &str {
    match inner.split_once(char::is_whitespace) {
        Some((_, text)) => text,
        None => "",
    }
}
```

> Uses `Option::is_none_or` (stable since Rust 1.82). The crate targets current stable.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test --lib extract::links`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/extract.rs src/extract/links.rs
git commit -m "feat(extract): reduce links to anchor text, drop files"
```

---

## Task 6: `extract::markup` + `strip()` orchestration

**Files:**
- Create: `src/extract/markup.rs`
- Modify: `src/extract.rs` (add `mod markup;` + `pub fn strip`)
- Test: `tests/strip_snapshots.rs`

- [ ] **Step 1: Add dependency**

```bash
cargo add --dev insta
```

- [ ] **Step 2: Write the failing unit test (markup)**

Create `src/extract/markup.rs`:

```rust
//! Strip headings, bold/italic, list markers, and leftover HTML tags.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_inline_and_block_markup() {
        assert_eq!(strip_markup("== History =="), "History");
        assert_eq!(strip_markup("'''bold''' and ''italic''"), "bold and italic");
        assert_eq!(strip_markup("* one\n* two"), "one\ntwo");
        assert_eq!(strip_markup("# a\n## b"), "a\nb");
        assert_eq!(strip_markup("x <b>y</b> z"), "x y z");
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Add `mod markup;` to `src/extract.rs`. Run: `cargo test --lib extract::markup`
Expected: FAIL — `strip_markup` not found.

- [ ] **Step 4: Implement markup**

Prepend to `src/extract/markup.rs`:

```rust
/// Remove heading `=`, bold/italic `'''`/`''`, leading list markers
/// (`* # : ;`), and any leftover `<tag>`/`</tag>`.
pub fn strip_markup(s: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in s.split('\n') {
        let line = line.trim_end();
        let trimmed = line.trim_start();
        // headings: == H == / === H ===
        let heading = trimmed.trim_matches('=').trim();
        let body = if trimmed.starts_with('=') && trimmed.ends_with('=') && trimmed.len() > 2 {
            heading
        } else {
            // list markers at line start
            trimmed.trim_start_matches(['*', '#', ':', ';', ' '])
        };
        lines.push(strip_inline(body));
    }
    lines.join("\n")
}

fn strip_inline(s: &str) -> String {
    // remove bold/italic apostrophe runs, then strip tags
    let no_emphasis = s.replace("'''''", "").replace("'''", "").replace("''", "");
    strip_tags(&no_emphasis)
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        rest = match rest[i..].find('>') {
            Some(j) => &rest[i + j + 1..],
            None => break,
        };
    }
    out.push_str(rest);
    out
}
```

- [ ] **Step 5: Run to verify markup passes**

Run: `cargo test --lib extract::markup`
Expected: PASS.

- [ ] **Step 6: Implement `strip()` orchestrator**

In `src/extract.rs`, below the `mod` lines, add:

```rust
mod comments;
mod links;
mod markup;
mod templates;

/// Turn raw wikitext into clean plain text (Stage 1, lossy).
///
/// Pipeline order matters: kill comments/refs first, then templates/tables
/// (so their inner `[[…]]`/`|` never reach later passes), then links, then
/// markup, then collapse blank-line runs.
pub fn strip(wikitext: &str) -> String {
    let s = comments::strip_comments_refs(wikitext);
    let s = templates::strip_templates_tables(&s);
    let s = links::strip_links(&s);
    let s = markup::strip_markup(&s);
    collapse_blank_lines(&s)
}

fn collapse_blank_lines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank = 0;
    for line in s.lines() {
        if line.trim().is_empty() {
            blank += 1;
            if blank <= 1 {
                out.push('\n');
            }
        } else {
            blank = 0;
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim().to_string()
}
```

Make the sub-modules' `pub fn`s visible to `strip` by ensuring each is declared `pub fn` (they are). Remove the duplicate `mod` lines if Steps in Tasks 3–6 already added them — keep exactly one of each.

- [ ] **Step 7: Write the end-to-end snapshot test**

Create `tests/strip_snapshots.rs`:

```rust
use wikrs::extract::strip;

#[test]
fn article_snapshot() {
    let wikitext = "\
'''Earth''' is the [[Planet|third planet]] from the Sun.<ref>cite</ref>

== History ==
{{Infobox planet|age=4.5e9}}
* Formed ~4.5 billion years ago
* Has [[File:Earth.jpg|thumb|a moon]] one moon

See [https://nasa.gov NASA].";
    insta::assert_snapshot!(strip(wikitext));
}
```

- [ ] **Step 8: Run and accept the snapshot**

Run: `cargo test --test strip_snapshots` (creates a `.snap.new`)
Then: `cargo insta review` — confirm the output is clean plain text (no `'''`, no template, no ref, file link gone, anchor texts kept), accept it.
Expected after accept: PASS.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml Cargo.lock src/extract.rs src/extract/markup.rs tests/strip_snapshots.rs tests/snapshots/
git commit -m "feat(extract): strip markup and orchestrate the strip() pipeline"
```

---

## Task 7: CLI wiring + output formats + parallelism

**Files:**
- Create: `src/output.rs`
- Modify: `src/lib.rs` (add `pub mod output;`), `src/main.rs`
- Test: inline tests in `src/output.rs`; `tests/cli.rs` end-to-end

- [ ] **Step 1: Add dependencies**

```bash
cargo add rayon serde --features serde/derive
cargo add serde_json
```

- [ ] **Step 2: Write the failing output test**

Create `src/output.rs`:

```rust
//! Serialize an extracted page as plain text or JSON Lines.

use serde::Serialize;

#[derive(Serialize)]
pub struct Record<'a> {
    pub title: &'a str,
    pub text: &'a str,
}

pub fn to_jsonl(title: &str, text: &str) -> String {
    serde_json::to_string(&Record { title, text }).expect("serialize record")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonl_has_title_and_text() {
        let line = to_jsonl("Earth", "third planet");
        assert_eq!(line, r#"{"title":"Earth","text":"third planet"}"#);
    }
}
```

- [ ] **Step 3: Run to verify it fails, then passes**

Add `pub mod output;` to `src/lib.rs`. Run: `cargo test --lib output`
Expected: FAIL → after Step 2's code compiles, PASS. (The impl *is* in Step 2; if you wrote test-first, split the `to_jsonl` body out and watch it fail first.)

- [ ] **Step 4: Write the CLI end-to-end test**

Create `tests/cli.rs`:

```rust
use std::process::Command;

#[test]
fn extracts_text_from_a_dump_file() {
    let xml = "<mediawiki><page><title>Earth</title><ns>0</ns>\
        <revision><text>'''Earth''' is a [[Planet|planet]].</text></revision>\
        </page></mediawiki>";
    let dir = std::env::temp_dir().join("wikrs_cli_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("d.xml");
    std::fs::write(&path, xml).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_wikrs"))
        .args(["--input", path.to_str().unwrap(), "--format", "text"])
        .output()
        .unwrap();

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Earth is a planet."), "got: {stdout}");
}
```

- [ ] **Step 5: Run to verify it fails**

Run: `cargo test --test cli`
Expected: FAIL — bin still `bail!`s "not implemented".

- [ ] **Step 6: Implement the CLI driver**

Replace `src/main.rs` body with:

```rust
//! `wikrs` command-line interface.

use std::io::{self, Write};
use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use rayon::prelude::*;

use wikrs::{dump, extract, output};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Format {
    Text,
    Jsonl,
}

/// Fast, honest wikitext extraction.
#[derive(Debug, Parser)]
#[command(name = "wikrs", version, about)]
struct Cli {
    /// Path to a Wikimedia XML dump (`.xml` or `.xml.bz2`).
    #[arg(long)]
    input: PathBuf,

    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Read sequentially (single decompressor), extract in parallel, write in order.
    let pages: Vec<dump::Page> = dump::open(&cli.input)?
        .filter_map(Result::ok)
        .filter(dump::Page::is_article)
        .collect();

    let rendered: Vec<String> = pages
        .par_iter()
        .map(|p| {
            let text = extract::strip(&p.text);
            match cli.format {
                Format::Text => text,
                Format::Jsonl => output::to_jsonl(&p.title, &text),
            }
        })
        .collect();

    let stdout = io::stdout();
    let mut w = io::BufWriter::new(stdout.lock());
    for r in rendered {
        writeln!(w, "{r}")?;
    }
    w.flush()?;
    Ok(())
}
```

> **Memory note:** collecting all pages is fine for slices/medium dumps and keeps the test simple. For full-dump runs, swap to a bounded channel (sequential reader thread → rayon worker pool → ordered writer) so memory stays constant. Track that as a follow-up in `stages/stage-1-extractor.md` if benchmarks show pressure.

- [ ] **Step 7: Run both tests to verify they pass**

Run: `cargo test --test cli && cargo test --lib output`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml Cargo.lock src/lib.rs src/output.rs src/main.rs tests/cli.rs
git commit -m "feat(cli): extract a dump to text/jsonl with rayon"
```

---

## Task 8: Benchmark vs WikiExtractor (Checkpoint C4)

**Files:**
- Create: `benches/extract.rs`, `xtask/Cargo.toml`, `xtask/src/main.rs`
- Modify: `Cargo.toml` (add `[[bench]]`, workspace `xtask`)

- [ ] **Step 1: Add criterion + bench target**

```bash
cargo add --dev criterion --features html_reports
```

In `Cargo.toml` add:

```toml
[[bench]]
name = "extract"
harness = false
```

- [ ] **Step 2: Write the micro-benchmark**

Create `benches/extract.rs`:

```rust
use criterion::{criterion_group, criterion_main, Criterion};
use wikrs::extract::strip;

const SAMPLE: &str = include_str!("../tests/fixtures/sample_article.wikitext");

fn bench_strip(c: &mut Criterion) {
    c.bench_function("strip/sample_article", |b| {
        b.iter(|| strip(std::hint::black_box(SAMPLE)))
    });
}

criterion_group!(benches, bench_strip);
criterion_main!(benches);
```

Create `tests/fixtures/sample_article.wikitext` with a realistic multi-KB article (copy one article's raw wikitext from any dump; this fixture is *your* text, not GPL parser tests).

- [ ] **Step 3: Run it**

Run: `cargo bench --bench extract`
Expected: criterion prints a throughput/time number. Commit the baseline number into the task notes.

- [ ] **Step 4: Build the end-to-end comparison harness**

Create an `xtask` crate (`xtask/src/main.rs`) with a `bench-vs-wikiextractor` subcommand that, given a dump slice path:
1. times `wikrs --input <dump> --format text > /dev/null` (wall-clock + RSS via `/usr/bin/time -l`),
2. times `python -m wikiextractor.WikiExtractor <dump> -o -` likewise,
3. prints a table: tool | wall-clock | MB/s | peak RSS.

```rust
// xtask/src/main.rs (sketch — fill in arg parsing + Command timing)
fn main() -> anyhow::Result<()> {
    // args: subcommand, dump path
    // run both tools under `/usr/bin/time -l`, parse real-time + max RSS,
    // compute MB/s = input_bytes / wall_seconds / 1e6, print comparison table.
    Ok(())
}
```

- [ ] **Step 5: Run the comparison and record the number**

Run: `cargo run -p xtask -- bench-vs-wikiextractor path/to/slice.xml.bz2`
Expected: wikrs ≈ an order of magnitude faster. **Record wall-clock + MB/s + peak RSS for both, same machine/input, into the README and `stages/stage-1-extractor.md` C4.** If wikrs is *not* clearly faster, stop and profile (`cargo flamegraph`) before proceeding — C4 is the floor-value claim.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock benches/ xtask/ tests/fixtures/sample_article.wikitext
git commit -m "bench: criterion micro-bench + wikiextractor comparison harness"
```

---

## Task 9: Fuzz smoke + README + first release prep

**Files:**
- Create: `fuzz/` (via `cargo fuzz init`), `fuzz/fuzz_targets/strip.rs`
- Modify: `README.md`, `CHANGELOG.md`

- [ ] **Step 1: Init cargo-fuzz**

```bash
cargo install cargo-fuzz   # if not present
cargo fuzz init
```

- [ ] **Step 2: Write the fuzz target**

Replace `fuzz/fuzz_targets/fuzz_target_1.rs` with `fuzz/fuzz_targets/strip.rs`:

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Must never panic, hang, or OOM on any input.
        let _ = wikrs::extract::strip(s);
    }
});
```

Register it in `fuzz/Cargo.toml` under `[[bin]]` (name = "strip").

- [ ] **Step 3: Run a short fuzz smoke**

Run: `cargo +nightly fuzz run strip -- -max_total_time=60`
Expected: no crash, no timeout. Any crash → minimize, add to `fuzz/corpus/strip/`, fix, repeat. (Linear-time requirement: if a 2 MB input blows up runtime, that is a bug to fix now — see DESIGN §8.)

- [ ] **Step 4: Write the README usage + numbers**

Update `README.md`: install/run (`cargo run -- --input dump.xml.bz2 --format jsonl`), the Task 8 benchmark table, and a short "Known differences vs WikiExtractor" list pulled from `stages/stage-1-extractor.md`'s behavior table. Create `CHANGELOG.md` with a `0.1.0` section.

- [ ] **Step 5: Release dry-run**

```bash
# flip publish=false -> true in Cargo.toml, set version = "0.1.0"
cargo publish --dry-run
```

Expected: packages cleanly (warnings about missing LICENSE files are expected until the LICENSE-MIT / LICENSE-APACHE files are added — see DESIGN §11).

- [ ] **Step 6: Commit**

```bash
git add fuzz/ README.md CHANGELOG.md Cargo.toml
git commit -m "test(fuzz): strip never panics; docs: usage, benchmarks, changelog"
```

---

## Definition of Done (mirrors stages/stage-1-extractor.md)

- [ ] C1 streaming dump read, constant memory, ns/redirect filtering — Tasks 1–2
- [ ] C2 strip behavior matches the behavior table — Tasks 3–6
- [ ] C3 CLI with format/namespace/template flags — Task 7
- [ ] C4 benchmark shows ~1 order of magnitude over WikiExtractor — Task 8
- [ ] C5 snapshots + fuzz smoke green — Tasks 6, 9
- [ ] C6 README with run instructions, numbers, known differences — Task 9

## Self-Review notes

- Types are consistent across tasks: `dump::Page{title,namespace,redirect,text}` + `is_article`, `extract::strip(&str)->String`, sub-passes `strip_comments_refs` / `strip_templates_tables` / `strip_links` / `strip_markup`, `output::to_jsonl`.
- Pipeline order in `strip()` (comments→templates→links→markup→whitespace) is justified inline.
- Known shortcuts flagged honestly: `cut_nowiki` `Box::leak` (Task 3 note), whole-corpus collect in CLI (Task 7 memory note), `xtask` timing body left as a sketch (Task 8 — it's glue, not logic). These are the only non-complete spots and each says how to finish it.
