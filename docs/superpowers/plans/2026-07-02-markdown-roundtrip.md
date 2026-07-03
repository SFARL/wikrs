# render::markdown + 往返 harness 实施计划（M1–M4）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (inline) to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `render::markdown`（AST → GFM 文本）+ CLI `--format markdown`，正确性由 pulldown-cmark 往返 harness 外部裁决（先红后绿）。

**Architecture:** 双方各自映射进一个共享的**规范形（Normal Form, NF）**再断言相等：wikrs 侧 `AST → NF`（声明意图），pulldown 侧 `我们输出的 markdown → pulldown-cmark 事件流 → NF`（独立实现眼中的实际含义）。渲染器不能给自己打分——它输出文本的含义由外人（pulldown-cmark，CommonMark/GFM 规范的独立实现）决定。

**Tech Stack:** pulldown-cmark（**仅 dev-dependency + fuzz workspace**，不进发布依赖）、insta、cargo-fuzz。

---

## §0 准确性到底怎么比（本计划的地基，先读这节）

### 机制

```
                    （声明意图 —— 人审的契约表）
   wikitext ──parse──► AST ────────── mdnorm::from_ast ──────────► NF_intent
                        │                                             │
                        └─ render::markdown ─► markdown 文本          ├─ assert_eq!
                                                  │                   │
                （独立裁判 —— 别人写的 GFM 实现）  ▼                   │
                              pulldown-cmark 事件流 ── events→NF ──► NF_actual
```

对每个输入断言 `NF_intent == NF_actual`。**红 = 渲染器发出的文本，在世界（GFM 规范）眼里不是我们想表达的结构。**

### 它抓什么（正是上次 `==`→h3 那类 bug）

- **层级映射错**：AST `Heading{level:2}` 的 NF_intent 是 `Heading(2,…)`；渲染器若发 `###`，pulldown 解出 H3 → NF_actual 是 `Heading(3,…)` → 红。
- **转义泄漏**：AST `Text("a*b*c")` 的 NF_intent 是无格式 Run；渲染器不转义 → pulldown 解出 Emphasis → NF_actual 带 italic 标志 → 红。行首 `#`、`1. `、表格内 `|`、`[`、`<` 全同理。
- **结构走样**：嵌套列表缩进错一格 → pulldown 解成兄弟列表而非子列表 → 红。表格少了分隔行 → pulldown 根本不认表格 → 红。

### 它抓不了什么（诚实边界）

NF_intent 的映射本身（"Bold ↦ bold 标志"这张表）是我们写的——如果表本身声明错了（比如声明 Bold ↦ italic），两侧一起错、一起绿。残余风险的去处：**这张表是声明式的、人审的**（就在下方"NF 映射契约"），且 insta 快照锁住人眼可读的输出。相比之下，命令式渲染代码里的错误（转义、缩进、fence、层级算术）全部暴露给独立裁判。**上次事故的教训覆盖住了：错误藏在实现里会被抓，只可能藏在一张人人可读的五行表里。**

### 防循环论证

wikrs 侧与渲染器**只共享两样**：`md_href`（内链 href 规则）与实体解码——它们本身就是契约条目，比对不该重推导它们，而是拿它们钉住渲染器。需要独立裁决的东西——块形状、标题层级、列表嵌套、**全部转义**——不共享，由 pulldown 说了算。

### NF 映射契约（人审的那张表）

| AST | NF（两侧都归一到此） | 声明的归一化 |
|---|---|---|
| `Text` | `Run{text, bold, italic}` | 实体解码；空白串折叠为单空格；同格式相邻 Run 合并；块首尾去空白；空 Run 丢弃 |
| `Bold`/`Italic` | Run 上的布尔标志（**拍平，不保留嵌套顺序**） | `Bold(Italic(x))` ≡ `Italic(Bold(x))`——语义是"哪段字什么格式"，不是树形 |
| `Link` | `Link{href, label:Vec<Run>}` | 内链 href 过 `md_href`（`./`+下划线+RFC3986 百分号编码，契约与 sections 时代一致）；外链原样；空 label（裸外链）→ label = href 文本 |
| `Heading{level}` | `Heading(level.clamp(1,6), inlines)` | level=等号数直映（`==`→2→`##`） |
| `Paragraph` | `Para(inlines)` | 归一化后空 → 整块丢弃 |
| `List{ordered,items}` | `List{ordered, items:[{content, sublists}]}` | 有序列表起始编号不比（恒发 `1.`）；紧列表（item 内单段落解包） |
| `Preformatted` | `Code{info:"", text}` | **行内格式拍平成纯文本**（fenced block 只能装字面文本——声明损失）；尾换行去除 |
| `Unsupported` | `Code{info:"wikitext", text:源码逐字}` | M3 可见标记：fence 长度=内容最长反引号串+1（≥3），info 串 `wikitext` 可 CSS/grep 定位 |
| `Table{rows}` | `Table{rows}`（cell=Vec<Run/Link>） | GFM 强制首行为表头：渲染 row0 为 header，NF 把 pulldown 的 head+body **拼回一个 rows**；对齐说明符不比；空表丢弃 |

pulldown 侧额外归一：相邻 `Text` 事件拼接；`SoftBreak`/`HardBreak` → 空格；`Code`（行内码）→ 普通 Run（我们不发行内码，若出现即红——转义漏了反引号）；出现 `Html`/`Rule`/脚注等我们从不发的事件 → 直接 panic 红。

### 语料（喂 harness 的输入，从小到大）

1. **手工 AST 用例**（M1 起常黑：转义炸弹、嵌套、空边界）。
2. **parserTests 全部 1077 条的 wikitext**（GPL fixture 缺失时软跳过，同既有 harness）——每条 parse 出 AST 即入比对，不筛诊断（Unsupported 走 fence 契约）。
3. **样例文章 fixture** + insta 快照（人眼层）。
4. **fuzz**（M3）：任意输入 → parse → render → 同一往返断言。

---

## File structure

| 文件 | 职责 |
|---|---|
| `src/mdnorm.rs`（新，`#[doc(hidden)] pub`） | NF 类型、`from_ast`、`normalize_inlines`、`md_href`。无新依赖（pulldown 不进 lib） |
| `src/render/markdown.rs`（新） | 渲染器。M1 先桩（空串），M2 实装 |
| `src/render.rs`（改） | `mod markdown; pub use markdown::markdown;` |
| `src/lib.rs`（改） | 注册 `#[doc(hidden)] pub mod mdnorm;` |
| `tests/support/pulldown_nf.rs`（新） | pulldown 事件流 → NF（integration test 与 fuzz 用 `#[path]` 共享；`tests/` 子目录不会被当作 test target） |
| `tests/markdown_roundtrip.rs`（新） | harness 本体：手工用例 + parserTests 语料 |
| `tests/markdown_snapshots.rs`（新，M4） | insta 快照 |
| `fuzz/fuzz_targets/markdown_roundtrip.rs`（新，M3） | 往返性质 fuzz |
| `src/main.rs`、`src/output.rs`（改，M4） | `--format markdown`、`to_markdown(title, body)` |
| `Cargo.toml`（改） | dev-dep `pulldown-cmark = "0.13"`；`fuzz/Cargo.toml` 同名 dep |

---

## Task M1: harness 先行（对桩渲染器跑红）

### M1.1 依赖 + NF 类型 + from_ast

- [ ] **Step 1:** `cargo add --dev pulldown-cmark@0.13`（若 0.13 不存在按 cargo 报错选最近版；API 差异在 M1.4 编译时消化）
- [ ] **Step 2:** 建 `src/mdnorm.rs`：

```rust
//! Markdown round-trip normal form (Stage 3 M-line). #[doc(hidden)]: dev/test
//! plumbing, no semver promise. Both sides of the round-trip harness map into
//! these types; equality here is the definition of "the markdown means what
//! the AST says". The mapping contract table lives in
//! docs/superpowers/plans/2026-07-02-markdown-roundtrip.md §0.

use crate::ast::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NfBlock {
    Heading(u8, Vec<NfInline>),
    Para(Vec<NfInline>),
    List { ordered: bool, items: Vec<NfItem> },
    Code { info: String, text: String },
    Table { rows: Vec<Vec<Vec<NfInline>>> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NfItem {
    pub content: Vec<NfInline>,
    pub sublists: Vec<NfBlock>, // List only
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NfInline {
    Run { text: String, bold: bool, italic: bool },
    Link { href: String, label: Vec<NfInline> }, // Runs only
}

/// The pinned internal-link href rule (same contract as the sections era):
/// spaces → `_`, RFC 3986 path charset kept, the rest percent-encoded,
/// `./` prefix forecloses scheme injection. External targets pass through.
pub fn md_href(target: &str) -> String {
    if ["http://", "https://", "ftp://", "mailto:", "//"]
        .iter()
        .any(|p| target.starts_with(p))
    {
        return target.to_string();
    }
    let mut href = String::with_capacity(target.len() + 2);
    href.push_str("./");
    for &b in target.as_bytes() {
        match b {
            b' ' => href.push('_'),
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => href.push(b as char),
            b'-' | b'.' | b'_' | b'~' | b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*'
            | b'+' | b',' | b';' | b'=' | b':' | b'@' | b'/' => href.push(b as char),
            _ => href.push_str(&format!("%{b:02X}")),
        }
    }
    href
}

/// Whitespace-collapse, merge same-style neighbors, trim the sequence edges,
/// drop empties. Both sides call this — it IS the declared inline normalization.
pub fn normalize_inlines(inlines: Vec<NfInline>) -> Vec<NfInline> {
    // 1. collapse whitespace runs inside every text
    let mut flat: Vec<NfInline> = inlines
        .into_iter()
        .map(|i| match i {
            NfInline::Run { text, bold, italic } => NfInline::Run { text: collapse_ws(&text), bold, italic },
            NfInline::Link { href, label } => NfInline::Link { href, label: normalize_inlines(label) },
        })
        .collect();
    // 2. merge adjacent same-style runs
    let mut merged: Vec<NfInline> = Vec::with_capacity(flat.len());
    for i in flat.drain(..) {
        match (merged.last_mut(), &i) {
            (
                Some(NfInline::Run { text: t0, bold: b0, italic: i0 }),
                NfInline::Run { text, bold, italic },
            ) if b0 == bold && i0 == italic => t0.push_str(text),
            _ => merged.push(i),
        }
    }
    // 3. trim sequence edges + drop empty runs
    if let Some(NfInline::Run { text, .. }) = merged.first_mut() {
        *text = text.trim_start().to_string();
    }
    if let Some(NfInline::Run { text, .. }) = merged.last_mut() {
        *text = text.trim_end().to_string();
    }
    merged.retain(|i| !matches!(i, NfInline::Run { text, .. } if text.is_empty()));
    merged
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !in_ws {
                out.push(' ');
            }
            in_ws = true;
        } else {
            out.push(ch);
            in_ws = false;
        }
    }
    out
}

/// AST → NF: the declared intent. Shares ONLY `md_href` + entity decoding with
/// the renderer (both are contract entries); everything the harness must judge
/// independently — escaping, indentation, fences, level arithmetic — is not here.
pub fn from_ast(nodes: &[Node]) -> Vec<NfBlock> {
    let mut out = Vec::new();
    for node in nodes {
        match node {
            Node::Heading { level, content } => {
                out.push(NfBlock::Heading((*level).clamp(1, 6), inline_nf(content, false, false)));
            }
            Node::Paragraph(children) => {
                let inl = inline_nf(children, false, false);
                if !inl.is_empty() {
                    out.push(NfBlock::Para(inl));
                }
            }
            Node::List { ordered, items } => out.push(list_nf(*ordered, items)),
            Node::Preformatted(lines) => {
                let text = lines.iter().map(|l| plain_text(l)).collect::<Vec<_>>().join("\n");
                out.push(NfBlock::Code { info: String::new(), text: text.trim_end().to_string() });
            }
            Node::Unsupported(s) => out.push(NfBlock::Code {
                info: "wikitext".to_string(),
                text: s.trim_end().to_string(),
            }),
            Node::Table { rows } => {
                if !rows.is_empty() {
                    out.push(NfBlock::Table {
                        rows: rows
                            .iter()
                            .map(|r| r.iter().map(|c| inline_nf(c, false, false)).collect())
                            .collect(),
                    });
                }
            }
            // stray top-level inline (parser wraps prose in Paragraph; defensive)
            other => {
                let inl = inline_nf(std::slice::from_ref(other), false, false);
                if !inl.is_empty() {
                    out.push(NfBlock::Para(inl));
                }
            }
        }
    }
    out
}

fn list_nf(ordered: bool, items: &[Vec<Node>]) -> NfBlock {
    let items = items
        .iter()
        .map(|item| {
            let mut content = Vec::new();
            let mut sublists = Vec::new();
            for n in item {
                if let Node::List { ordered, items } = n {
                    sublists.push(list_nf(*ordered, items));
                } else {
                    content.extend(inline_nf(std::slice::from_ref(n), false, false));
                }
            }
            NfItem { content: normalize_inlines(content), sublists }
        })
        .collect();
    NfBlock::List { ordered, items }
}

fn inline_nf(nodes: &[Node], bold: bool, italic: bool) -> Vec<NfInline> {
    let mut out = Vec::new();
    walk_inline(nodes, bold, italic, &mut out);
    normalize_inlines(out)
}

fn walk_inline(nodes: &[Node], bold: bool, italic: bool, out: &mut Vec<NfInline>) {
    for node in nodes {
        match node {
            Node::Text(s) => out.push(NfInline::Run {
                text: crate::entities::decode(s).into_owned(),
                bold,
                italic,
            }),
            Node::Bold(children) => walk_inline(children, true, italic, out),
            Node::Italic(children) => walk_inline(children, bold, true, out),
            Node::Link { target, label } => {
                let href = md_href(target);
                let label_nf = if label.is_empty() {
                    vec![NfInline::Run { text: target.to_string(), bold: false, italic: false }]
                } else {
                    inline_nf(label, false, false)
                };
                out.push(NfInline::Link { href, label: label_nf });
            }
            // block nodes inside inline position: flatten to text (defensive)
            other => out.push(NfInline::Run { text: plain_text(std::slice::from_ref(other)), bold, italic }),
        }
    }
}

fn plain_text(nodes: &[Node]) -> String {
    let mut s = String::new();
    collect_text(nodes, &mut s);
    s
}

fn collect_text(nodes: &[Node], out: &mut String) {
    for n in nodes {
        match n {
            Node::Text(s) => out.push_str(&crate::entities::decode(s)),
            Node::Bold(c) | Node::Italic(c) => collect_text(c, out),
            Node::Link { label, target } => {
                if label.is_empty() {
                    out.push_str(target);
                } else {
                    collect_text(label, out);
                }
            }
            Node::Heading { content, .. } => collect_text(content, out),
            Node::Paragraph(c) => collect_text(c, out),
            Node::List { items, .. } => items.iter().for_each(|i| collect_text(i, out)),
            Node::Preformatted(lines) => lines.iter().for_each(|l| collect_text(l, out)),
            Node::Table { rows } => rows.iter().flatten().for_each(|c| collect_text(c, out)),
            Node::Unsupported(s) => out.push_str(s),
        }
    }
}
```

- [ ] **Step 3:** `src/lib.rs` 注册（挨着 diff/output 的 doc(hidden) 段）：

```rust
#[doc(hidden)]
pub mod mdnorm;
```

- [ ] **Step 4:** `cargo test --lib` 编译通过（mdnorm 自身此刻无测试；它的判据是 harness）
- [ ] **Step 5:** commit（`md nf 123MB/s`）

### M1.2 渲染器桩

- [ ] **Step 1:** 建 `src/render/markdown.rs`：

```rust
//! AST → GFM markdown (Stage 3 M-line). Correctness contract: the round-trip
//! harness (tests/markdown_roundtrip.rs) — pulldown-cmark must parse this
//! module's output back to the same normal form mdnorm::from_ast declares.
//! M1 stub: renders nothing, so the harness starts RED by construction.

use crate::ast::Node;

/// Render nodes to GFM markdown.
pub fn markdown(nodes: &[Node]) -> String {
    let _ = nodes;
    String::new()
}
```

- [ ] **Step 2:** `src/render.rs`：`mod markdown;` + `pub use markdown::markdown;`（模块声明挨着 `mod html` 曾在的位置——现在只有 plain，放文件头）
- [ ] **Step 3:** `cargo test --lib` 过；commit（`md stub 123MB/s`）

### M1.3 pulldown 侧 NF（tests/support，fuzz 共享）

- [ ] **Step 1:** 建 `tests/support/pulldown_nf.rs`：

```rust
//! pulldown-cmark event stream → mdnorm NF. Shared by the round-trip
//! integration test and the fuzz target via `#[path]` include (files under
//! tests/support/ are not compiled as test targets on their own).

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
    // inline accumulation stack: (bold, italic) context + current runs
    inline: Vec<NfInline>,
    bold: u32,
    italic: u32,
    link: Option<(String, Vec<NfInline>)>, // (href, saved-outer-inline)
    // block context stacks
    lists: Vec<(bool, Vec<NfItem>)>,        // (ordered, items)
    item_content: Vec<(Vec<NfInline>, Vec<NfBlock>)>, // per open item
    heading: Option<u8>,
    code: Option<(String, String)>, // (info, text)
    table: Option<Vec<Vec<Vec<NfInline>>>>,
    row: Option<Vec<Vec<NfInline>>>,
    in_cell: bool,
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

    fn close_block(&mut self, b: NfBlock) {
        // route to the innermost open container: item > top level
        if let Some((_, subs)) = self.item_content.last_mut() {
            match b {
                NfBlock::Para(inl) => {
                    // tight-list paragraph unwrap: paragraph inside an item is content
                    self.item_content.last_mut().unwrap().0.extend(inl);
                }
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
            // inline code back from our output = an escaping bug; make it loud
            Event::Code(t) => panic!("unexpected inline code from our markdown: {t:?}"),
            other => panic!("unexpected event from our markdown: {other:?}"),
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => {}
            Tag::Heading { level, .. } => self.heading = Some(heading_num(level)),
            Tag::Strong => self.bold += 1,
            Tag::Emphasis => self.italic += 1,
            Tag::Link { dest_url, .. } => {
                let outer = std::mem::take(&mut self.inline);
                self.link = Some((dest_url.to_string(), outer));
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
            Tag::TableCell => self.in_cell = true,
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
                let l = self.heading.take().unwrap_or(heading_num(level));
                self.close_block(NfBlock::Heading(l, inl));
            }
            TagEnd::Strong => self.bold -= 1,
            TagEnd::Emphasis => self.italic -= 1,
            TagEnd::Link => {
                let label = self.take_inline();
                let (href, outer) = self.link.take().expect("link end without start");
                self.inline = outer;
                self.inline.push(NfInline::Link { href, label });
            }
            TagEnd::Item => {
                let leftover = self.take_inline(); // tight-list item text (no Para wrap)
                let (mut content, subs) = self.item_content.pop().expect("item end");
                content.extend(leftover);
                let (_, items) = self.lists.last_mut().expect("item outside list");
                items.push(NfItem { content: normalize_inlines(content), sublists: subs });
            }
            TagEnd::List(_) => {
                let (ordered, items) = self.lists.pop().expect("list end");
                self.close_block(NfBlock::List { ordered, items });
            }
            TagEnd::CodeBlock => {
                let (info, text) = self.code.take().expect("code end");
                self.close_block(NfBlock::Code { info, text: text.trim_end().to_string() });
            }
            TagEnd::TableCell => {
                let cell = self.take_inline();
                self.row.as_mut().expect("cell outside row").push(cell);
                self.in_cell = false;
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
```

（pulldown 0.13 的 `Tag`/`TagEnd` 具体形状以编译器为准——本步允许按编译错误微调字段名，语义不变。）

- [ ] **Step 2:** 暂不单跑（无 target 引用它不编译）；随 M1.4 一起验证

### M1.4 harness 本体（对桩跑红）

- [ ] **Step 1:** 建 `tests/markdown_roundtrip.rs`：

```rust
//! Stage 3 M-line round-trip harness (M1). For every input AST:
//! mdnorm::from_ast(ast) must equal pulldown_nf(render::markdown(ast)).
//! The renderer's output is judged by an INDEPENDENT GFM implementation —
//! the renderer cannot grade its own homework. Contract table:
//! docs/superpowers/plans/2026-07-02-markdown-roundtrip.md §0.

#[path = "support/pulldown_nf.rs"]
mod pulldown_nf;

use std::borrow::Cow;
use std::path::Path;
use wikrs::ast::Node;
use wikrs::{mdnorm, parser, render};

fn check(label: &str, nodes: &[Node]) -> Result<(), String> {
    let intent = mdnorm::from_ast(nodes);
    let md = render::markdown(nodes);
    let actual = std::panic::catch_unwind(|| pulldown_nf::markdown_to_nf(&md))
        .map_err(|_| format!("[{label}] pulldown_nf panicked on our markdown:\n{md}"))?;
    if intent == actual {
        Ok(())
    } else {
        Err(format!(
            "[{label}] round-trip mismatch\n--- markdown ---\n{md}\n--- intent ---\n{intent:#?}\n--- actual ---\n{actual:#?}"
        ))
    }
}

fn check_wikitext(label: &str, wt: &str) -> Result<(), String> {
    check(label, &parser::parse(wt).nodes)
}

fn text(s: &str) -> Node<'static> {
    Node::Text(Cow::Owned(s.to_string()))
}

#[test]
fn hand_built_cases_roundtrip() {
    let cases: Vec<(&str, Vec<Node>)> = vec![
        ("para", vec![Node::Paragraph(vec![text("plain prose")])]),
        (
            "escaping bomb",
            vec![Node::Paragraph(vec![text("a*b _c_ [d] <e> `f` 1. g # h & i | j \\k")])],
        ),
        (
            "heading level",
            vec![Node::Heading { level: 2, content: vec![text("History")] }],
        ),
        (
            "bold italic nesting",
            vec![Node::Paragraph(vec![
                Node::Bold(vec![text("two")]),
                text(" moons "),
                Node::Italic(vec![Node::Bold(vec![text("Alpha")])]),
            ])],
        ),
        (
            "links",
            vec![Node::Paragraph(vec![
                Node::Link { target: Cow::Borrowed("terrestrial planet"), label: vec![text("planet")] },
                text(" and "),
                Node::Link { target: Cow::Borrowed("https://e.org/a?b=1&c=(2)"), label: vec![] },
            ])],
        ),
        (
            "nested list",
            vec![Node::List {
                ordered: false,
                items: vec![
                    vec![text("a"), Node::List { ordered: true, items: vec![vec![text("b")]] }],
                    vec![text("c")],
                ],
            }],
        ),
        (
            "pre",
            vec![Node::Preformatted(vec![vec![text("line<1")], vec![text("line2")]])],
        ),
        (
            "unsupported fence",
            vec![Node::Unsupported(Cow::Borrowed("{{Infobox|x=```\ny}}"))],
        ),
        (
            "table",
            vec![Node::Table {
                rows: vec![
                    vec![vec![text("Property")], vec![text("Va|ue")]],
                    vec![vec![text("Radius")], vec![text("6,051 km")]],
                ],
            }],
        ),
    ];
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|(label, nodes)| check(label, nodes).err())
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} hand cases failed:\n\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n\n")
    );
}

#[test]
fn sample_article_roundtrips() {
    let wt = std::fs::read_to_string("tests/fixtures/sample_article.wikitext").unwrap();
    check_wikitext("sample_article", &wt).map_err(|e| println!("{e}")).unwrap();
}

/// Every parserTests wikitext (GPL fixture, fetched at test time; soft-skip
/// when missing — same policy as tests/parser_tests.rs).
#[test]
fn parser_tests_corpus_roundtrips() {
    const FIXTURE: &str = "tests/fixtures/parserTests.txt";
    if !Path::new(FIXTURE).exists() {
        eprintln!("SKIP: {FIXTURE} missing — run `cargo xtask fetch-parser-tests`.");
        return;
    }
    let input = std::fs::read_to_string(FIXTURE).unwrap();
    // minimal extraction: every `!! wikitext` section body inside a test block
    let mut failures = Vec::new();
    let mut total = 0usize;
    for (i, wt) in extract_wikitext_sections(&input).iter().enumerate() {
        total += 1;
        if let Err(e) = check_wikitext(&format!("case #{i}"), wt) {
            failures.push(e);
        }
    }
    eprintln!("markdown round-trip over parserTests: {}/{total} ok", total - failures.len());
    assert!(
        failures.is_empty(),
        "{}/{} parserTests inputs failed round-trip; first 5:\n\n{}",
        failures.len(),
        total,
        failures.iter().take(5).cloned().collect::<Vec<_>>().join("\n\n")
    );
}

/// Tiny format reader: `!! test`…`!! end` blocks, `!! wikitext` section body.
/// (Full-fidelity parser lives in tests/parser_tests.rs; this harness only
/// needs the raw wikitext bodies.)
fn extract_wikitext_sections(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut in_test, mut in_wt) = (false, false);
    let mut cur = String::new();
    for line in input.lines() {
        let tag = line.strip_prefix("!!").map(str::trim);
        match tag {
            Some("test") => in_test = true,
            Some("end") => {
                if in_test && !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                in_test = false;
                in_wt = false;
            }
            Some("wikitext") if in_test => {
                in_wt = true;
                cur.clear();
            }
            Some(t)
                if in_test
                    && (t.starts_with("html")
                        || matches!(t, "options" | "metadata" | "config" | "wikitext/edited")) =>
            {
                in_wt = false;
            }
            _ if in_wt => {
                if !cur.is_empty() {
                    cur.push('\n');
                }
                cur.push_str(line);
            }
            _ => {}
        }
    }
    out
}
```

- [ ] **Step 2: 跑红。** `cargo test --test markdown_roundtrip`
  预期：`hand_built_cases_roundtrip` FAIL（桩输出空串，9 个手工用例除空输入外全 mismatch）；`sample_article_roundtrips` FAIL；corpus 有 fixture 则大面积 FAIL（无则 SKIP）。**把失败计数记进 WORKLOG（红的证据）。**
- [ ] **Step 3:** commit（`md harness red 123MB/s`——WORKLOG 注明这是刻意的红，M1 完成标志）

---

## Task M2: 渲染器实装（迭代到 harness 全绿）

### M2.1 块结构 + 行内 + 转义（一次写全，harness 驱动修）

- [ ] **Step 1:** 重写 `src/render/markdown.rs`：

```rust
//! AST → GFM markdown (Stage 3 M-line). Correctness contract: the round-trip
//! harness — pulldown-cmark must parse this output back to the same normal
//! form `mdnorm::from_ast` declares. Design choices that keep the round-trip
//! unambiguous: render inline content from mdnorm's normalized runs (kills
//! `***`-adjacency ambiguity at the source), `*`-family emphasis only (`_`
//! has intraword rules), tight lists, fenced code with adaptive fence length.

use crate::ast::Node;
use crate::mdnorm::{self, NfBlock, NfInline, NfItem};

/// Render nodes to GFM markdown.
pub fn markdown(nodes: &[Node]) -> String {
    let blocks = mdnorm::from_ast(nodes);
    let mut out = String::new();
    render_blocks(&blocks, 0, &mut out);
    out.trim_end().to_string()
}

fn render_blocks(blocks: &[NfBlock], indent: usize, out: &mut String) {
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 {
            out.push('\n'); // blank line between sibling blocks
        }
        render_block(b, indent, out);
    }
}

fn render_block(b: &NfBlock, indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    match b {
        NfBlock::Heading(level, inl) => {
            out.push_str(&pad);
            for _ in 0..*level {
                out.push('#');
            }
            out.push(' ');
            render_inlines(inl, false, out);
            out.push('\n');
        }
        NfBlock::Para(inl) => {
            out.push_str(&pad);
            render_inlines(inl, false, out);
            out.push('\n');
        }
        NfBlock::List { ordered, items } => {
            for item in items {
                render_item(*ordered, item, indent, out);
            }
        }
        NfBlock::Code { info, text } => {
            let fence_len = 3.max(longest_backtick_run(text) + 1);
            let fence: String = "`".repeat(fence_len);
            out.push_str(&pad);
            out.push_str(&fence);
            out.push_str(info);
            out.push('\n');
            for line in text.lines() {
                out.push_str(&pad);
                out.push_str(line);
                out.push('\n');
            }
            out.push_str(&pad);
            out.push_str(&fence);
            out.push('\n');
        }
        NfBlock::Table { rows } => {
            let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
            for (ri, row) in rows.iter().enumerate() {
                out.push_str(&pad);
                out.push('|');
                for ci in 0..cols {
                    out.push(' ');
                    if let Some(cell) = row.get(ci) {
                        render_inlines(cell, true, out);
                    }
                    out.push_str(" |");
                }
                out.push('\n');
                if ri == 0 {
                    out.push_str(&pad);
                    out.push('|');
                    for _ in 0..cols {
                        out.push_str(" --- |");
                    }
                    out.push('\n');
                }
            }
        }
    }
}

fn render_item(ordered: bool, item: &NfItem, indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    let marker = if ordered { "1. " } else { "- " };
    out.push_str(&pad);
    out.push_str(marker);
    render_inlines(&item.content, false, out);
    out.push('\n');
    for sub in &item.sublists {
        render_block(sub, indent + marker.len(), out);
    }
}

fn render_inlines(inlines: &[NfInline], in_table_cell: bool, out: &mut String) {
    let at_block_start = out.is_empty() || out.ends_with(' ') && is_line_start_after_marker(out);
    let mut first = true;
    for inl in inlines {
        match inl {
            NfInline::Run { text, bold, italic } => {
                if *bold {
                    out.push_str("**");
                }
                if *italic {
                    out.push('*');
                }
                push_escaped_text(text, first && at_block_start, in_table_cell, out);
                if *italic {
                    out.push('*');
                }
                if *bold {
                    out.push_str("**");
                }
            }
            NfInline::Link { href, label } => {
                out.push('[');
                render_inlines(label, in_table_cell, out);
                out.push_str("](");
                push_href(href, out);
                out.push(')');
            }
        }
        first = false;
    }
}

/// Are we at the start of a line's *content* (only list markers/indent behind)?
fn is_line_start_after_marker(out: &str) -> bool {
    let line = out.rsplit('\n').next().unwrap_or(out);
    line.chars().all(|c| c == ' ' || c == '-' || c == '.' || c.is_ascii_digit())
}

/// Escape so pulldown reads this back as literal text. Inline set always;
/// line-start hazards only when the run opens a block line (our runs contain
/// no newlines — whitespace was collapsed); `|` only inside table cells.
fn push_escaped_text(text: &str, at_line_start: bool, in_table_cell: bool, out: &mut String) {
    for (i, ch) in text.chars().enumerate() {
        let line_start_hazard = at_line_start
            && i == 0
            && matches!(ch, '#' | '>' | '-' | '+' | '=' | '~');
        match ch {
            '\\' | '*' | '_' | '[' | ']' | '`' => {
                out.push('\\');
                out.push(ch);
            }
            '<' => out.push_str("&lt;"),
            '&' => out.push_str("&amp;"),
            '|' if in_table_cell => out.push_str("\\|"),
            '.' | ')' if at_line_start && leading_digits(text, i) => {
                out.push('\\');
                out.push(ch);
            }
            _ if line_start_hazard => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
}

/// `text[..i]` is all digits and nonempty (ordered-list lookalike `12. x`).
fn leading_digits(text: &str, i: usize) -> bool {
    i > 0 && text[..i].bytes().all(|b| b.is_ascii_digit())
}

/// Link destination: angle-wrap when it contains characters that break the
/// plain `(dest)` form; escape closing angle inside.
fn push_href(href: &str, out: &mut String) {
    if href.contains([' ', '(', ')', '<', '>']) {
        out.push('<');
        for ch in href.chars() {
            match ch {
                '<' => out.push_str("%3C"),
                '>' => out.push_str("%3E"),
                _ => out.push(ch),
            }
        }
        out.push('>');
    } else {
        out.push_str(href);
    }
}

fn longest_backtick_run(text: &str) -> usize {
    let mut max = 0;
    let mut cur = 0;
    for ch in text.chars() {
        if ch == '`' {
            cur += 1;
            max = max.max(cur);
        } else {
            cur = 0;
        }
    }
    max
}
```

- [ ] **Step 2: 跑 harness。** `cargo test --test markdown_roundtrip 2>&1 | tail -20`
  预期：手工用例大部分绿；**必有残余红**（转义/缩进/表格总有始料未及的），逐个读 mismatch 的三段输出（markdown/intent/actual）修渲染器或（若是声明歧义）修 §0 契约表+mdnorm，**改契约必须在 WORKLOG 记一条为什么**
- [ ] **Step 3:** corpus 有 fixture 时全量过（`cargo xtask fetch-parser-tests` 先跑）；迭代到 `markdown_roundtrip` 三个测试全绿
- [ ] **Step 4:** 全量 CI 三件套 + `scripts/bench.sh`（渲染器是新增面，criterion 应噪声内）
- [ ] **Step 5:** commit（`md render 123MB/s`，WORKLOG 记：红→绿迭代中修掉的坑清单）

---

## Task M3: fuzz 往返性质

- [ ] **Step 1:** `fuzz/Cargo.toml`：`[dependencies]` 加 `pulldown-cmark = "0.13"`（与 dev-dep 同版）+ 新 `[[bin]] name = "markdown_roundtrip"`（test/doc/bench = false，同现有三个）
- [ ] **Step 2:** 建 `fuzz/fuzz_targets/markdown_roundtrip.rs`：

```rust
#![no_main]

use libfuzzer_sys::fuzz_target;

#[path = "../../tests/support/pulldown_nf.rs"]
mod pulldown_nf;

// Stage 3 M-line safety+correctness target: for ANY input, parse → render
// markdown → an independent GFM parser must read back exactly the declared
// normal form. Catches escaping leaks the hand corpus missed.
// Run: cargo +nightly fuzz run markdown_roundtrip
fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let parsed = wikrs::parser::parse(s);
        let intent = wikrs::mdnorm::from_ast(&parsed.nodes);
        let md = wikrs::render::markdown(&parsed.nodes);
        let actual = pulldown_nf::markdown_to_nf(&md);
        assert_eq!(intent, actual, "round-trip mismatch\n--- md ---\n{md}");
    }
});
```

- [ ] **Step 3:** 种子：`mkdir -p fuzz/corpus/markdown_roundtrip && find fuzz/corpus/parse -type f -print0 | xargs -0 -J % cp % fuzz/corpus/markdown_roundtrip/`
- [ ] **Step 4:** 冒烟 ≥120s：`cargo +nightly fuzz run markdown_roundtrip -- -max_total_time=120`
  预期：零 crash/零断言失败；有失败则回 M2.2 循环修（fuzz 抓到的每个都是手工语料漏掉的真 bug，WORKLOG 点名）
- [ ] **Step 5:** commit（`md fuzz 123MB/s`；WORKLOG 记执行数）。发布前义务：25 分钟长跑

---

## Task M4: CLI + 快照 + 全量验收

### M4.1 output::to_markdown + CLI

- [ ] **Step 1（先红）:** `tests/cli.rs` 追加：

```rust
#[test]
fn markdown_format_renders_structured_pages() {
    let xml = "<mediawiki><page><title>A*B</title><ns>0</ns>\
        <revision><text>'''Earth''' is a [[Planet|planet]].\n\n== History ==\n\nOld.</text>\
        </revision></page></mediawiki>";
    let out = run(&["--format", "markdown"], xml, "md.xml");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.contains("# A\\*B"), "escaped title as h1: {s}");
    assert!(s.contains("**Earth** is a [planet](./Planet)."), "body: {s}");
    assert!(s.contains("## History"), "section heading: {s}");
}

#[test]
fn markdown_format_rejects_strip_engine_and_stats() {
    let xml = "<mediawiki><page><title>T</title><ns>0</ns>\
        <revision><text>body</text></revision></page></mediawiki>";
    let out = run(&["--format", "markdown", "--engine", "strip"], xml, "m1.xml");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("ast"));
    let out = run(&["--format", "markdown", "--stats"], xml, "m2.xml");
    assert!(!out.status.success());
}
```

`src/output.rs` 单测追加：

```rust
#[test]
fn markdown_page_has_escaped_h1_then_body() {
    assert_eq!(
        to_markdown("A*B", "**Earth** is here."),
        "# A\\*B\n\n**Earth** is here.\n"
    );
}
```

- [ ] **Step 2:** 跑红（`cargo test --test cli markdown_format`：unexpected value 'markdown'）
- [ ] **Step 3:** 实装。`src/output.rs`：

```rust
/// One markdown document per page: escaped `# title`, blank line, body.
/// Title escaping reuses the renderer's own rules via a single-Text render —
/// one escaping path, no drift.
pub fn to_markdown(title: &str, body: &str) -> String {
    use std::borrow::Cow;
    let title_md = crate::render::markdown(&[crate::ast::Node::Paragraph(vec![
        crate::ast::Node::Text(Cow::Borrowed(title)),
    ])]);
    let mut out = String::with_capacity(title_md.len() + body.len() + 8);
    out.push_str("# ");
    out.push_str(&title_md);
    out.push('\n');
    if !body.is_empty() {
        out.push('\n');
        out.push_str(body);
    }
    out.push('\n');
    out
}
```

`src/main.rs`：`Format` 加 `Markdown` 变体（doc 注释注明需 ast 引擎）；两条 bail 复制 sections 的模式（`--format markdown` + strip / + stats）；map 匹配臂：

```rust
(Format::Markdown, _) => {
    let parsed = parser::parse(&p.text);
    output::to_markdown(&p.title, &render::markdown(&parsed.nodes))
}
```

writer 臂：`Format::Markdown` 归入 `Format::Text | Format::Sections` 直写分支。
- [ ] **Step 4:** 跑绿 + 全量三件套
- [ ] **Step 5:** commit（`cli md 123MB/s`）

### M4.2 快照 + 全量 + 文档

- [ ] **Step 1:** 建 `tests/markdown_snapshots.rs`（复用 parse_snapshots 的代表性 wikitext，`insta::assert_snapshot!(render::markdown(...))`）；`cargo insta` 首跑生成，人审后 mv .snap.new → .snap
- [ ] **Step 2:** 全量 simplewiki：`/usr/bin/time ./target/release/wikrs --input target/realdump/simplewiki-articles.xml --format markdown > /dev/null`，抽查头两页人眼看结构；耗时进 WORKLOG
- [ ] **Step 3:** 文档四件套：README（Usage 格式清单 + 状态行 + Roadmap 行 + 日期）、CHANGELOG `[Unreleased]` 新增 markdown 条目、`stages/stage-3-llm-output.md` M1–M4 勾选 + 状态行、WORKLOG 收尾条
- [ ] **Step 4:** 全量三件套 + bench + commit（`md done 123MB/s`）

**发布（不在本计划内）：** M4 全绿后按 0.2.0 的机械流程出 0.3.0（bump→dry-run→push→tag→publish→Release），等用户指令。

---

## Self-review 记录

- 规格覆盖：M1=harness 红（含语料三层）✓ M2=绿 ✓ M3=fuzz ✓ M4=CLI/快照/全量/文档 ✓；§0 回答"准确性怎么比"✓
- 占位符扫描：无 TBD/"适当处理"；pulldown API 字段名标注了"以编译器为准"的许可（0.13 的 Tag 结构有版本差异，语义固定）
- 类型一致性：`NfBlock/NfInline/NfItem/md_href/normalize_inlines/from_ast` 各任务间签名一致 ✓；`render::markdown` 消费 `mdnorm` 的 NF（M2 依赖 M1.1）✓
