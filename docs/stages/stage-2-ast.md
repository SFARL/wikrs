# Stage 2 — 进阶档：结构化 AST 引擎

**状态:** 🛠 进行中 · **日期:** 2026-06-25 · **定位:** 真正的声誉项目

> 产出结构化 AST，保留链接/结构，对处理不了的输入**报警而非静默丢弃**。
> 这是 D2（诚实划界）的主战场。设计见 [../DESIGN.md](../DESIGN.md) §3、§7；测试见 [../TESTING.md](../TESTING.md)。

---

## 当前进度（实测，2026-06-25）

thin vertical slice 已跑通：`wikitext → tokenizer → parser → AST → render::plain`，诊断系统就位。

- **支持子集（零诊断 = 完全支持）**：段落、标题（`==`）、粗体（`'''`）、斜体（`''`）、内链（`[[…]]`）、外链（`[url …]`）、flat 列表（`*`/`#`）。
- **范围外 → `Unsupported` + Diagnostic（保留原文 span，不假装）**：模板 `{{`（U-TEMPLATE）、表格 `{|`/`|`/`!`（U-TABLE）、HTML/ref 标签（U-HTML）、嵌套/定义列表（U-LIST）、预格式（U-PRE）。
- **parserTests 覆盖率**：**27.1%（292/1077）** 零诊断完全支持 —— 这是 Stage 2 的核心追踪数，随子集扩大往上爬（见 README 记分牌 + `cargo test --test parser_tests stage2_coverage_rate`）。

**实际模块**（都是单文件，起步从简）：`src/ast.rs`、`src/tokenizer.rs`、`src/parser.rs`、`src/diag.rs`、`src/render.rs`。

---

## Checkpoint（Definition of Done）

- [~] **C1** AST 类型：`Node` 覆盖段落/标题/粗斜体/链接/列表/Unsupported；**待**：表格、模板占位、per-node span（目前 span 在 Diagnostic 上）。
- [~] **C2** tokenizer + parser 单趟线性；**待**：对 parser 专门 fuzz（Stage 1 robustness 已覆盖 strip）。
- [~] **C3** 诊断系统落地：`Unsupported` + 原文 span ✓；**待**：CLI 汇总 parser 路线的 X/Y/Z（`--stats` 目前还是 strip 路线）。
- [~] **C4** parserTests：零诊断**覆盖率** 27.1% ✓ 可追踪；**待**：HTML conformance 通过率 + 自动生成 `SUPPORTED.md`。
- [ ] **C5** 差分测试 vs Parsoid/REST（X/Y/Z 三个数字）。
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
- **文件**：`src/parser.rs`。块级（空行分段 + 标题 + flat 列表）+ inline 组装（粗斜体/内外链配对，未闭合降级为文本）。模板/表格/嵌套列表/HTML → `Unsupported` + Diagnostic。
- **持续**：每加一类构造，coverage↑（外链 → 列表 → 已做；下一步 `<ref>`/nowiki/表格…）。
- **已知限制**：块切分靠空行——紧贴正文（无空行）的 `== 标题 ==` 暂识别不到（真实 wikitext 常见），待修；这也压低了真实文章上的 coverage。

### Task 4：诊断系统 ✅
- **文件**：`src/diag.rs`。`Diagnostic{severity, code, span, message}` + `Severity{Error/Warning/Unsupported}` + 稳定码（U-TEMPLATE/U-TABLE/U-HTML/U-LIST/U-PRE）。

### Task 5：渲染器 🛠
- **文件**：`src/render.rs`。`render::plain(&[Node])` ✅。**待**：struct-JSONL、html（Stage 3）；接进 CLI 与 strip 对比（Stage 2 步骤 3）。

### Task 6：parserTests 跑测框架 ✅（覆盖率）
- **文件**：`tests/parser_tests.rs`。格式解析器 + `stage2_coverage_rate`（零诊断百分比，floor 防回退）。
- **待**：`xtask supported` 生成 `SUPPORTED.md`；逐例 HTML conformance（需 render::html）。

### Task 7：差分测试 + 三个数字（声誉证据） ⏳ 未开工
- **文件**：`tests/diff/`、`xtask diff-report`。N 万 ns0 页面 vs Parsoid/REST，归一化 DOM diff，出 X/Y/Z。

---

## 风险 / 提醒

- **这是项目会淹死人的地方**。守住 D2：范围外就报警，**绝不为了好看而假装解析对**——一旦硬凑就是滑向"复刻 MediaWiki bug"的死结。
- coverage 阈值要现实：诚实的支持范围 + 把"过不了的"光明正大列清楚，这正是叙事强项。
- 性能不能因为上结构而崩——AST 仍 borrow 优先（`Cow`）、流式友好。
