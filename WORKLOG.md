# WORKLOG — wikrs

> **Append-only 操作流水账。** 每条 = 一次有意义的操作，**最新追加在文件底部**。
> 格式：`## [YYYY-MM-DD] 标题` → 做了什么 / 为什么 / 产出。
> 不在这里写设计（设计进 docs/DESIGN.md），这里只记"发生了什么"。

---

## [2026-06-23] 初始化 docs/ 与交接文档

- **做了什么**：创建 `docs/` 目录；把冷启动交接文档写入 `docs/PROJECT-HANDOFF.md`。
- **为什么**：换环境冷启动需要单一背景来源，保住决策链不丢。
- **产出**：`docs/PROJECT-HANDOFF.md`

---

## [2026-06-23] 写规划文档：design + 各 stage + testing

- **做了什么**：在写任何代码前，先把文档骨架搭好。
  - `docs/DESIGN.md` — 架构总览、4 条核心战略决策、模块/crate 结构、依赖选型、I/O 契约、错误诊断哲学、性能目标、非目标。
  - `docs/TESTING.md` — 四层测试体系（parserTests / 真实 dump 差分 / fuzzing / 快照）+ 基准 + CI + 命令速查。
  - `docs/stages/stage-1-extractor.md` — 保底档：checkpoint、WikiExtractor 行为对照表、Task 0–6、基准设计。
  - `docs/stages/stage-2-ast.md` — 进阶档：AST + 诊断报警，checkpoint + Task 1–7。
  - `docs/stages/stage-3-html.md` — 可选档：AST→HTML，触发条件 + checkpoint + tasks。
  - `docs/README.md` — 文档索引 + 维护约定。
  - `WORKLOG.md` — 本文件（append-only 操作流水账）。
- **为什么**：用户要求先把文档写好（design doc + 每个 stage 独立的 checkpoint/task + 如何测试），再动代码。
- **关键决策**：起步单 crate（不开 workspace）；命名 `wikrs` 未锁，列为 Stage 1 Task 0；stage 文档停在 roadmap 层，真正执行某 stage 时再用 writing-plans 生成 code-complete TDD 计划。
- **产出**：上述 7 个文件。
- **下一步**：等用户定方向——锁命名 / 起 Cargo 骨架进 Stage 1 / 调整文档。

---

## [2026-06-24] 写根 README + git init + 推送 GitHub（private）

- **做了什么**：
  - 写英文根 `README.md`（项目对外的脸）+ `.gitignore`（Rust/dump 数据/macOS）。
  - `git init -b main`，stage 指定文件（不是 `-A`），首个 commit（10 文件，带 Co-Authored-By）。
  - `gh repo create wikrs --private --push`，创建并推送。
- **为什么**：用户要求 init git + 推 GitHub，先 private，README 写好。
- **关键决策**：README 用**英文**（开源声誉项目，目标读者以英文为主），内部 docs/ 暂留中文；仓库名锁 **wikrs**；License 暂定 dual MIT/Apache-2.0（public 前再放 LICENSE 文件）。
- **产出**：私有仓库 https://github.com/SFARL/wikrs（账号 SFARL，默认分支 main）。
- **验证**：`gh repo view` 确认 visibility=PRIVATE；远端文件树 = 本地 10 文件，无 .DS_Store。
- **下一步**：等用户定方向——锁 crate 名占用核查 / 起 Cargo 骨架进 Stage 1。

---

## [2026-06-24] crate 名核查 + Stage 1 骨架/CI + License 调研 + Stage 1 TDD 计划

- **做了什么**：
  - **名占用核查**：crates.io `wikrs` 可用（HTTP 404，`mwx`/`mwparser`/`unwiki` 也都可用）。锁名 **wikrs**。回填 DESIGN §9。
  - **Cargo 骨架 + CI**：lib+bin（`Cargo.toml`、`src/lib.rs`、`src/main.rs`、`src/dump.rs`、`src/extract.rs` stub）+ `.github/workflows/ci.yml`（fmt+clippy`-D warnings`+test）。本地四件套全绿，`wikrs --help` 可用。
  - **License 调研**：MediaWiki 核心 = **GPL-2.0-or-later**；`parserTests.txt` 也是 GPL。→ 新增 DESIGN §11 + 改 TESTING 层1：wikrs 走 MIT/Apache，**绝不抄 MediaWiki 代码（clean-room）**，**parserTests 不入库、测试时下载**。
  - **Stage 1 TDD 计划**：`docs/superpowers/plans/2026-06-24-stage-1-extractor.md`，code-complete，Task 1–9（dump 流式读 → strip 各 pass → CLI/rayon → 基准 → fuzz）。Stage-1 doc Task 0 标记完成。
- **为什么**：用户要 (1) 做名核查 (2) 进 Stage 1 起骨架+CI 再生成 TDD 计划 (3) 问 MediaWiki license。
- **关键决策**：edition 2021；CLI 先整体 collect（大 dump 再换 bounded channel，已在计划注明）；License 文件先不放，等用户拍板。
- **产出**：commit `521f3a8`，已 push。骨架 4 件套绿。
- **下一步**：执行 Stage 1 计划（subagent-driven 或 inline）/ 放 LICENSE 文件 / 调计划。

---

## [2026-06-24] 搭好 test 框架 + 三个对比基线

- **做了什么**：在写 strip 逻辑前，先把测试/对比框架立起来。
  - **workspace + xtask**：`cargo xtask fetch-parser-tests`（拉 GPL parserTests.txt 到 gitignore 目录，528K/1077 例，不入库）；`bench-compare` 骨架（待 CLI）。
  - **parserTests 框架**：`tests/parser_tests.rs` 写了格式解析器（`!! test/wikitext/html[/php|/parsoid]/end`，忽略 `!! article`），inline 样例 + 真 fixture 加载都过；逐例一致性比对 `#[ignore]`（待 Stage 2 render::html）。CI 无 fixture 时优雅 skip。
  - **parse_wiki_text 对比**：dev-dependency + `benches/compare.rs`（criterion），样例 ~258 MiB/s 基线；wikrs strip 实现后加入同组。
  - **WikiExtractor 对比**：`tools/wikiextractor/`（uv venv，**pin Python 3.10**——3.11+ 因行内 `(?i)` 正则报错）+ setup.sh + README。
  - **样例 fixture**：`tests/fixtures/sample_article.wikitext`（自己写的，非 GPL）。
- **为什么**：用户要先搭 test 框架，并把 parserTests / parse_wiki_text / WikiExtractor 都接成对比。
- **关键决策**：三个对比都是 dev/外部，不进发布物、不沾 license；parse_wiki_text 用原版 0.1.5（编译 OK）；WikiExtractor 3.0.6 + Python 3.10。
- **验证**：fmt + clippy(`-D warnings`, all-targets, workspace) + test 全绿；GPL fixture 和 .venv 确认未入库；对比基准实测出数。
- **产出**：commit `3951c1a`，已 push。
- **下一步**：执行 Stage 1 计划实现 `extract::strip`（实现后对比基准/parserTests 才有 wikrs 这一侧的数）/ 放 LICENSE。

---

## [2026-06-24] bench 脚本 + wikrs-dev-workflow 项目 skill

> 本条用新 skill 定义的格式记录（Change / Tests / Benchmark / Regression），作为示范。

- **Change:** 新增 `scripts/bench.sh`（跑 `cargo bench --bench compare` + 抽 time/thrpt/change 摘要）；新增项目级 skill `.claude/skills/wikrs-dev-workflow/SKILL.md`，把"每改动 → 先写失败单测 → 跑 bench → tests 绿且 bench 无静默回退 → 记 WORKLOG + 刷 README"固化成流程；README 加 "Benchmarks & test status" 公开记分牌。
- **Tests:** 无 Rust 改动；`cargo test --all-features` 仍绿（沿用上次）。
- **Benchmark:** 无 perf 相关改动。基线现值 `parse_wiki_text` ~314 MiB/s（机器较空，相对上次 258 偏高，属噪声，非代码变化）。
- **Regression?** none。
- **关键决策:** bench gate 取"无_无法解释_的回退"而非"必须每次更快"——纠正/新增 case 允许带来**可记录的合理回退**，避免逼着造假或跳过难题；skill 放仓库内 `.claude/skills/`，随项目分发、团队共用。
- **产出:** commit（见下），已 push。
- **下一步:** 用这个 workflow 跑 Stage 1 第一个 Task（先写失败单测）。

---

## [2026-06-24] skill 补提交信息规则

- **Change:** `wikrs-dev-workflow` skill 加"git 提交信息"规则——subject ≤20 字符、且必带 benchmark 数值（新吞吐或 Δ）；详情 + `Co-Authored-By` 入 body。目的：`git log --oneline` 直接当性能历史看。
- **Tests:** 无 Rust 改动，不影响 CI。
- **Benchmark:** 无 perf 改动；最新基线 `parse_wiki_text` 258 MiB/s。
- **Regression?** none。
- **本条 commit 即首例**：subject 走新规则。
