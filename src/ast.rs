//! The wikitext AST (Stage 2).
//!
//! Borrow-friendly: text holds `Cow<'a, str>`, so the common case borrows
//! straight from the input and only *transformed* text (resolved entities,
//! normalized whitespace) allocates. This keeps the engine fast, the way the
//! Stage 1 extractor's five allocating passes are not.
//!
//! Honest by design (DESIGN.md D2): anything we cannot faithfully parse becomes
//! [`Node::Unsupported`], which keeps the original source verbatim and pairs
//! with a diagnostic — we never silently reshape it into something plausible
//! but wrong.

use std::borrow::Cow;

/// A node in the wikitext AST. Inline nodes (`Text`, `Bold`, …) and block nodes
/// (`Heading`, `Paragraph`) share one enum; the parser only nests them in
/// sensible ways.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node<'a> {
    /// Literal text.
    Text(Cow<'a, str>),
    /// `'''bold'''`.
    Bold(Vec<Node<'a>>),
    /// `''italic''`.
    Italic(Vec<Node<'a>>),
    /// `[[target|label]]` — `label` defaults to the target text when absent.
    Link {
        /// The link target (page title or URL).
        target: Cow<'a, str>,
        /// The visible label (inline nodes).
        label: Vec<Node<'a>>,
    },
    /// `== heading ==`, level 1–6.
    Heading {
        /// Heading depth: `==` is 1 … `======` capped at 6.
        level: u8,
        /// The heading's inline content.
        content: Vec<Node<'a>>,
    },
    /// A block of inline content.
    Paragraph(Vec<Node<'a>>),
    /// A list. `ordered` = `#` (numbered) vs `*`/`:`/`;` (bulleted). Each item is
    /// inline content, optionally followed by a nested `List` node for deeper
    /// levels. Definition (`:`/`;`) markers fold into an unordered list (text
    /// kept, not the term/definition split).
    List {
        /// `true` for `#` (numbered), `false` for `*`/`:`/`;` (bulleted).
        ordered: bool,
        /// One entry per list item (each item is inline content).
        items: Vec<Vec<Node<'a>>>,
    },
    /// A leading-space preformatted block; each entry is one line's inline content.
    Preformatted(Vec<Vec<Node<'a>>>),
    /// A table: rows of cells of inline content. Cell attributes and structure
    /// beyond rows×cells aren't preserved; complex (e.g. multi-line-cell) tables
    /// stay Unsupported instead.
    Table {
        /// Rows of cells; each cell is inline content.
        rows: Vec<Vec<Vec<Node<'a>>>>,
    },
    /// A construct outside our declared support range, kept verbatim and
    /// reported via a diagnostic rather than guessed at.
    Unsupported(Cow<'a, str>),
}
