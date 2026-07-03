//! pulldown-cmark event stream → mdnorm NF. Shared by the round-trip
//! integration test and the fuzz target via `#[path]` include (files under
//! tests/support/ are not compiled as test targets on their own).
//!
//! This is the INDEPENDENT side of the round-trip: what our emitted markdown
//! actually means per the CommonMark/GFM spec, as read by someone else's
//! implementation. Events we never intend to emit (inline code, HTML, rules,
//! footnotes…) panic loudly — reaching them means an escaping leak.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use wikrs::mdnorm::{normalize_inlines, NfBlock, NfInline, NfItem};

pub fn markdown_to_nf(md: &str) -> Vec<NfBlock> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(md, opts);
    let mut st = St::default();
    for ev in parser {
        st.event(ev);
    }
    st.blocks
}

#[derive(Default)]
struct St {
    blocks: Vec<NfBlock>,
    inline: Vec<NfInline>,
    bold: u32,
    italic: u32,
    link: Vec<(String, Vec<NfInline>)>, // (href, saved-outer-inline)
    lists: Vec<(bool, Vec<NfItem>)>,    // (ordered, items)
    item_content: Vec<(Vec<NfInline>, Vec<NfBlock>)>, // per open item
    code: Option<(String, String)>,     // (info, text)
    table: Option<Vec<Vec<Vec<NfInline>>>>,
    row: Option<Vec<Vec<NfInline>>>,
}

impl St {
    fn push_run(&mut self, text: &str) {
        self.inline.push(NfInline::Run {
            text: text.to_string(),
            bold: self.bold > 0,
            italic: self.italic > 0,
        });
    }

    fn take_inline(&mut self) -> Vec<NfInline> {
        normalize_inlines(std::mem::take(&mut self.inline))
    }

    /// A tight-list item's text has no Paragraph wrapper, so when a nested
    /// block starts, the pending inline belongs to the currently open item —
    /// flush it there before the block opens, or it leaks into the sublist.
    fn flush_inline_to_open_item(&mut self) {
        if self.inline.is_empty() {
            return;
        }
        if let Some((content, _)) = self.item_content.last_mut() {
            content.append(&mut self.inline);
        }
    }

    fn close_block(&mut self, b: NfBlock) {
        // Route to the innermost open container: item > top level.
        if let Some((content, subs)) = self.item_content.last_mut() {
            match b {
                // Loose-list paragraph unwrap: a paragraph inside an item is
                // item content in NF (tight/loose is not part of the contract).
                NfBlock::Para(inl) => content.extend(inl),
                other => subs.push(other),
            }
        } else {
            self.blocks.push(b);
        }
    }

    fn event(&mut self, ev: Event) {
        match ev {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(t) => {
                if let Some((_, text)) = &mut self.code {
                    text.push_str(&t);
                } else {
                    self.push_run(&t);
                }
            }
            Event::SoftBreak | Event::HardBreak => self.push_run(" "),
            // Inline code back from our output = an escaping bug; be loud.
            Event::Code(t) => panic!("unexpected inline code from our markdown: {t:?}"),
            other => panic!("unexpected event from our markdown: {other:?}"),
        }
    }

    fn start(&mut self, tag: Tag) {
        if matches!(
            tag,
            Tag::Paragraph | Tag::Heading { .. } | Tag::List(_) | Tag::CodeBlock(_) | Tag::Table(_)
        ) {
            self.flush_inline_to_open_item();
        }
        match tag {
            Tag::Paragraph => {}
            Tag::Heading { .. } => {}
            Tag::Strong => self.bold += 1,
            Tag::Emphasis => self.italic += 1,
            Tag::Link { dest_url, .. } => {
                let outer = std::mem::take(&mut self.inline);
                self.link.push((dest_url.to_string(), outer));
            }
            Tag::List(start) => self.lists.push((start.is_some(), Vec::new())),
            Tag::Item => self.item_content.push((Vec::new(), Vec::new())),
            Tag::CodeBlock(kind) => {
                let info = match kind {
                    CodeBlockKind::Fenced(i) => i.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.code = Some((info, String::new()));
            }
            Tag::Table(_) => self.table = Some(Vec::new()),
            Tag::TableHead | Tag::TableRow => self.row = Some(Vec::new()),
            Tag::TableCell => {}
            other => panic!("unexpected tag from our markdown: {other:?}"),
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                let inl = self.take_inline();
                if !inl.is_empty() {
                    self.close_block(NfBlock::Para(inl));
                }
            }
            TagEnd::Heading(level) => {
                let inl = self.take_inline();
                self.close_block(NfBlock::Heading(heading_num(level), inl));
            }
            TagEnd::Strong => self.bold -= 1,
            TagEnd::Emphasis => self.italic -= 1,
            TagEnd::Link => {
                let label = self.take_inline();
                let (href, outer) = self.link.pop().expect("link end without start");
                self.inline = outer;
                self.inline.push(NfInline::Link { href, label });
            }
            TagEnd::Item => {
                let leftover = self.take_inline(); // tight-list item text (no Para wrap)
                let (mut content, subs) = self.item_content.pop().expect("item end");
                content.extend(leftover);
                let (_, items) = self.lists.last_mut().expect("item outside list");
                items.push(NfItem {
                    content: normalize_inlines(content),
                    sublists: subs,
                });
            }
            TagEnd::List(_) => {
                let (ordered, items) = self.lists.pop().expect("list end");
                self.close_block(NfBlock::List { ordered, items });
            }
            TagEnd::CodeBlock => {
                let (info, text) = self.code.take().expect("code end");
                self.close_block(NfBlock::Code {
                    info,
                    text: text.trim_end().to_string(),
                });
            }
            TagEnd::TableCell => {
                let cell = self.take_inline();
                self.row.as_mut().expect("cell outside row").push(cell);
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                let row = self.row.take().expect("row end");
                self.table.as_mut().expect("row outside table").push(row);
            }
            TagEnd::Table => {
                let rows = self.table.take().expect("table end");
                self.close_block(NfBlock::Table { rows });
            }
            _ => {}
        }
    }
}

fn heading_num(l: HeadingLevel) -> u8 {
    match l {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}
