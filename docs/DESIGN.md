# wikrs 设计文档 (Design Doc)

**状态:** Draft · **日期:** 2026-06-23 · **工作名:** `wikrs`（命名未锁定，见 §9）

---

## 0. 这份文档是什么

架构与设计决策的**单一事实来源**。读完应能回答：要建什么、为什么这么分层、模块怎么切、输入输出契约、错误处理哲学、性能目标、明确的非目标。

- 战略背景 / 决策链：见 [PROJECT-HANDOFF.md](PROJECT-HANDOFF.md)
- 每个阶段的施工 checkpoint 和 task：见 [stages/](stages/)
- 测试怎么做：见 [TESTING.md](TESTING.md)

本文档只放**稳定的架构决策**。会随实现演进，但每次改动要在 [../WORKLOG.md](../WORKLOG.md) 留一条。

---

## 1. 目标与定位

用 Rust 写一个 wikitext 处理工具：

- **保底（确定能拿到）**：又快一个量级的 WikiExtractor。wikitext → 干净 plain text，靠 Rust 的速度。
- **上行（声誉项目）**：又快又准、保留结构、对病态输入**报警而非静默丢弃**的现代 wikitext 引擎。

护城河是"难到劝退所有人"——前人倒在"正确 + 活跃维护"这两个词上。

---

## 2. 核心战略决策（不可动摇）

| # | 决策 | 理由 |
|---|------|------|
| **D1** | **不追求和 MediaWiki byte-level 一致** | 唯一完整规范是 6200 行 PHP 正则屎山；100% 兼容 = 复刻它所有 bug = 死结、必烂尾。 |
| **D2** | **诚实划界：声明范围内高正确率，范围外明确报错（不静默出错）** | 这是最强的技术叙事，也是和 WikiExtractor 的核心差异点。 |
| **D3** | **分层交付：先 ship 速度，再叠结构** | 下行有底（速度几乎确定），上行有空间（解析质量有风险）。先发布拿反馈。 |
| **D4** | **速度是兜底维度，正确性是上行维度** | 解析比预期难时，速度这维兜得住底，项目不会变负声誉墓碑。 |

---

## 3. 为什么 wikitext 难（设计必须正视的死结）

这不是工程努力能消除的结构性问题，设计上必须正面应对：

- **模板系统是文本宏处理器**（类似 C 预处理器），模板展开**不保证产出自包含 DOM**——有的模板只吐 `<table>` 开始标签、或单独一个 `<tr>`。
- 所以"先解析后展开"和"先展开后解析"**都不成立**，两者纠缠。连官方 Parsoid（全职团队 + 十几年）都没用干净单趟架构吃下模板，最后退回去调 PHP preprocessor。

**设计上的应对：**

- **Stage 1 绕开它**：纯文本 strip，不建 DOM，模板按 drop/whitelist 处理。死结碰不到。
- **Stage 2 用"诊断式"解析**：能干净解析的范围内建 AST；遇到模板纠缠/越界结构，产出 `Diagnostic` 报警并降级（保留原文 span），**绝不假装解析对了**。这正是 D2 的落地。

---

## 4. 架构总览

```
                         ┌─────────────── Stage 1（保底档）───────────────┐
 XML dump (.bz2)         │                                                │
 ───────────────►  dump::reader ──►  PageStream{title, ns, wikitext}      │
 单页 wikitext (stdin) ─────────────────────────────────┐                 │
                                                         ▼                 │
                                              extract::strip ──► plain text / JSONL
                         └────────────────────────────────────────────────┘

                         ┌─────────────── Stage 2（进阶档）───────────────┐
   wikitext ──► tokenizer ──► parser ──► AST ──► render::plain  (取代 Stage 1 strip)
                                          │  └──► render::struct (JSONL，保留 table/link/结构)
                                          └──► diag::Diagnostics（范围外报警，不静默丢）
                         └────────────────────────────────────────────────┘

                         ┌─────────────── Stage 3（可选）─────────────────┐
                                          AST ──► render::html
                         └────────────────────────────────────────────────┘
```

**关键点：** Stage 1 的 `extract::strip` 是一个**独立的、有意做成 lossy 的**文本剥离器，不是解析器——目的是尽快 ship 速度价值。Stage 2 引入真正的 tokenizer→parser→AST，`render::plain` 最终取代 `extract::strip`。两者刻意解耦，避免 Stage 1 的快糙设计锁死 Stage 2。

---

## 5. 模块 / Crate 结构

**起步：单 crate `wikrs`（lib + bin），不开 workspace。** 等 Stage 2 的 AST 稳定、且 `wikrs-dump` 想被独立复用时再拆 workspace。

```
wikrs/
├── Cargo.toml
├── src/
│   ├── lib.rs            # 公共 API 入口，re-export
│   ├── main.rs           # bin `wikrs`：CLI（clap）
│   ├── dump/             # 流式读 XML dump（multistream .bz2），产出 PageStream
│   ├── extract/          # Stage 1：wikitext → plain text（lossy strip）
│   ├── tokenizer/        # Stage 2：wikitext → token 流
│   ├── parser/           # Stage 2：token → AST
│   ├── ast/              # AST 类型定义（Node 枚举、span）
│   ├── render/           # plain / struct(JSONL) / html 渲染器
│   └── diag/             # Diagnostic 类型、severity、span、错误码
├── benches/              # criterion 基准（速度叙事）
├── fuzz/                 # cargo-fuzz target（安全网）
└── tests/                # 集成测试 + parserTests 跑测 + insta 快照
```

**每个模块单一职责；变在一起的放一起；优先小而专的文件。** `dump` 与 wikitext 语法完全无关，必须能独立测试。

### 依赖选型（初定，可在 WORKLOG 记录变更）

| 用途 | crate | 备注 |
|------|-------|------|
| 流式 XML | `quick-xml` | 事件式、零拷贝友好，dump 几十 GB 必须流式 |
| 解压 | `bzip2` / `flate2` | dump 是 multistream `.bz2`；可分块并行解压 |
| 并行 | `rayon` | 按页并行，吃满核 |
| 快速字节扫描 | `memchr` / `bstr` | strip 热路径 |
| CLI | `clap` (derive) | |
| 库错误 | `thiserror` | 库内 typed error |
| 应用错误 | `anyhow` | bin 层 |
| 快照测试 | `insta` | 回归保护 |
| 基准 | `criterion` | 速度数字 |
| 模糊测试 | `cargo-fuzz` (libFuzzer) | 不崩溃/不死循环/不爆内存 |

> tokenizer 用手写还是 `logos`：**待定**，Stage 2 开工时决定。先验证手写在病态输入上的最坏复杂度。

---

## 6. 输入 / 输出契约

### 输入
- **Wikimedia XML dump**：`pages-articles-multistream.xml.bz2`。流式迭代，**常数内存/页**。默认只取 namespace 0（正文），跳过 redirect。
- **单页 wikitext**：stdin 或 `--file`，给开发/调试和 library 用户用。

### 输出（`--format` 切换）
- `text`（默认，Stage 1）：每篇文章干净 plain text。
- `jsonl`：每行一个 `{title, text, ...}`，喂训练/RAG 管线最常用。
- `ast-json`（Stage 2）：结构化 AST。
- `html`（Stage 3）。

### 过滤 / 行为开关
- `--namespaces 0`、`--skip-redirects`（默认开）、`--min-text-len`、`--templates drop|whitelist`。
- **对标 WikiExtractor 的具体行为**（剥哪些、保留哪些）在 [stages/stage-1-extractor.md](stages/stage-1-extractor.md) 逐条列。

---

## 7. 错误与诊断哲学（D2 的落地）

不静默丢。处理过程产出结构化诊断：

```
Diagnostic {
  severity: Error | Warning | Unsupported,
  code:     &'static str,   // 稳定错误码，如 "E-TPL-NESTED", "U-TABLE-FROM-TEMPLATE"
  span:     Range<usize>,   // 原文字节范围，可定位
  message:  String,
}
```

- **范围内的真错** → `Error`。
- **能恢复的怪输入** → `Warning`，降级处理后继续。
- **声明不支持的构造**（如模板吐半个 table）→ `Unsupported`，保留原文 span，继续处理后面。

CLI 退出时打印汇总统计：`X 篇完全干净 / Y 篇有 Warning / Z 篇命中 Unsupported`。**这三个数字就是 README 里最有说服力的声誉证据**（详见 TESTING.md §2）。

---

## 8. 性能目标与手段

- **目标**：处理全量英文 dump 从 WikiExtractor 的"几小时" → "几十分钟"，可 benchmark（wall-clock + 吞吐 MB/s + 峰值内存）。
- **手段**：全程流式（绝不全量载入）、常数内存/页、`rayon` 按页并行、`memchr`/`bstr` 热路径、AST 尽量 `&str` borrow 零拷贝、避免正则回溯。
- **安全要求**（对标 MediaWiki）：2MB 恶意输入，最坏执行时间**线性而非平方**。由 fuzzing 守（TESTING.md §3）。

---

## 9. 命名（未决，动手锁前必查占用）

| 候选 | 含义 | 取舍 |
|------|------|------|
| **`wikrs`**（首选） | wiki + rs | 一眼是 Rust wiki 工具，不锁死在提取或解析任一层，有成长空间。当前工作目录已用此名。 |
| `mwx` / `mwparser` | mw = MediaWiki | 精准命中圈内人搜索 |
| `unwiki` | 拆掉 wiki 包装 | 有性格、有记忆点 |

> **动手前去 crates.io + GitHub + 域名查占用。** 这是 Stage 1 的 Task 0。

---

## 10. 非目标 (YAGNI)

明确**不做**，写进 README 管理预期：

- ❌ 和 MediaWiki byte-level 一致（D1）。
- ❌ 完整模板展开 / Lua(Scribunto) 执行。
- ❌ 可视化编辑器往返（Parsoid 的 data-* 注解）。
- ❌ 写 wikitext / 编辑功能。本项目只做**读取方向**：wikitext → text/AST/HTML。
