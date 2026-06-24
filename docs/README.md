# wikrs 文档索引

Rust wikitext 提取/解析引擎。保底=更快的 WikiExtractor，上行=现代 wikitext 引擎。

## 怎么读这些文档

| 想知道 | 看 |
|--------|-----|
| 战略背景、为什么做这个、决策链 | [PROJECT-HANDOFF.md](PROJECT-HANDOFF.md) |
| 架构、模块切分、I/O 契约、错误哲学、非目标 | [DESIGN.md](DESIGN.md) |
| 测试体系（四层）、怎么跑、声誉证据怎么来 | [TESTING.md](TESTING.md) |
| 操作流水账（append-only） | [../WORKLOG.md](../WORKLOG.md) |

## 分阶段路线

| Stage | 文档 | 一句话 |
|-------|------|--------|
| 1 保底档 | [stages/stage-1-extractor.md](stages/stage-1-extractor.md) | wikitext → plain text，卖点速度，**先发这个** |
| 2 进阶档 | [stages/stage-2-ast.md](stages/stage-2-ast.md) | 结构化 AST + 诊断报警，真正的声誉项目 |
| 3 可选 | [stages/stage-3-html.md](stages/stage-3-html.md) | AST → HTML，锦上添花 |

## 文档维护约定

- **DESIGN.md** 只放稳定架构决策；每次改动在 WORKLOG 留一条。
- **stage 文档** 里的 checkpoint 用 `- [ ]`，完成就勾。
- 真正开工某个 stage 时，用 `superpowers:writing-plans` 生成 code-complete 的 TDD 实施计划，存到 `docs/superpowers/plans/`，stage 文档里链过去。
- 每次有意义的操作 → append 一条到 [../WORKLOG.md](../WORKLOG.md)。
