# wikrs 测试策略 (Testing)

**状态:** Draft · **日期:** 2026-06-23

> **目标不是"逼近 MediaWiki 100% 一致"（死结），是"声明范围内高一致 + 范围外报错"。**
> 测试体系是项目命门，**第一周就要搭骨架**——它既验证正确性，又直接产出 README 里的声誉证据。

---

## 核心卖点公式

> **"在英文维基 N 万随机页面上 X% 结构一致"** + 对剩下不一致的**清醒解释**。

这句话能不能立住，全靠下面第 1、2 层。

---

## 四层测试体系

### 层 1 — 地基：MediaWiki 官方 `parserTests.txt`

- **是什么**：MediaWiki 仓库里几千条 `wikitext → 期望 HTML` 配对，机器可读、公开。
- **怎么用**：
  1. 拉取 `parserTests.txt`，写一个解析器把它读成测试用例（每条含 `!! wikitext` / `!! html` 段）。
  2. 每条跑我们的引擎，比对结果。
  3. **能过的 = 我们声明的支持范围；过不了的 = 明确声明"不支持"并归档原因。**
- **产出**：一份 `SUPPORTED.md`（自动生成）——支持范围清单，本身就是 D2 诚实划界的证据。
- **⚠️ 许可**：`parserTests.txt` 是 **GPL**，**不能 vendor 进** MIT/Apache 仓库。改为测试时下载（`cargo xtask fetch-parser-tests` 拉到 **.gitignore 的** `tests/fixtures/`，不提交）。详见 [DESIGN.md](DESIGN.md) §11。
- **落点**：`tests/parser_tests.rs`（用例读取器 + 比对）；fixture 由 `xtask fetch-parser-tests` 拉取，**不入库**。
- **Stage 映射**：Stage 1 只可能过纯文本类用例；Stage 2 AST 起来后覆盖率才会涨。**把通过率当进度指标。**

### 层 2 — 规模验证：真实页面差分测试（声誉证据来源）

**已落地**（2026-06-27）：`cargo xtask diff-fetch` + `diff-report`，核心在 `src/diff.rs`（`wikrs::diff`）。

- **是什么**：一批真实英文维基页面，wikrs 抽出的正文 vs ground truth，做**文本级**差分。
- **ground truth 来源**：Wikipedia REST API（`/page/html/{title}`）拿 Parsoid 官方 HTML，用 `scraper` 抽可见正文。
- **为什么文本级而非 DOM**：Stage 2 只渲染 plain text（`render::plain`），没有 `render::html`（那是 Stage 3）。所以现在比的是 **wikrs plain text vs Parsoid HTML 的可见正文**。
- **diff 方法**：两边归一化成 3-词 shingle 集合，算 **precision**（wikrs 输出的有多少被原文佐证）与 **coverage**（原文正文 wikrs 复现了多少）。
- **关键设计——precision-led，不惩罚模板省略**：wikrs 按设计丢模板（D4 护城河），其输出是 Parsoid 正文的**子集**。所以头条是 precision（"wikrs 给的对不对"），coverage 单独报（模板省略的缺口，**非失败**）。
- **页级三桶 vs 真实数据**：`classify` 把每页分 `Faithful`/`Divergent`/`Reported`。但 "Reported" 是页级的——任一越界构造（`{|` 表格 / `<math>` / gallery）就整页标记，而真实 featured 文章必含其一 → **页级桶在真实页面坍缩成 0/0/100**，诚实但无信息量。
- **真正的头条（fidelity overlay，逐页、与桶无关）**——种子样本（18 篇 featured，2026-06-27）：
  ```
  mean precision ~91%  /  coverage ~49%  /  13-of-18 faithful  /  0 silent outliers
  ```
  precision 是**保守下限**（~9% 缺口是 `<math>`/实体/分词噪声，不是 garbling——聚类紧、零离群）。页级 0/0/100 作为**透明度层**保留（每页都标记跳过了什么 = 和 WikiExtractor"静默出错"的对比点）。
- **落点**：`src/diff.rs`（归一化/precision/coverage/classify/Report，零依赖、7 单测）；`xtask diff-fetch`/`diff-report`；`tests/diff_report.rs`（离线集成 smoke，CI 无网络）。
- **采样**：`tests/diff/titles.txt`（18 篇 featured，仅页名、入库、可复现）+ `tests/diff/titles-random.txt`（25 篇随机 ns0，`cargo xtask diff-sample` 抽样并 pin）。页面内容 CC-BY-SA 运行时拉、缓存到 gitignore 的 `tests/diff/cache/`，不入库。**随机样本更诚实**：25 页随机样本**初测** precision ~82%、40% 静默 structural-diff（featured 样本掩盖了简单页的 markup 泄漏）。**这些已修**——entity 解码 / File·Category 丢弃 / order-robust 度量 / `]]` File-caption / 多行模板碎裂 / 表格 brace 修复后，**120 页随机样本上 word-precision 99.3%、0% 静默、115/120 完全忠实**（`cargo xtask diff-report --cache target/diff-cache-random`）。真实 dump 转化率另见 README（simplewiki 全量 98.0% clean）。

### 层 3 — 安全网：fuzzing (`cargo-fuzz`)

- **目标**：喂畸形 wikitext，保证**不崩溃 / 不死循环 / 不爆内存**。
- **硬指标**（对标 MediaWiki）：**2MB 恶意输入，最坏执行时间线性而非平方。** 这是 Rust vs Python/PHP 的安全故事。
- **target**：`fuzz/fuzz_targets/parse.rs`、`fuzz/fuzz_targets/strip.rs`。
- **运行**：`cargo +nightly fuzz run parse`；CI 里跑短时 + 语料回归。
- **崩溃即 P0**：任何 panic/超时进 `fuzz/corpus` 回归集。

### 层 4 — 回归保护：snapshot 测试 (`insta`)

- **目标**：防止改坏已经对的东西。
- **怎么用**：选一组代表性 wikitext 片段（链接、表格、列表、嵌套模板、ref、病态输入），`insta::assert_snapshot!` 锁住输出。
- **落点**：`tests/snapshots/`。改动审查靠 `cargo insta review`。

---

## 支撑层（不算四层，但必须有）

### 单元测试
每个模块就近测：`dump`（喂小 XML，验证页面切分/redirect 跳过/ns 过滤）、`tokenizer`、`parser`、`extract`、`diag`。`dump` 与语法解耦，必须能独立测。

### 基准 (`criterion`) — 速度叙事的硬证据
- **落点**：`benches/extract.rs`、`benches/parse.rs`。
- **核心对比基准**（项目立项的命门）：**同一个 dump 切片，wikrs vs WikiExtractor**，报告 wall-clock + 吞吐 MB/s + 峰值内存。
- 详见 [stages/stage-1-extractor.md](stages/stage-1-extractor.md) 的 benchmark 任务。这个数字立不住，Stage 1 不算完成。

### CI
- `cargo test` + `cargo clippy -D warnings` + `cargo fmt --check`。
- 定时跑层 2 差分报告（慢，不进每次 PR）。
- 短时 fuzz smoke。

---

## 对比基线（comparison baselines）

三个对比对象，**框架已搭好**（数字待 `extract::strip` / engine 实现后填）：

| 基线 | 是什么 | 怎么用 | 状态 |
|------|--------|--------|------|
| **MediaWiki `parserTests.txt`** | 正确性 oracle（wikitext→期望 HTML），**GPL** | `cargo xtask fetch-parser-tests` 拉取（不入库）→ `cargo test --test parser_tests` | ✅ 1077 例已加载；**Stage 2 零诊断覆盖率 49.1%**（`stage2_coverage_rate`）；逐例 HTML 一致性待 `render::html` |
| **真实 dump 全量验证** | 规模层：真实异构语料上的转化率/内存/吞吐（`--stats` 残留标记 floor） | 下载 dump → `wikrs --input <dump.xml.bz2> [--index <ms-index>] --stats` | ✅ **全量 enwiki 7,189,653 页 98.0% clean、7.4 min（`--index` 并行）/ 38 min（单流）、零崩溃**；全量 simplewiki 同为 98.0%（跨语料一致）。挖出并修掉：`]]` File-caption 泄漏、`{{…\|}}` 表格碎裂、dump 实体静默丢失 |
| **`parse_wiki_text`** | 最认真的民间 Rust 解析器（0.1.5/2018，停更），速度基线 | `cargo bench --bench compare`（dev-dependency，**不进发布物**） | ✅ 样例 ~319 MiB/s；与 `wikrs_strip`（~118）同组 |
| **WikiExtractor** | Python 事实标准提取器，速度+行为基线 | `tools/wikiextractor/setup.sh`（venv，**pin Python 3.10**）→ `cargo xtask bench-compare <dump>` | ✅ 全量真实 simplewiki（1.67 GB）：**wikrs ~32× 更快**（322 vs 10.2 MB/s；8.3 MB 合成 dump 为 ~22×——小输入被 wikrs 启动开销压低，见 WORKLOG 2026-06-30） |
| **Bliki**（Java，via XWiki） | mature 的 wikitext→HTML 引擎（含**模板展开**），上游已弃 | `tools/bliki/setup.sh`（JDK + coursier 取 jar）→ `cargo xtask bench-bliki` | ✅ 样例 ~**0.4 MB/s**（wikrs strip ~118，**约 300× 差距**；它做的多但慢得多） |

> `parse_wiki_text` / WikiExtractor / **Bliki** 都是 **dev / 外部对比**，不进 wikrs 发布物，不沾它们的 license（Bliki 的 jar + 编译产物 gitignore，不入库）。parserTests 是 GPL，故只在测试时下载、不 vendor（[DESIGN.md](DESIGN.md) §11、前人对比见 §12）。
> WikiExtractor 3.0.6 用了行内 `(?i)` 正则 flag，Python 3.11+ 直接报错，故 pin 3.10（uv 托管，不动系统 Python）。

## 命令速查

| 做什么 | 命令 |
|--------|------|
| 全部单元 + 集成测试 | `cargo test` |
| 拉取 parserTests.txt（GPL，不入库） | `cargo xtask fetch-parser-tests` |
| 跑 parserTests 解析/加载（1077 例） | `cargo test --test parser_tests` |
| 看快照 diff 并接受 | `cargo insta review` |
| 跑模糊测试 | `cargo +nightly fuzz run parse` |
| 跑对比基准（parse_wiki_text 基线） | `cargo bench --bench compare` |
| 装 WikiExtractor（Python 对比基线） | `tools/wikiextractor/setup.sh` |
| wikrs vs WikiExtractor 端到端 | `cargo xtask bench-compare <dump>` |
| 装 Bliki（Java 对比基线） | `tools/bliki/setup.sh` |
| 跑 Bliki 基准 | `cargo xtask bench-bliki` |
| 抽随机 ns0 标题并 pin（可复现样本） | `cargo xtask diff-sample --count N --out tests/diff/titles-random.txt` |
| 取真实页面到差分缓存（gitignore） | `cargo xtask diff-fetch` |
| 生成差分报告（precision/coverage 三个数字） | `cargo xtask diff-report` |
| 生成支持范围清单 | `cargo xtask supported` |

> 标 `xtask` 的是自定义任务，Stage 1 / Stage 2 里建。

---

## 每个 stage 的测试门槛（DoD 摘要）

| Stage | 必须有 | 门槛 |
|-------|--------|------|
| 1 提取器 | 单元 + 快照 + **vs WikiExtractor 基准** | 输出与 WikiExtractor 行为对齐（逐条对照），且**速度快一个量级**可复现 |
| 2 AST | 全四层 + 差分报告 | parserTests 覆盖率达标 + 产出 precision/coverage 三个数字（precision-led）+ fuzz 无崩溃 |
| 3 HTML | 层 1（HTML 比对）+ 快照 | parserTests HTML 比对在支持范围内通过 |

详细 checkpoint 见各 stage 文档。
