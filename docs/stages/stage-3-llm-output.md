> *Internal dev-history document (Chinese). For English, see [DESIGN.md](../DESIGN.md) and [TESTING.md](../TESTING.md).*

# Stage 3 — LLM 取向的结构化输出（sections JSONL + Markdown）

**状态:** sections 已发布（v0.2.0）· Markdown 未开工 · **日期:** 2026-07-02 · **定位:** Stage 2 AST 的 LLM 消费面

> 取代原 [stage-3-html.md](stage-3-html.md)（HTML 方案，已否决——见下"决策史"）。
> 设计见 [../DESIGN.md](../DESIGN.md)；测试见 [../TESTING.md](../TESTING.md)。

---

## 场景（触发条件，2026-07-02 拍板）

**用户 = LLM 数据管线。** 两个具体消费形态：

1. **RAG 切块**：需要按章节切、带层级元数据——`render::plain` 的平文本把 heading 层级扔了，下游只能瞎切。
2. **上下文注入 / 语料**：需要 Markdown 文本（GFM 表格、列表结构在 context window 里可读）。

预览/静态镜像场景不存在 → **HTML 出局**。

## 决策史（为什么不是 HTML）

- **2026-07-02 上午**：HTML 方案全量实现后被回滚（工作留档于 `feat/stage3-html` 分支）——违反"外部判据先行"：渲染语义（`==`→h2 等）落地时，能审判它们的 C2 harness 一行不存在。教训进 memory（stage3-ground-truth-first）。
- **2026-07-02 下午，方向重议**：HTML 的两个论据在本项目不成立——
  - "HTML 是信息超集"对 wikitext 成立、**对 wikrs AST 不成立**：AST 的 9 种节点（简单表格在内）GFM 全部装得下，复杂构造本来就落 `Unsupported`。
  - "Markdown 没有 ground truth"不对：**往返差分**就是它的外部锚点——`render::markdown` 输出 → 独立 GFM 实现（pulldown-cmark）解析 → 结构必须与源 AST 一致。heading 层级映射错、转义泄漏（字面 `*`/`_` 变格式符）都会当场红，且该性质可 fuzz。
- parserTests 的 HTML 期望输出继续闲置：它锚定的是 parse 侧语义，那一侧已有 ratchet + 差分覆盖。

## 输出契约

### `--format sections`（本次）

每页一行 JSON：

```json
{"title": "April", "sections": [
  {"level": 0, "heading": "", "text": "April (Apr.) is the fourth month…"},
  {"level": 2, "heading": "The Month", "text": "April comes between March and May…"},
  {"level": 3, "heading": "Fixed Events", "text": "…"}
]}
```

钉死的语义：

| 决策点 | 契约 |
|---|---|
| 切分 | **平铺**：每个顶层 `Heading` 节点开新 section，不分级嵌套；层级由 `level` 携带，下游可重建树 |
| `level` | 等号数（`==`→2，parser 既有语义，有测试钉着）；**导语 section = 0** |
| 导语 | 首个 heading 之前的内容,`level:0, heading:""`；页面以 heading 开头则**无导语 section**（不发空壳） |
| `heading` | heading 内联内容过 `render::plain`（含实体解码） |
| `text` | 该 section 的块节点过 `render::plain`——复用已验证路径（差分 precision-led）,`Unsupported` 沿用 strip 回退 |
| 连续 heading | 保留空 `text` 的 section（heading 本身是真实结构,下游自行取舍） |
| 边界拒绝 | `--format sections` + `--engine strip` → 硬错（无 AST 可切）;+ `--stats` → 硬错（stats 量的是纯文本 clean 率） |

**为什么这里不需要外部 harness（与 ground-truth-first 不矛盾）：** sections 是 **AST 的序列化**，不是向另一种标记语言的**翻译**——没有任何"我们的约定 vs 权威约定"的映射决策（不存在"`==` 该出几个 `#`"这类问题）。text/heading 走的 `render::plain` 已被 Stage 2 差分锚定；level 是 parser 的既有受测语义。自写单测即充分判据。

### `render::markdown`（开工中）

**Code-complete TDD 实施计划：[../superpowers/plans/2026-07-02-markdown-roundtrip.md](../superpowers/plans/2026-07-02-markdown-roundtrip.md)**（§0 是"准确性怎么比"的完整机制——规范形 NF、它抓什么、抓不了什么、防循环论证）。

**Harness 先行，先红后绿：**

1. 往返 harness：`parse(wikitext) → render::markdown → pulldown-cmark 解析 → 结构比对源 AST`（声明范围内断言一致；pulldown-cmark 进 dev-dependencies）。先对桩渲染器跑红。
2. 实现渲染语义至 harness 绿：`#`×level、`**`/`*`、`[label](target)`、嵌套列表、fenced `pre`、GFM 管道表格。
3. 转义是主战场（字面 `*`/`_`/`[`/行首 `#`/`|` 在表格内）——fuzz 同一往返性质。
4. `Unsupported` → 可见标记：fenced block 装 strip 文本（MD 无注释语法,fenced 块可见、可 grep、不污染正文格式）。

## Checkpoint（Definition of Done）

**sections（本次）：**
- [x] **S1** 单测覆盖契约表全部行（导语有无、平铺切分、level、空 section、实体、边界拒绝）。（2026-07-02：5 单测 + 2 CLI 测试）
- [x] **S2** CLI 端到端：真实二进制输出逐行可 `serde_json` 回解、字段正确。
- [x] **S3** 全量 simplewiki 跑通（281,799 页 **0 坏行**、平均 3.3 sections/页、端到端 4.81s）；criterion 无回归。

**markdown（下一步）：**
- [ ] **M1** 往返 harness 存在且先红后绿（pulldown-cmark 独立判据）。
- [ ] **M2** fuzz 往返性质零 crash/零断言失败。
- [ ] **M3** `Unsupported` 可见标记与诊断一致。
- [ ] **M4** 快照锁代表性页面；CLI `--format markdown`。

## 风险 / 提醒

- **schema 是对外契约**：`output` 模块虽 `#[doc(hidden)]` 无 semver 承诺,但 CLI 输出格式用户会依赖——字段增删要过 CHANGELOG。
- Markdown 阶段最容易复发 D1：GFM 表格和 MediaWiki 表格对不上时,守住"声明范围内"（AST 本来只有简单表格）,别去追表格方言。
