# Stage 2 — 进阶档：结构化 AST 引擎

**状态:** 未开工（依赖 Stage 1 发布 + 拿到反馈） · **日期:** 2026-06-23 · **定位:** 真正的声誉项目

> 产出结构化 AST，保留表格 / 链接锚文本 / 结构，对处理不了的怪异输入**报警而非静默丢弃**。
> 这是 D2（诚实划界）的主战场。设计见 [../DESIGN.md](../DESIGN.md) §3、§7；测试见 [../TESTING.md](../TESTING.md)。

---

## 目标

`wikitext → tokenizer → parser → AST`，AST 上挂多个渲染器（plain / struct-JSONL / 后续 html）。在**声明的支持范围内**结构正确，范围外产出 `Diagnostic` 报警。`render::plain` 最终取代 Stage 1 的 `extract::strip`。

---

## Checkpoint（Definition of Done）

- [ ] **C1** AST 类型定义稳定：`Node` 枚举覆盖段落/标题/链接/列表/表格/模板占位/格式标记，每个节点带原文 `span`。
- [ ] **C2** tokenizer + parser 在病态输入上**最坏复杂度线性**（fuzz 验证，2MB 输入不退化成平方）。
- [ ] **C3** **诊断系统落地**：范围外构造产出 `Unsupported` 诊断 + 保留原文 span，不静默丢；CLI 汇总 `X/Y/Z` 三个数字。
- [ ] **C4** parserTests 通过率达到目标阈值（阈值在开工时定，记入本文件）；自动生成 `SUPPORTED.md`。
- [ ] **C5** 差分测试出报告：N 万真实页面 vs Parsoid/REST ground truth，归一化 DOM diff，得到"X% 完全一致 / Y% 结构差异 / Z% 主动报错"。
- [ ] **C6** 四层测试全到位（parserTests / 差分 / fuzz / 快照）。
- [ ] **C7** `render::plain` 输出质量 ≥ Stage 1 `extract::strip`，且能切过去。

---

## Tasks（roadmap 层，开工时再细化为 TDD 计划）

### Task 1：AST 类型设计
- **文件**：`src/ast/`（`node.rs`、`span.rs`）
- **做什么**：定义 `Node` 枚举 + `Span`；优先 `&str` borrow 零拷贝；为"模板未展开占位"留专门变体。
- **验证**：能手构 AST 并 round-trip 到 plain text；类型评审（可用 `type-design-analyzer` agent）。

### Task 2：tokenizer
- **文件**：`src/tokenizer/`、`tests/tokenizer.rs`、`fuzz/fuzz_targets/parse.rs`
- **做什么**：wikitext → token 流。先定手写 vs `logos`（DESIGN §5 待定项），以最坏复杂度线性为硬约束。
- **验证**：单元测试 + fuzz 验证线性、不崩溃。

### Task 3：parser → AST
- **文件**：`src/parser/`、`tests/parser.rs`
- **做什么**：token → AST。链接、列表、标题、格式标记、表格（在声明范围内）。**遇模板纠缠/越界结构 → 发 Diagnostic 降级，不假装解析对**。
- **验证**：单元 + 快照；parserTests 通过率开始爬升。

### Task 4：诊断系统
- **文件**：`src/diag/`
- **做什么**：实现 `Diagnostic{severity, code, span, message}` + 稳定错误码表；CLI 汇总统计。
- **验证**：构造越界输入，断言产出正确 `Unsupported` 码 + span，且后续内容继续处理。

### Task 5：渲染器
- **文件**：`src/render/`（`plain.rs`、`struct_jsonl.rs`）
- **做什么**：AST → plain text（取代 Stage 1）；AST → 结构化 JSONL（保留 table/link/结构）。
- **验证**：`render::plain` 在 Stage 1 的快照集上 ≥ `extract::strip`；JSONL schema 固定 + 快照。

### Task 6：parserTests 跑测框架
- **文件**：`tests/parser_tests.rs`、`tests/fixtures/parserTests.txt`、`xtask supported`
- **做什么**：解析 `parserTests.txt`、逐条比对、生成 `SUPPORTED.md`。
- **验证**：通过率可量化、可追踪；生成的支持清单合理。

### Task 7：差分测试 + 三个数字（声誉证据）
- **文件**：`tests/diff/`、`xtask diff-report`
- **做什么**：取 N 万 ns0 页面（固定 seed），取 Parsoid/REST ground truth，归一化 DOM diff，出 X/Y/Z 报告。
- **验证**：报告可复现；数字进 README 头条。

---

## 风险 / 提醒

- **这是项目会淹死人的地方**（6200 行多趟解析的深坑）。守住 D2：范围外就报警，**绝不为了好看而假装解析对**——一旦开始硬凑，就是滑向"复刻 MediaWiki bug"的死结。
- 通过率阈值要现实：定一个诚实的支持范围，把"过不了的"光明正大列进 `SUPPORTED.md`，这正是叙事强项。
- 性能不能因为上结构而崩——AST 仍要 borrow 优先、流式友好。
