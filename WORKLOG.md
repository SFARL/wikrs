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
