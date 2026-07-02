> *Internal dev-history document (Chinese). For English, see [DESIGN.md](../DESIGN.md) and [TESTING.md](../TESTING.md).*

# Stage 2 — 进阶档：结构化 AST 引擎

**状态:** 🛠 进行中（核心管线 + 规模验证完成，coverage 缓爬）· **日期:** 2026-06-27，快照更新 2026-07-01 · **定位:** 真正的声誉项目

> **2026-07-01 里程碑快照:** 差分（120 随机页）word-precision **99.3% / 0% 静默 / 115-of-120 完全忠实**；**全量 enwiki 验证 7,189,653 页 98.0% clean、7.4 分钟**（`--index` 并行 multistream 解码，5.1×）、零崩溃；CLI 有界流式（O(batch) 内存）+ dump 错误硬失败；dump XML 实体修复（quick-xml GeneralRef 静默丢失）；parse 全路径 fuzz target（首小时抓到并修掉 UTF-8 切片 panic，26M 次执行零 crash）。逐条见 [WORKLOG.md](../../WORKLOG.md)。

> 产出结构化 AST，保留链接/结构，对处理不了的输入**报警而非静默丢弃**。
> 这是 D2（诚实划界）的主战场。设计见 [../DESIGN.md](../DESIGN.md) §3、§7；测试见 [../TESTING.md](../TESTING.md)。

---

## 当前进度（实测，2026-06-27）

完整管线已跑通：`wikitext → blocks() → 逐块 dispatch → AST + 诊断 → render::plain`，**已是 CLI 默认引擎**。

- **支持子集（零诊断 = 完全支持）**：段落、标题（`==`）、粗/斜体、内链、外链、flat/嵌套/定义列表、预格式、简单表格、`<ref>`/nowiki/注释、行内 HTML 格式标签、表现型 HTML 容器（`<div>`/`<center>`/`<blockquote>`/`<p>`）、显示型转写标签（`<noinclude>`/`<onlyinclude>`）、HTML 列表（`<ul>`/`<ol>`/`<li>`）。
- **模板 `{{…}}` → `W-TEMPLATE`（Warning，丢弃留正文，非 Unsupported）**：刻意不展开（D4 护城河），所以**不计入"完全支持"**。
- **范围外 → `Unsupported` + Diagnostic（保留原文 span，不假装）**：HTML 表格 / test 扩展标签 / `<includeonly>`（U-HTML）、wikitext 表格 `{|`（U-TABLE）、不规则列表嵌套（U-LIST）、预格式边角（U-PRE）。
- **parserTests 覆盖率**：**49.1%（529/1077）** 零诊断完全支持 —— Stage 2 核心追踪数（见 README 记分牌 + `cargo test --test parser_tests stage2_coverage_rate`）。直方图（2026-06-27 快照）：`W-TEMPLATE 391 / U-HTML 139 / U-TABLE 56 / U-LIST 51 / U-PRE 14`。
- **诚实天花板**：~49% 是**不展开模板**前提下的干净 chipping 上限——剩下的 W-TEMPLATE 是刻意丢弃，U-HTML/U-TABLE/U-LIST/U-PRE 多是 HTML 表格 + 误报 + 模板 fostering 边角，不是干净可拿的。**所以覆盖率不再是该追的指标**，重心已转向差分"三个数字"（见 Task 7）。

**实际模块**：`src/ast.rs`、`src/tokenizer.rs`、`src/parser.rs`、`src/diag.rs`、`src/render.rs`、`src/diff.rs`（层 2 差分核心）。

---

## Checkpoint（Definition of Done）

- [~] **C1** AST 类型：`Node` 覆盖段落/标题/粗斜体/链接/列表/Unsupported；**待**：表格、模板占位、per-node span（目前 span 在 Diagnostic 上）。
- [~] **C2** tokenizer + parser 单趟线性；**待**：对 parser 专门 fuzz（Stage 1 robustness 已覆盖 strip）。
- [~] **C3** 诊断系统落地：`Unsupported` + 原文 span ✓；差分报告已出页级桶 + precision/coverage（见 C5）；**待**：CLI `--stats` 接 parser 路线（目前还是 strip 路线）。
- [~] **C4** parserTests：零诊断**覆盖率** 49.0% ✓ 可追踪 + 反向兼容 ratchet ✓；**待**：HTML conformance 通过率 + 自动生成 `SUPPORTED.md`。
- [x] **C5** 差分测试 vs Parsoid/REST ✅（2026-06-27）：`xtask diff-fetch`/`diff-report` + `src/diff.rs`。**precision-led**——页级三桶在真实页面坍缩成 0/0/100，头条改用 fidelity overlay（种子样本 18 篇：precision ~91% / coverage ~49% / 13-of-18 faithful / 0 离群）。**待**：扩样到 N 万随机 ns0、truth 归一化再清一档。
- [~] **C6** 四层测试：单元/快照/coverage ✓；差分待，parser fuzz 待。
- [~] **C7** `render::plain` 存在；**待**：接进 CLI、与 `extract::strip` 对比并切换。

---

## Tasks

### Task 1：AST 类型设计 ✅（部分）
- **文件**：`src/ast.rs`（单文件）。`Node<'a>` 用 `Cow<'a,str>` borrow-friendly；`Unsupported` 占位有了。
- **待**：表格 / 模板占位变体；是否给每个 Node 加 span。

### Task 2：tokenizer ✅
- **文件**：`src/tokenizer.rs`。手写 inline 分词器（Text/Bold/Italic/Link/ExtLink/Pipe），单趟线性、UTF-8 安全。

### Task 3：parser → AST ✅（子集，持续扩）
- **文件**：`src/parser.rs`。块级（**空行 + 标题行**分段 + 列表/预格式/表格 dispatch）+ inline 组装（粗斜体/内外链配对，未闭合降级为文本）。范围外构造 → `Unsupported` + Diagnostic；模板 → `W-TEMPLATE` 丢弃。
- **已做**：外链、flat/嵌套/定义列表、`<ref>`/nowiki/注释、行内 + 容器 HTML、转写标签、HTML 列表——把 coverage 从 ~27% 推到 49.0%。
- **已知限制（已修）**：块切分现按**空行 + 标题行**——紧贴正文无空行的 `== 标题 ==` 现在能识别。

### Task 4：诊断系统 ✅
- **文件**：`src/diag.rs`。`Diagnostic{severity, code, span, message}` + `Severity{Error/Warning/Unsupported}` + 稳定码（U-TEMPLATE/U-TABLE/U-HTML/U-LIST/U-PRE）。

### Task 5：渲染器 🛠
- **文件**：`src/render.rs`。`render::plain(&[Node])` ✅。**待**：struct-JSONL、html（Stage 3）；接进 CLI 与 strip 对比（Stage 2 步骤 3）。

### Task 6：parserTests 跑测框架 ✅（覆盖率）
- **文件**：`tests/parser_tests.rs`。格式解析器 + `stage2_coverage_rate`（零诊断百分比，floor 防回退）。
- **待**：`xtask supported` 生成 `SUPPORTED.md`；逐例 HTML conformance（需 render::html）。

### Task 7：差分测试 + 三个数字（声誉证据） ✅（2026-06-27，precision-led）
- **文件**：`src/diff.rs`（`wikrs::diff`：shingle 归一化 + precision/coverage + classify 三桶 + Report，零依赖、7 单测）、`xtask diff-fetch`（curl wikitext + Parsoid HTML，`scraper` 抽正文，缓存 gitignore）、`xtask diff-report`（离线，逐页 parse→render→classify，出三个数字 + fidelity overlay）、`tests/diff_report.rs`（离线集成 smoke）、`tests/diff/titles.txt`（仅名字、入库、可复现）。
- **关键发现**：比的是**文本级**（wikrs plain text vs Parsoid HTML 可见正文），不是 DOM——Stage 2 没 `render::html`（那是 Stage 3）。**precision-led**：wikrs 按设计丢模板，输出是 Parsoid 正文子集，所以测"wikrs 给的对不对"（precision），coverage 单独报。"Reported" 是页级的 → 真实 featured 文章必含越界构造 → 页级三桶坍缩 0/0/100（诚实但无信息量），真正头条是逐页 fidelity overlay。详见 README 记分牌。
- **待**：随机 ns0 扩样（pin 结果保可复现）；truth 归一化剥 `<math>`/`<ref>` 把保守的 ~91% precision 顶上去。

---

## 风险 / 提醒

- **这是项目会淹死人的地方**。守住 D2：范围外就报警，**绝不为了好看而假装解析对**——一旦硬凑就是滑向"复刻 MediaWiki bug"的死结。
- coverage 阈值要现实：诚实的支持范围 + 把"过不了的"光明正大列清楚，这正是叙事强项。
- 性能不能因为上结构而崩——AST 仍 borrow 优先（`Cow`）、流式友好。
