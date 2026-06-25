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

### 层 2 — 规模验证：真实 dump 差分测试（声誉证据来源）

- **是什么**：同一批真实页面，我们的输出 vs ground truth，做**结构化 DOM diff**。
- **ground truth 来源**：本地 MediaWiki/Parsoid，或调 Wikipedia REST API（`/page/html/{title}`）拿官方 HTML。
- **diff 方法**：两边都**归一化**后比结构和文本，**忽略无意义格式差异**（空白、属性顺序、自闭合写法等）。
- **产出三个数字**（README 头条）：
  ```
  X% 完全一致  /  Y% 仅结构差异  /  Z% 主动报错(Unsupported)
  ```
  关键是 **Z 不算失败**——主动报错正是和 WikiExtractor"静默出错"的差异点。
- **落点**：`tests/diff/`（取页面、归一化、diff、出报告的工具）+ `xtask diff-report` 命令。
- **采样**：固定一个 seed 抽 N 万 ns0 页面，结果可复现。

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
| **MediaWiki `parserTests.txt`** | 正确性 oracle（wikitext→期望 HTML），**GPL** | `cargo xtask fetch-parser-tests` 拉取（不入库）→ `cargo test --test parser_tests` 解析+加载 | ✅ 解析/加载就绪（1077 例）；逐例一致性比对待 Stage 2 `render::html` |
| **`parse_wiki_text`** | 最认真的民间 Rust 解析器（0.1.5/2018，停更），速度基线 | `cargo bench --bench compare`（dev-dependency，**不进发布物**） | ✅ 样例 ~319 MiB/s；与 `wikrs_strip`（~118）同组 |
| **WikiExtractor** | Python 事实标准提取器，速度+行为基线 | `tools/wikiextractor/setup.sh`（venv，**pin Python 3.10**）→ `cargo xtask bench-compare <dump>` | ✅ 端到端 8.3 MB dump：**wikrs ~22× 更快** |
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
| 生成差分报告（三个数字） | `cargo xtask diff-report --pages 50000 --seed 42` |
| 生成支持范围清单 | `cargo xtask supported` |

> 标 `xtask` 的是自定义任务，Stage 1 / Stage 2 里建。

---

## 每个 stage 的测试门槛（DoD 摘要）

| Stage | 必须有 | 门槛 |
|-------|--------|------|
| 1 提取器 | 单元 + 快照 + **vs WikiExtractor 基准** | 输出与 WikiExtractor 行为对齐（逐条对照），且**速度快一个量级**可复现 |
| 2 AST | 全四层 + 差分报告 | parserTests 覆盖率达标 + 产出"X/Y/Z 三个数字" + fuzz 无崩溃 |
| 3 HTML | 层 1（HTML 比对）+ 快照 | parserTests HTML 比对在支持范围内通过 |

详细 checkpoint 见各 stage 文档。
