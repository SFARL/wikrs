> *Internal dev-history document (Chinese). For English, see [DESIGN.md](../DESIGN.md) and [TESTING.md](../TESTING.md).*

# Stage 3 —（已否决）AST → HTML 渲染

> **⚠️ 已被 [stage-3-llm-output.md](stage-3-llm-output.md) 取代（2026-07-02）。** 实际场景是 LLM 数据管线（sections JSONL + Markdown），HTML 无消费者。首次实现（4 提交）留档于 `feat/stage3-html` 分支。本文件保留作决策史。

**状态:** 已否决 · **日期:** 2026-06-23 · **定位:** 锦上添花，非主线

> 只有在 Stage 1/2 立住、且有真实需求拉动时才做。**不要在它身上重新掉进 byte-level 兼容的坑（D1）。**
> 设计见 [../DESIGN.md](../DESIGN.md)；测试见 [../TESTING.md](../TESTING.md)。

---

## 目标

在 Stage 2 的 AST 上挂一个 HTML 渲染器：`AST → render::html → HTML`。目的是让 wikrs 能直接产出可读 HTML（给预览、静态站、轻量镜像等用），而**不是**追求和 MediaWiki/Parsoid 的 HTML 字节一致。

---

## 触发条件（先满足再开工）

- [ ] Stage 2 的 AST 已稳定、差分报告已出。
- [ ] 有具体用户/场景在要 HTML 输出（不是"顺手做一下"）。
- [ ] 明确接受：HTML 在**声明支持范围内**正确即可，范围外沿用 Stage 2 的 `Unsupported` 诊断。

---

## Checkpoint（Definition of Done）

- [ ] **C1** `render::html` 覆盖 AST 全部节点类型，输出格式良好（well-formed）的 HTML 片段。
- [ ] **C2** parserTests 里 HTML 比对类用例，在声明范围内通过（归一化后比对，忽略无意义格式差异）。
- [ ] **C3** 快照测试锁住代表性页面的 HTML 输出。
- [ ] **C4** 范围外构造在 HTML 输出里有可见标注（如注释/占位），与诊断一致，不静默丢。
- [ ] **C5** XSS/注入安全：用户内容正确转义，fuzz 不产出畸形/危险 HTML。

---

## Tasks（roadmap 层）

### Task 1：HTML 渲染器骨架
- **文件**：`src/render/html.rs`
- **做什么**：遍历 AST 输出 HTML；正确转义文本；段落/标题/列表/链接/格式标记。
- **验证**：基本节点的 `insta` 快照；输出可被 HTML 解析器接受。

### Task 2：表格与复杂结构
- **文件**：`src/render/html.rs`
- **做什么**：表格、嵌套列表、引用等结构映射到 HTML。
- **验证**：parserTests HTML 比对（归一化）在支持范围内通过。

### Task 3：安全与范围外标注
- **文件**：`src/render/html.rs`、`fuzz/fuzz_targets/render_html.rs`
- **做什么**：转义审计；`Unsupported` 节点渲染成可见占位/注释。
- **验证**：fuzz 无危险/畸形输出；范围外标注与诊断一致。

---

## 风险 / 提醒

- **最容易复发 D1 病**：HTML 一对比就手痒想贴近 MediaWiki，进而开始复刻 legacy 行为。守住"声明范围内正确"，范围外报警，到此为止。
- 这阶段是可选的——如果声誉和用户都来自 Stage 1/2，Stage 3 可以无限期搁置，不算欠债。
