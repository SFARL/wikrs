# Stage 1 — 保底档：wikitext → plain text 提取器

**状态:** 未开工 · **日期:** 2026-06-23 · **定位:** Rust 版 WikiExtractor，卖点是**速度**

> 先把这层做出来拿第一波反馈，再往上叠。即使解析质量只和 WikiExtractor 打平，光速度就有人用、能拿 star。
> 设计背景见 [../DESIGN.md](../DESIGN.md) §4–§6；测试见 [../TESTING.md](../TESTING.md)。

---

## 目标

输入 Wikimedia XML dump（或单页 wikitext），输出干净 plain text，**比 WikiExtractor 快一个量级**，且行为可逐条对照。

**不做**：建 AST、保留结构、模板展开。这些是 Stage 2。Stage 1 是有意 lossy 的 strip。

---

## Checkpoint（Definition of Done）

全部满足才算 Stage 1 完成，可以发首个 release：

- [ ] **C1** 能流式读 `pages-articles-multistream.xml.bz2`，常数内存/页，正确切分页面、跳过 redirect、过滤到 ns0。
- [ ] **C2** `wikitext → plain text` 行为与 WikiExtractor **逐条对照表**对齐（见下"行为对照"），偏差都是有意识的、记录在案的。
- [ ] **C3** CLI 可用：`wikrs --input dump.xml.bz2 --format text|jsonl -o out/`，带 namespace/redirect/模板开关。
- [ ] **C4** **基准立住**：同一 dump 切片，wikrs vs WikiExtractor，wall-clock + 吞吐 MB/s + 峰值内存，**快一个量级可复现**。
- [ ] **C5** 快照测试锁住代表性输入；fuzz smoke 跑通不崩溃。
- [ ] **C6** README 有跑法 + 基准数字 + 明确的"非目标/已知差异"。

---

## 行为对照（对标 WikiExtractor，逐条定）

> C2 的核心。开工第一件事是把这张表填全——**先定行为，再写代码**。

| 构造 | WikiExtractor 行为 | wikrs Stage 1 行为（待确认） |
|------|--------------------|------------------------------|
| 内链 `[[A\|text]]` | 保留 `text` | 同 |
| 外链 `[url text]` | 保留 `text` | 同 |
| 模板 `{{...}}` | 丢弃 | 默认丢弃；`--templates whitelist` 保留白名单 |
| 表格 `{\| ... \|}` | 丢弃 | 丢弃（Stage 2 才保留） |
| 标题 `== H ==` | 保留文本 / 可选标记 | 保留文本，可选 `--keep-headings` |
| 列表 `* / #` | 保留文本 | 同，行首符号可选保留 |
| 粗斜体 `''' ''` | 去标记留文本 | 同 |
| `<ref>...</ref>` | 丢弃 | 丢弃 |
| `<nowiki>` / 注释 `<!-- -->` | 去除 | 去除 |
| HTML 标签 | 去标签 | 去标签 |
| 文件/图片 `[[File:...]]` | 丢弃（含 caption？） | 丢弃，caption 处理待定 |

（"?"项开工时查 WikiExtractor 源码逐条敲定，结论回填本表。）

---

## Tasks

> 这是 roadmap 层的 task 拆分（目标 / 涉及文件 / 做什么 / 如何验证）。
> **code-complete TDD 实施计划（每步含失败测试→实现→通过→commit）已生成：**
> 👉 [../superpowers/plans/2026-06-24-stage-1-extractor.md](../superpowers/plans/2026-06-24-stage-1-extractor.md)。下面的 Task 编号与计划对应。

### Task 0：命名占用核查 + 工程初始化 ✅ 完成（2026-06-24）
- **文件**：`Cargo.toml`、`src/lib.rs`、`src/main.rs`、`src/dump.rs`、`src/extract.rs`、`.github/workflows/ci.yml`
- **做了什么**：crates.io 核查（`wikrs` 可用），锁名 `wikrs`；建 lib+bin 骨架（模块 stub）；CI = fmt + clippy(`-D warnings`) + test。
- **验证**：本地 `cargo fmt --check` / `cargo clippy -D warnings` / `cargo test` / `cargo build` 全绿；`wikrs --help` 可用。结果已记入 WORKLOG + DESIGN §9。

### Task 1：dump 流式读取
- **文件**：`src/dump/`（`reader.rs`、`page.rs`）、`tests/dump.rs`
- **做什么**：用 `quick-xml` + `bzip2` 流式迭代 `<page>`，产出 `Page{title, ns, redirect, wikitext}`；跳 redirect、过滤 ns。
- **验证**：喂一个小 multistream 切片，单元测试断言页面数、标题、ns 过滤、redirect 跳过；内存常数（不随 dump 增长）。

### Task 2：行为对照表敲定
- **文件**：本文件"行为对照"表
- **做什么**：读 WikiExtractor 源码，把每个构造的处理逐条确认，回填表，标注有意偏差。
- **验证**：表无 "?"；每条有对应计划测试用例 id。

### Task 3：strip 核心（wikitext → plain text）
- **文件**：`src/extract/`（`strip.rs`、`links.rs`、`templates.rs`、`tags.rs`）、`tests/snapshots/`
- **做什么**：按对照表实现剥离；热路径用 `memchr`/`bstr`；模板/ref/表格按规则丢弃；链接留锚文本。
- **验证**：每个构造一个 `insta` 快照；与对照表预期一致。

### Task 4：CLI + 输出格式
- **文件**：`src/main.rs`、`src/extract/output.rs`
- **做什么**：`clap` 接 `--input/--file/--format text|jsonl/-o/--namespaces/--skip-redirects/--templates/--keep-headings`；`rayon` 按页并行；输出分片写盘。
- **验证**：端到端跑小 dump 出 text 和 jsonl；并行结果与串行一致（顺序无关内容一致）。

### Task 5：基准（C4 命门）
- **文件**：`benches/extract.rs`、`xtask`/脚本：拉同一 dump 切片、跑 WikiExtractor、跑 wikrs、出对比表
- **做什么**：`criterion` 跑 wikrs；脚本量 WikiExtractor wall-clock；报告吞吐 MB/s + 峰值内存。
- **验证**：同输入下 wikrs **快约一个量级**，数字可复现，写进 README。

### Task 6：fuzz smoke + 发布打磨
- **文件**：`fuzz/fuzz_targets/strip.rs`、`README.md`、`CHANGELOG.md`
- **做什么**：strip 的 fuzz target 跑通不崩溃；写 README（跑法/基准/非目标/已知差异）；准备 crates.io 首发。
- **验证**：短时 fuzz 无 panic/超时；README 自查清单过；`cargo publish --dry-run` 通过。

---

## 风险 / 提醒

- **别让 Stage 1 偷偷长成解析器**。一旦发现"为了剥干净不得不建结构"，那是 Stage 2 的信号，打住、记一笔、按 lossy 处理。
- 基准要诚实：同机、同输入、同 warmup，写清环境，否则"快一个量级"会被质疑。
- multistream bz2 的分块边界容易出 bug，dump 读取要重点测。
