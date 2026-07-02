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

---

## [2026-06-24] Stage 1 Task 1：dump 流式读取

- **Change:** 实现 `dump::Page` + `dump::Pages`（quick-xml 0.40 流式迭代 `<page>`，跳 redirect、记 ns、解 XML 实体）；`Page::is_article()` = ns0 且非 redirect。TDD：先写失败单测 → 实现 → 绿。
- **Tests:** 新增 2 个单测（字段解析、ns/redirect 过滤）；`cargo test --all-features` 全绿；clippy `-D warnings` 干净。
- **Benchmark:** dump **不在 benched 路径**（`benches/compare.rs` 目前只测 parse_wiki_text，wikrs `strip` 未实现）。当前基线读数 ~304 MiB/s，相对上次 ±1.5% 属噪声。→ **wikrs 自己的吞吐数从 Task 6（strip 串通）才开始进 commit subject / README。**
- **Regression?** none。
- **备注:** quick-xml 0.40 去掉了 `BytesText::unescape()`，改用 `decode()` + `quick_xml::escape::unescape()`。
- **下一步:** Task 2（`dump::open` 支持 `.xml.bz2` multistream）。

---

## [2026-06-24] Stage 1 Task 2：dump::open（含 .xml.bz2）

- **Change:** `dump::open(path)` 按扩展名透明解压 multistream `.bz2`（bzip2 0.6 `MultiBzDecoder`），否则直读；返回 `Pages<Box<dyn BufRead>>`。
- **Tests:** 新增 `tests/dump_open.rs`（写 `.xml` 和 `.xml.bz2` 各跑一遍，断言解出同一页）；`cargo test --all-features` 全绿。
- **Benchmark:** 仍不在 benched 路径（dump 不进 bench）；沿用 ~304 MiB/s 基线读数，无变化。
- **Regression?** none。

---

## [2026-06-24] Stage 1 Task 3–6：extract::strip 管道串通 ⭐ 里程碑

- **Change:** 实现 `extract::strip` 四个 pass + 编排：
  - `comments`：去 `<!-- -->` / `<ref>…</ref>` / `<nowiki>`（大小写不敏感，leak-free）
  - `templates`：去 `{{…}}` / `{|…|}`（嵌套感知，UTF-8 安全）
  - `links`：`[[A|t]]`→t、`[[File:…]]`→空、`[url t]`→t、裸 url→空
  - `markup`：标题 / 列表符 / 粗斜体 / 残留标签
  - `strip()`：comments→templates→links→markup→collapse 空行
  并把 `wikrs::extract::strip` 接进 `benches/compare.rs`。
- **Tests:** 每个 pass 都有单测（comments×2、templates、links、markup）+ `strip()` 端到端单测 + `tests/strip_snapshots.rs`（insta，已人工核对输出正确）。`cargo test --all-features` 全绿，clippy `-D warnings` 干净。
- **Benchmark:** **首次有 wikrs 自己的数** → `wikrs_strip` ~118 MiB/s vs `parse_wiki_text` ~319 MiB/s。
- **Regression?** none（新代码，无既有基线可退）。**诚实标注：wikrs 当前比 parse_wiki_text 慢**——工作量不同（strip 产出完整 owned 文本、5 趟分配；parse_wiki_text 建借用 AST、不产文本）。**Stage 1 真正要赢的是 vs WikiExtractor（Python，Task 8）**，不是 parse_wiki_text。strip 是诚实未优化基线，单趟化是后续 perf 活，workflow 会跟踪。
- **里程碑:** wikrs 第一次能把 wikitext → 干净纯文本；README 记分牌从 pending 变真实数字。
- **下一步:** Task 7（CLI + rayon 端到端 text/jsonl）→ Task 8（vs WikiExtractor 基准，立"快一个量级"）。

---

## [2026-06-24] Stage 1 Task 7：CLI + 输出 + 转化率指标

- **Change:** `wikrs` CLI（clap）：`--input`、`--format text|jsonl`、`--stats`；`rayon` 并行 strip；`output::to_jsonl`。新增 `extract::looks_clean`（残留 `{{`/`[[`/`{|` 检测）作为"转化率"指标，`--stats` 打印 `pages / clean / %`。
- **Tests:** `output` 单测、`looks_clean` 单测、`tests/cli.rs`（text 输出 + 转化率统计，2 个集成测试）、`tests/parser_tests.rs::stage1_conversion_rate`（跑 1077 真实 case，floor>90%）。`cargo test --all-features` 全绿，clippy 干净。
- **转化率（新指标，回答"test 要不要含转化率"）:** parserTests 1077 例 → **98.1% 干净转化**（输出无残留括号标记）。**诚实标注**：这是"标记有没有漏出来"的宽松下限，**不是正确性**；真正确性比对在 Stage 2（vs Parsoid）。21 个漏标记 case = strip 边界，后续硬化。
- **Benchmark:** strip 未改，~118 MiB/s 不变（CLI/looks_clean 不在 benched 路径）。
- **Regression?** none。
- **下一步:** Task 8（vs WikiExtractor 端到端基准，立"快一个量级"）。

---

## [2026-06-24] Stage 1 Task 8：vs WikiExtractor 端到端基准 ⚡ 命门数字

- **Change:** xtask 加 `make-sample-dump`（从 `sample_article` 生成 N 页合成 dump，**逐标签换行**以喂 WikiExtractor 的行式解析器——单行格式会让它抽出 0 篇）+ 实现 `bench-compare`（构建 release → 分别计时 wikrs 与 WikiExtractor → 算吞吐 + speedup）。
- **Tests:** xtask 编译 + clippy `-D warnings` 干净；既有测试全绿不受影响。
- **Benchmark（命门）:** 8.3 MB 合成 dump（5000 篇）：**wikrs ~0.18 s / 47 MB/s vs WikiExtractor ~3.9 s / 2.1 MB/s → ~22× 更快**（连跑 3 次稳定 22.0–22.1×）。strip 微基准不变 ~118 MiB/s。
- **Regression?** none。
- **意义:** **保底档核心论点"又快一个量级"实测成立（实为 20×+）。** README 头条从"roughly an order of magnitude"改成实测 ~22×。
- **诚实标注:** 合成 dump = 同一篇文章重复，真实异构 dump 比例会变；WikiExtractor 的启动/模板预处理有固定开销，大 dump 上会摊薄——但量级成立。
- **下一步:** Task 9（fuzz + README usage + 首发 0.1.0）。Stage 1 接近收尾。

---

## [2026-06-24] Stage 1 Task 9：robustness/fuzz + README + 0.1.0 收尾

- **Change:**
  - `tests/robustness.rs`：11 种畸形输入不 panic + 2 MB 线性（~150 ms）。**CI 常跑，不需 nightly。**
  - cargo-fuzz 脚手架 `fuzz/`（独立 workspace + root `exclude=["fuzz"]`，不进 CI；`cargo +nightly fuzz run strip` 按需深 fuzz）。
  - README：Usage、Known differences vs WikiExtractor、Robustness、status 更新；新增 `CHANGELOG.md`（0.1.0）。
  - 版本 `0.0.0 → 0.1.0`（Stage 1 feature-complete）；`publish=false` **暂留**（发布是公开动作，待用户拍板）。
- **Tests:** robustness 2 个全绿；全量 19 测试通过，clippy `-D warnings` 干净；确认 fuzz 不进 root workspace（members = xtask, wikrs）。
- **Benchmark:** strip 不变 ~118 MiB/s；2 MB 畸形输入 ~150 ms（线性，非平方）。
- **Regression?** none。
- **⚠️ 待用户决定:** 发 `0.1.0` 到 crates.io 会让**源码公开**——与当前 **private** 仓库冲突。两条路：① 转 public 再发布；② 先留私有，晚点发。我不会擅自 publish。

---

## [2026-06-24] Stage 2 起步：AST + plain 渲染器（thin slice 地基）

- **决定:** **不转 public、不发布**——`PROJECT-HANDOFF.md` 是内部战略备忘，公开到"声誉"项目反而减分；仓库留 **private**，转头做 Stage 2（真正的声誉项目）。GitHub 可见性未改。
- **Change:** 新增 `src/ast.rs`（`Node<'a>` 枚举，`Cow<'a,str>` borrow-friendly：Text/Bold/Italic/Link/Heading/Paragraph + **`Unsupported` 占位**——范围外构造保留原文、配诊断，不假装）；`src/render.rs`（`render::plain(&[Node])`）。先把"AST → 文本"这端立起来，再用 tokenizer/parser 去填（thin vertical slice，避免一上来奔完整引擎淹死）。
- **Tests:** `render::renders_ast_to_plain_text`（手构 AST → 纯文本）。全量 **20 测试绿**，clippy `-D warnings` 干净。
- **Benchmark:** 不在 benched 路径（strip 仍 ~118 MiB/s）。Stage 2 串通后把 `render::plain` 接进 bench 与 strip 对比。
- **Regression?** none。
- **下一步:** tokenizer（wikitext → token 流，最坏复杂度线性）→ parser（token → AST，范围外发 `Unsupported` + Diagnostic）。先做一个最小诚实子集（段落/粗斜体/标题/链接）。

---

## [2026-06-24] Stage 2：tokenizer + 最小子集 parser + diagnostics ⭐

- **Change:**
  - `src/tokenizer.rs`：inline 分词器（Text/Bold/Italic/LinkOpen/LinkClose/Pipe），单趟线性，ASCII marker、UTF-8 安全。
  - `src/diag.rs`：`Diagnostic{severity,code,span,message}` + `Severity`（Error/Warning/Unsupported）——D2 诚实机制落地。
  - `src/parser.rs`：`parse(wikitext) -> Parsed{nodes, diagnostics}`。块级（空行分段 + 标题）+ inline 组装（粗斜体/链接配对，未闭合**降级为文本**不吞后文）。**最小诚实子集**：段落/标题/粗斜体/内链；其余（模板/表格/ref/列表/HTML/预格式）→ `Unsupported` + Diagnostic（保留原文 span，不硬解）。
- **Tests:** tokenizer ×1 + parser ×2（真 wikitext → AST → text 全对；unsupported 块出正确诊断码 `U-TEMPLATE`/`U-LIST`）。全量 **23 测试绿**，clippy `-D warnings` 干净。
- **Benchmark:** parser 暂不在 benched 路径（strip 仍 ~118 MiB/s）；接 `render::plain` 端到端后再进 bench 与 strip 对比。
- **Regression?** none。
- **里程碑:** AST 第一次能从真实 wikitext 长出来；诊断系统就位（范围外报警而非静默）。
- **下一步:** ① parser + `render::plain` 接进 CLI/bench 与 strip 对比；② 接 parserTests 的 `#[ignore]` conformance（让"X% 一致"出真实数）；③ 扩子集（列表/外链…），通过率随之爬。

---

## [2026-06-24] 加 Bliki 为第三对比基线 + DESIGN §12 前人分析

- **背景:** 用户问 wikrs vs "blick" → 实为 **Bliki 引擎**（`info.bliki.wiki`，XWiki MediaWiki Syntax 扩展的底层），mature 的 wikitext→HTML（含模板展开），上游已弃、只剩 XWiki fork。
- **Change:**
  - `docs/DESIGN.md` §12「前人与竞品」：WikiExtractor / parse_wiki_text / **Bliki** / wikitextparser / wikitextprocessor / Parsoid 对比表 + wikrs 三条差异化（Rust 速度 / 诚实诊断 / 新+活跃）。Bliki = "前人倒在活跃维护"的活样本。
  - `tools/bliki/`：`BlikiBench.java`（Bliki 渲染微基准）+ `setup.sh`（coursier 取 11 jar + javac）+ README；jar/编译产物 gitignore。
  - xtask `bench-bliki` 子命令；`docs/TESTING.md` Bliki 进对比基线表 + 命令（顺手刷新 parse_wiki_text/WikiExtractor 过期状态）。
- **Benchmark（新对比）:** 样例 article（wikitext→HTML）：**Bliki ~0.4 MB/s** vs wikrs strip ~118 MB/s → **约 300× 差距**。诚实标注：Bliki 做的多（全 HTML + 模板），非 apples-to-apples；但量级差距强力支撑速度论点。strip 本身不变 ~118 MiB/s。
- **Tests:** xtask clippy `-D warnings` 干净；`cargo xtask bench-bliki` 端到端跑通；wikrs 测试不受影响。
- **Regression?** none。
- **环境踩坑:** 有 JDK15、无 Maven/coursier；Bliki 模板路径缺类 → `TemplateParserError:NoClassDefFoundError`，用 coursier launcher 解全依赖（11 jar）后正常渲染（1476B→3722B HTML）。

---

## [2026-06-25] Stage 2 步骤 1：parserTests 真实覆盖率

- **Change:** `tests/parser_tests.rs::stage2_coverage_rate`——跑 1077 真实 case，报"**零诊断 = 完全支持**"百分比；floor>20% 防回退。README 记分牌加 Stage 2 coverage 行。
- **Coverage（新指标，比 Stage 1 那个 98% 宽松下限有意义得多）:** **24.8%（267/1077）** 在当前最小子集（段落/标题/粗斜体/内链）内零 `Unsupported`、完全支持。
- **Tests:** 新测试绿；`parser_tests` 4 passed + 1 ignored；clippy `-D warnings` 干净。
- **Benchmark:** 无 perf 改动；strip ~118 MiB/s 不变。
- **Regression?** none。
- **下一步（步骤 2）:** 扩子集（外链 / 列表 / `<ref>`…），看 24.8% 往上爬。

---

## [2026-06-25] Stage 2 步骤 2a：外链

- **Change:** tokenizer 加 `ExtOpen`/`ExtClose`（`[http://… …]`，scheme 检测）；parser `make_ext_link`（URL = 首个空格前，其余 = label；裸 url → 空 label，渲染为空，对齐 Stage 1）。复用 `Link` 节点。
- **Coverage:** **24.8%（267/1077）不变**——外链此前未被标 Unsupported（静默当文本漏过），所以这是**正确性 + 诚实性修复**（堵上"声称支持却 mangle"的洞），不是覆盖率增量。下一类（列表）才推高数字。
- **Tests:** tokenizer ext-link + parser `parses_external_links`；15 lib 测试绿，clippy 干净。
- **Benchmark:** strip ~118 MiB/s 不变。
- **Regression?** none。
- **下一步:** 列表（`*`/`#`/`:`/`;`）——会把覆盖率推上去。

---

## [2026-06-25] Stage 2 步骤 2b：列表（flat `*`/`#`）

- **Change:** AST 加 `List{ordered, items}`；`render::plain` 渲染列表（逐项一行）；parser `parse_list`——块内每行单 `*`/`#` → List；嵌套（`**`）/混合/定义（`:`/`;`）仍 → Unsupported（U-LIST）。`flags_unsupported` 测试改用嵌套列表保持 U-LIST 断言。
- **Coverage:** **24.8% → 27.1%（267→292，+25 例）**。简单列表从 Unsupported 变完全支持。
- **Tests:** parser `parses_simple_lists`（含嵌套仍 Unsupported）；16 lib 测试绿，clippy 干净。
- **Benchmark:** strip ~118 MiB/s 不变。
- **Regression?** none。
- **下一步:** `<ref>` / nowiki 等再扩；然后步骤 3（`render::plain` 接进 CLI/bench 与 strip 对比）。

---

## [2026-06-25] 同步 Stage 2 文档 + 测试

- **Change（文档）:**
  - `docs/stages/stage-2-ast.md` 重写：状态"未开工"→"进行中"，加「当前进度（实测）」（支持子集 / Unsupported 清单 / coverage 27.1% / 实际模块），checkpoint+task 标注 ✓/部分/未；记一条已知限制（紧贴正文无空行的标题暂识别不到）。
  - `docs/DESIGN.md` §5 模块树更新为实际布局（多数单文件 `.rs`，Stage 2 模块已建）。
  - `docs/TESTING.md` parserTests 行加「Stage 2 零诊断覆盖率 27.1%」。
- **Change（测试）:** 新增 `tests/parse_snapshots.rs`——Stage 2 AST 路线快照（`parse → render::plain` + 诊断码），干净演示支持构造渲染、范围外 → Unsupported 丢弃 + 诊断（U-TEMPLATE/U-TABLE）。
- **Tests:** 27 测试绿，clippy `-D warnings` 干净。
- **Benchmark:** 无 perf 改动；strip ~118 MiB/s 不变。
- **Regression?** none。
- **顺手发现:** 标题块切分依赖空行 → 紧贴正文的标题漏识别（已记进 stage 文档待修；也解释了真实文章 coverage 偏低）。

---

## [2026-06-25] Stage 2：修块切分（标题自成块）

- **Change:** `blocks()` 现在把 `== 标题 ==` 行也当块边界并自成一块（此前只按空行切，紧贴正文的标题漏识别）。抽出 `heading_parts` helper 共用。
- **Coverage:** **27.1% 不变**——这些 case 此前已零诊断（被当段落里的字面 `==` 文本，无诊断但语义错），所以是**正确性修复**（真实文章标题不再变字面文本），非覆盖率增量。再次印证 coverage ≠ 正确性。
- **Tests:** `isolates_headings_without_blank_lines`；17 lib 测试绿，clippy 干净。
- **Benchmark:** strip ~118 MiB/s 不变。
- **Regression?** none。
- **下一步:** `<ref>` / nowiki / 注释（会真正推高真实内容 coverage——含 ref 的整块现在被整块判 Unsupported）。

---

## [2026-06-25] Stage 2：`<ref>` / nowiki / 注释（inline 跨度）

- **Change:** tokenizer 加 `<` 跨度处理（`tag_span`）：`<ref>…</ref>`/`<ref/>` 与 `<!--…-->` **丢弃**，`<nowiki>…</nowiki>` **留内文**（字面，不再当 markup 解析）——全 borrow-friendly（内文是输入切片，无额外分配）。parser `has_tag` 收窄：ref/nowiki/comment 不再判 U-HTML（交给 tokenizer），其余 HTML 标签（`<div>` 等）仍 U-HTML。
- **Coverage:** **27.1% → 30.4%（292→327，+35 例）**。含 ref/nowiki/注释的块从整块 Unsupported → 完全支持。
- **Tests:** tokenizer 3 例（ref/comment 丢、nowiki 留）+ parser `handles_refs_nowiki_comments`（含 `<div>` 仍 U-HTML）；18 lib 测试绿，clippy 干净。
- **Benchmark:** strip ~118 MiB/s 不变。
- **Regression?** none。
- **下一步:** 步骤 3（`render::plain` 接进 CLI/bench 与 strip 对比）或表格。

---

## [2026-06-25] Stage 2 步骤 3：render::plain 接进 CLI + bench ⭐

- **Change:** CLI 加 `--engine strip|ast`（默认 strip）；`ast` 走 `parser::parse → render::plain`。bench 加 `wikrs_ast`（parse+render）。`tests/cli.rs::ast_engine_extracts_text`。
- **Benchmark（意外好结果）:** 样例上 **`wikrs_ast` ~276 MiB/s vs `wikrs_strip` ~118 MiB/s → AST 路线快 ~2.3×！** 原因：strip 是 5 趟分配；AST 路线 tokenizer/parser/render 借用友好（`Cow`），分配少。AST 还逼近 `parse_wiki_text`（~308），但**同时产出文本 + 诊断**（parse_wiki_text 只建借用 AST、不产文本）。
- **诚实标注:** strip 仍是 CLI 默认——AST 在模板重的真实块上还把整块判 Unsupported 丢掉（输出更稀），strip 把模板剥掉留周围散文。等 coverage 上去，AST 接管默认。
- **Tests:** cli ast 测试；全绿，clippy 干净。
- **Regression?** none。
- **里程碑:** Stage 2 AST 路线端到端在 CLI 跑通，且**比 strip 快**——borrow-friendly 设计兑现。
- **下一步:** 表格 / 更多子集；strip 优化已被 AST 超越（可考虑让 AST 早点接管默认）。

---

## [2026-06-25] Stage 2：HTML 标签分类（透明 / void / 结构）

- **数据驱动:** coverage 测试加诊断码直方图 → 拦路是 U-TEMPLATE(462) 和 U-HTML(334)。模板是 deferred 硬骨头；HTML 是可做的最高杠杆。
- **Change:** tokenizer 加 `tag_kind` 分类（Ref/Nowiki/Transparent/Void/Unsupported）。透明格式标签（`<b>`/`<i>`/`<span>`/`<code>`/`<sub>`… 约 30 种）**丢标签留内文**（内文继续 tokenize，markup 不丢）；void（`<br>`/`<hr>`）→ 空格；结构标签（`<div>`/`<table>`…）仍 U-HTML。parser `has_tag` 改用同一分类（保持一致）。
- **Coverage:** **30.4% → 36.0%（327→388，+61 例）**。U-HTML 334→248。
- **Tests:** tokenizer 3 例 + parser `keeps_inner_of_transparent_html_tags`；19 lib 测试绿，clippy 干净。
- **Benchmark:** strip ~118 MiB/s 不变（AST 路线 ~276）。
- **Regression?** none。
- **现状:** 剩余拦路 **U-TEMPLATE(462) 一家独大**（deferred）；其余 U-HTML(248，多为结构标签)/U-TABLE(60)/U-LIST(47)/U-PRE(22)。
- **下一步:** 表格 / 嵌套·定义列表 / pre；模板是终极硬骨头。

---

## [2026-06-25] Stage 2：定义列表（`;` / `:`）

- **Change:** `parse_list` 扩到接受 `*`/`#`/`:`/`;` 任意单层标记（含混合，如 `;term` + `:desc`）；嵌套（多字符标记）仍 Unsupported。
- **Coverage:** **36.0% → 36.8%（388→396，+8）**。U-LIST 47→38。
- **Tests:** `parses_simple_lists` 加定义列表断言；19 lib 绿，clippy 干净。
- **Benchmark:** strip ~118 MiB/s 不变。
- **Regression?** none。
- **下一步:** pre（22，易）/ 表格（60，复杂）。

---

## [2026-06-25] Stage 2：preformatted（前导空格块）

- **Change:** AST 加 `Preformatted(逐行 inline)`；render 逐行输出；parser `parse_pre`——全行前导空格 → 去一格缩进、逐行 inline-parse；含模板/表格/未支持标签的 pre 仍走诊断路径（保持诚实）。
- **Coverage:** **36.8% → 37.4%（396→403，+7）**。U-PRE 22→6（剩 6 个是含模板的 pre，正确仍 flagged）。
- **Tests:** `parses_preformatted_blocks`；20 lib 绿，clippy 干净。
- **Benchmark:** strip ~118 MiB/s 不变。
- **Regression?** none。
- **现状（直方图）:** **U-TEMPLATE(461) 一家独大**；其余 U-HTML(247，结构标签)/U-TABLE(58)/U-LIST(37，嵌套)/U-PRE(6)。简单杠杆基本榨干，剩下是模板（deferred 死结）和复杂的表格 / 嵌套列表。

---

## [2026-06-25] Stage 2：选项 A —— 内联模板"丢弃 + Warning"（不展开）⭐

- **战略前提（讨论后定）:** C（真展开）要重写 preprocessor + parser functions + Lua/Scribunto + 全套模板语料，且会把速度砸到 Bliki 那档（~0.4 MB/s，慢 2 个量级），正中交接文档警告的"淹死"陷阱。**选 A**：丢模板、留散文、发诚实 Warning。
- **Change:** tokenizer 加内联 `{{…}}` 丢弃（brace-match 嵌套）；diag 加 `Diagnostic::warning`（`Severity::Warning`）；parser：`{{` 不再判 Unsupported，非 Unsupported 块含 `{{` → 发 `W-TEMPLATE`；`strip_inline_templates` 让块分类不被模板内部的 `{|`/标签骗到（修 over-flag）。
- **输出（A 的核心价值）:** 含模板的真实块**现在提取出散文**而非整块丢。快照实证：`The planet {{convert}} has '''two''' moons.{{citation needed}}` → `The planet  has two moons.` + `W-TEMPLATE`。
- **Coverage:** 37.4% → **36.7%** —— 是**更诚实**：此前含模板的标题/列表块被当"零诊断完全支持"（却输出字面 `{{x}}`），现在诚实带 Warning → 不再虚高。
- **Benchmark:** `wikrs_ast` 276 → **180 MiB/s** —— 现在真在模板重的块上干活（提取散文）而非跳过；仍比 strip（119）快 ~1.5×，输出好得多。
- **Tests:** tokenizer 2 例 + `drops_inline_templates_with_warning` + 改 `flags_unsupported` + 更新快照；21 lib 绿，clippy 干净。
- **Regression?** none（行为有意改变，全测试覆盖）。
- **意义:** AST 引擎在真实文章上输出像样了，诊断模型多了 Warning 档（丢了什么一清二楚）；距"AST 接管 CLI 默认"更近。

---

## [2026-06-25] Stage 2：AST 接管 CLI 默认（strip 退居 `--engine strip`）⭐

- **Change:** `--engine` 默认 strip → **ast**。`render::plain` 的 `Unsupported` 块**回退到 strip**（best-effort 文本，散文不丢），诊断仍记录"没法结构化"。
- **意义:** **更好的引擎成了默认**——结构化能解的、strip 兜底没法解的、全程诚实诊断。AST 输出 ≥ strip 且更快。Stage 2 的 `render::plain` 正式开始取代 Stage 1 `strip`（交接文档的目标）。
- **Benchmark:** `wikrs_ast` ~174 MiB/s（含 Unsupported 块 strip 兜底的额外开销），仍比 strip（118）快 ~1.5×。
- **Tests:** `extracts_clean_text` 改测 `--engine strip`；cli + 全量绿，clippy 干净，快照不变（表格 strip → 空）。
- **Regression?** none（默认有意切换；strip 仍可 `--engine strip`）。
- **下一步:** 表格（把 cell 文本抠出来）/ 收口发布 / 嵌套列表。

---

## [2026-06-25] Stage 2：表格（简单表 → cell 文本）

- **Change:** AST 加 `Table{rows}`；render 逐行 cell（tab 分隔，行换行）；parser `parse_table`（`{|…|}` / `|-` 行 / `|`·`!` cell，`||`·`!!` 分隔）+ `cell_content`（按 MediaWiki 规则取"属性管道"——`[[`/`{{` 外第一个 `|`——后的内容）。**多行 cell / 非表格行 → None → Unsupported → strip 兜底**（不假装解析）。
- **Coverage:** **36.7% → 39.6%（395→427，+32）**。U-TABLE 100→54（简单表解了，复杂表仍诚实 Unsupported）。
- **输出:** 表格 cell 文本现在抠出来了（快照：表格 → `Property\tValue`，整篇文章只剩 `W-TEMPLATE` 一条诊断）。
- **Tests:** `parses_simple_tables`（属性丢弃、链接 pipe 不误判、多行 cell 仍 Unsupported）；22 lib 绿，clippy 干净，快照更新。
- **Benchmark:** strip ~118 MiB/s 不变。
- **Regression?** none。
- **现状（直方图）:** W-TEMPLATE(368，丢+警告) / U-HTML(301，结构标签) / U-TABLE(54，复杂表) / U-LIST(47，嵌套) / U-PRE(13)。

---

## [2026-06-26] backward-compat ratchet（覆盖率只能涨不能悄悄跌）

- **Change:** 加 backward-compatibility 棘轮。`tests/coverage_baseline.txt` 钉住当前 **427** 个"零诊断"干净通过的 case（**只存名字** —— 派生自 wikrs 的事实，非 GPL fixture 正文，可安全提交）。新增 `coverage_ratchet` 测试：旧 case 退化（曾干净、现在报诊断）→ **硬失败**并列出退化的 case 名；新 case 干净通过但未登记 → 失败并提示 bless。`BLESS_COVERAGE=1 cargo test --test parser_tests coverage_ratchet` 重新生成 baseline。
- **解决的问题:** 单一覆盖率百分比会**掩盖个案回退** —— 某次改动让 10 个新 case 过、同时弄坏 8 个旧 case，% 还在涨但已 break backward compatibility 且看不出来。棘轮把"过了哪些 test"变成 repo 里可审计的名单，每次变动都是一条经审阅的 baseline diff。
- **Tests:** TDD —— 先写纯逻辑单测 `ratchet_diff_reports_regressions_and_additions`（red：函数不存在 → green）；再接集成 `coverage_ratchet`。**planted-regression 实测**：往 baseline 塞一个假名字 → 测试如期 FAIL 并点名该 case → 再 bless 复原。全量绿（lib 22 + parser_tests 6/1 ignored + cli 3 + dump/snapshots/robustness），clippy `--all-targets` 干净。
- **Benchmark:** **仅测试代码改动，`src/` 引擎零改动 → 无性能影响。** 今日机器读数 ast 154–160 / strip 116 / parse_wiki_text 303–305 MiB/s，连跑两次全线同向 −2%/run（含我无法影响的外部 crate parse_wiki_text）→ 系机器负载噪声，非回退；引擎吞吐维持既有 **~174 MiB/s**。
- **Regression?** none（test-only；棘轮本身就是防回退的闸门）。
- **下一步:** 后续每次扩子集/改 parser，先 `coverage_ratchet` 把关，再 bless 把新通过的 case 登记进 baseline（连同 code 一起提交）。

---

## [2026-06-26] Stage 2：嵌套列表（U-LIST 直方图最大可治项之一）

- **Change:** `parse_list` 重写为深度感知的递归构建：每行的 leading marker run（`*#:;`）= 深度，更深的行嵌进上一条浅项。`build_list(lines, depth)` 递归；定义列表（`:`/`;`）仍折叠为无序（保文本，不拆 term/def）。**不规则嵌套**（块从中间深度起、或跳级）→ `None` → 保持 Unsupported，**不臆造缺失的父级**（D2）。render 加 `render_list_items`：嵌套子列表各占一行（`* a / ** b` → `a\nb`），不再把父项文本和子列表黏在一起。
- **Coverage:** **39.6% → 40.2%（427→433，+6 net）**；U-LIST **47→40**。ratchet 如期拦下并点名这 6 个新通过的 case（全是 `Nested lists *` 类，无误判），bless 登记进 baseline。
- **Tests:** TDD —— 先写 `parses_nested_lists`（well-formed 嵌套零诊断 + 结构 + render；混合 marker；不规则嵌套仍 U-LIST），red→green。改两处旧测试（`parses_simple_lists` 删掉"嵌套=Unsupported"旧断言；`flags_unsupported` 把样例块换成 `<div>`/U-HTML，因为嵌套列表已支持）。23 lib 绿、全量绿、clippy 干净、快照不变。
- **Benchmark（含一处诚实修正）:** 本次改动对 ast 路线 **持平**：`change:` −0.3% [−1.2%, +0.6%]，跨零 = 噪声内。但顺手发现 **README 的 ~174 MiB/s 是过期数**：本次 strip(~117)/parse_wiki_text(~305) 都在各自 idle 基线上（机器没负载），ast 实测稳定在 **~155 MiB/s**（连跑 4 次 153–160）。174 是 tables 提交之前的数——tables 把 `parse_table` 加进了 ast 路线却**只重测了 strip、没重测 ast**，于是我上一条 ratchet 还把 155 误判成"噪声"、继续carry 174。**已改正**：README ast 174→**155**、"~1.5× faster than strip"→"~1.3×"。
- **Regression?** 本次改动无回退（per-change 持平）。174→155 的差距**早于本次**、追溯到 tables 提交（ast 路线首次被诚实重测才暴露）——正是 skill 警告的"性能悄悄回退被过期数字掩盖"，现已纠正。
- **现状（直方图）:** W-TEMPLATE(369) / U-HTML(299) / U-TABLE(54) / U-LIST(40，仅剩不规则嵌套) / U-PRE(13)。下一个可治项：U-PRE(13) 或复杂表(54)。

---

## [2026-06-26] Stage 2：透明 HTML 容器标签（最大单次覆盖率跃升）⭐

- **先调查后动手:** dump 了 U-PRE/U-TABLE/U-HTML 三类。**U-PRE/U-TABLE 是死胡同**（误报噪声 + 模板 fostering 边角，0 净覆盖率，见 [[wikrs-upre-deadend]]）；**U-HTML 才是金矿**——247 例里 191 例是"纯 U-HTML、无模板"的干净候选，被 `<div>`(80)/`<center>`/`<blockquote>`/`<p>` 等卡住。
- **Change:** `tag_kind` 把表现型容器标签 `div`/`center`/`blockquote`/`p` 从 `Unsupported` 改判 `Transparent`（单一中枢——同时驱动 `has_tag` 的块级 U-HTML 标记和 tokenizer 的解包）。这些标签无文本语义，丢壳留文，跟 `<code>`/`<b>` 一个待遇。另把 transparent/void 的闭合 `>` 查找改为**引号感知**（`tag_close`）：属性值里的 `>`（如 `<div title="a>b">`）不再截断标签体。
- **Coverage:** **40.2% → 47.1%（433→507，+74）**，**单次最大跃升**。U-HTML 299→174。（U-LIST +7 / W-TEMPLATE +20：解包容器后，原本被 `<div>` 包住的列表/模板浮现出来 → 诚实，我们现在看得更深。）
- **诚实抽查（关键）:** 我把全部 74 个新通过 case 的 **输入→输出逐个看了**。~70 个干净抽文（`<div id=x>HTML rocks</div>`→`HTML rocks`、blockquote 段落、center 格式）。**~3 个是对抗性畸形 HTML**（T4304/T5244 属性安全、odd-caps nowiki）输出有点脏（漏属性尾巴），但**只是文本脏、没有伪造结构**——没越过 D2 红线（"绝不静默重塑成貌似对的结构"）。这是已知边界，记在此。
- **Tests:** TDD `keeps_inner_of_transparent_block_tags`（red→green）；改两个把 `<div>` 当"不支持样例"的旧测试 → 换 `<table>`（仍 Unsupported）。24 lib 绿、全量绿、clippy 干净、快照不变。
- **Benchmark:** ast **~159 MiB/s**（strip 119 / parse_wiki_text 310 都在 idle 基线），相对上次 ~155 **持平**（噪声内）。tokenizer 改动没拖慢——`<div>`/`<center>` 在样例文章里少见，`tag_close` 扫描 ≈ `find('>')` 成本。
- **Regression?** none。
- **现状（直方图）:** W-TEMPLATE(389) / U-HTML(174) / U-TABLE(54) / U-LIST(47) / U-PRE(14)。U-HTML 仍是第二大但已腰斩；剩下的多是 HTML 表格/列表/test 扩展标签 + 模板边角。

---

## [2026-06-26] Stage 2：转写控制标签 `<noinclude>`/`<onlyinclude>`（收尾 U-HTML 干净子集）

- **Change:** `tag_kind` 把 `noinclude`/`onlyinclude` 改判 `Transparent`——这俩的内容在**被渲染的页面本身**上是**显示**的（转写时才有别），所以丢壳留文是对的。**`includeonly` 故意留 Unsupported**：它要**隐藏**内容，而跨块的 includeonly（T8563"被 includeonly 抑制的小节"）在我们的逐块 tokenizer 下会**静默把本该隐藏的内容显示出来** = 违反 D2，所以诚实标记、不假装处理。
- **Coverage:** **47.1% → 47.8%（507→515，+8）**，全是 noinclude/onlyinclude 用例。
- **D2 by construction:** transparent 解包**不可能伪造结构**——它只丢标签、留文本，文本再走正常 parser；最坏是文本不完美（如 noinclude 块内的 `==x==` 被当字面文本而非标题渲染，1 例），但文本不丢、结构不假。所以这类改动天然安全。
- **Tests:** `keeps_inner_of_noinclude_onlyinclude`（red→green，并断言 includeonly 仍 U-HTML）。25 lib 绿、全量绿、clippy 干净。
- **Benchmark:** ast **~161 MiB/s**，`change:` −0.08%（跨零=噪声内）。加两个 match 分支零运行时成本，符合预期。
- **Regression?** none。
- **现状（直方图）:** W-TEMPLATE(391) / U-HTML(159) / U-TABLE(56) / U-LIST(49) / U-PRE(14)。**覆盖率 chipping 已近诚实天花板**：剩下 U-HTML 多是 HTML 表格/列表（黏连风险）/test 扩展标签 + includeonly + 模板边角；U-TABLE/U-LIST/U-PRE 基本是误报+模板 fostering。再往上要么改方向（差分"三个数字"），要么 block 分类器 span 感知。

---

## [2026-06-26] Stage 2：HTML 列表标签（收官 U-HTML 干净 chipping，破 49%）

- **Change:** `tag_kind` 把 `ul`/`ol`/`li`/`dl`/`dt`/`dd` 改判 `Transparent`。**靠源码里 item 之间本就有的换行**（语料里 HTML 列表都是 `</li>\n<li>`）分隔，不合成 bullet——跟 wiki 列表 plain 渲染一个路子（文本上行、无符号）。
- **Coverage:** **47.8% → 49.0%（515→528，+13）**。U-HTML 159→139。
- **诚实抽查（关键，怕黏连）:** 逐个看了全部 13 个新通过 case 的输入→输出。**全干净、零黏连**——每个 case 的 item 间都有源码换行 → "One\nTwo"；唯一的单行 case 只有一个 item（无从黏）。1 例（多重空行）只是间距脏，不是黏连/假结构。transparent 解包本就不会伪造结构（见上一条）。
- **Tests:** `keeps_inner_of_html_lists`（red→green）。26 lib 绿、全量绿、clippy 干净。
- **Benchmark:** ast **~159 MiB/s**，持平（又是纯 match 分支，零成本）。
- **Regression?** none。
- **本会话 U-HTML 战果:** 透明容器(div…) + 转写标签(noinclude…) + HTML 列表，U-HTML **299→139**，coverage **40.2%→49.0%**。**干净 chipping 到此为止**：剩下的 U-HTML(139) 是 HTML 表格/test 扩展标签/includeonly + 模板边角，U-TABLE/U-LIST/U-PRE 是误报+fostering——都不是干净可拿的。**~49% 是不展开模板前提下的诚实天花板。** 下一步应换方向：差分"三个数字"（声誉证据）或 block 分类器 span 感知。

---

## [2026-06-27] Stage 2：层 2 差分谐("三个数字")— wikrs 抽文 vs Parsoid，真实页面 ⭐

- **Change:** 新增 `wikrs::diff` 核心模块（零依赖：分词 shingle 归一化、`precision`/`coverage`、`classify`→三桶 `Faithful`/`Divergent`/`Reported`、`Report` 聚合）+ xtask 两命令：`diff-fetch`（curl 取 wikitext + Parsoid HTML，scraper 抽可见正文，缓存到 gitignore 的 `tests/diff/cache/`）、`diff-report`（离线，逐页 `parse`→`render::plain`→`classify`，出三个数字 + fidelity overlay）。标题列表 `tests/diff/titles.txt`（仅名字、可复现、入库；页面内容 CC-BY-SA 运行时拉取、不入库——同 parserTests 纪律）。
- **真实数据发现（关键）:** 18 篇 featured 文章上，**页级三桶坍缩成 0/0/100**——"Reported" 是页级的，任一越界构造（`{|` 表格 / `<math>` / gallery）就整页标记，而真实 featured 文章必含其一。页级桶**诚实但无信息量**。
- **真正的证据是 fidelity overlay（逐页、与桶无关）:** **mean precision 91.3% / coverage 48.7% / 13-of-18 faithful / 0 empty / 无 precision 离群**。precision 是**保守下限**——~9% 缺口是 `<math>`/实体/分词边界噪声，不是 wikrs garbling（聚类紧、零离群 = 没有静默重塑）。coverage ~49% = 模板展开内容**按设计丢弃**（D4 护城河可视化）。最低几页（Prime number 96 个 `<math>`、Euler 16 个）正是数学页，佐证缺口是归一化噪声而非错误。
- **决策（用户）:** README 头条用 **precision-led**（91/49/faithful/0-silent），页级 0/0/100 桶作为**透明度/诚实层**呈现（每页都标记越界构造 = 和 WikiExtractor 静默出错的对比点）。不改 `classify` 语义——`diff-report` 已同时打印两视图。
- **Tests:** TDD `wikrs::diff` 7 单测（red→green：三桶优先级、precision 子集/空=1.0、coverage 低不致 divergent、`Report` 百分比和=100）。离线集成 smoke `tests/diff_report.rs` 3 例（真 wikitext 走 parse→render→classify：clean prose=Faithful、HTML 表格=Reported、捏造句=Divergent），CI 无需网络。33 lib 绿、全量绿、clippy（`--workspace -D warnings`，含 xtask + scraper）干净、`coverage_ratchet` 不退。
- **Benchmark:** ast **~159 MiB/s**，持平（diff 模块不在 parse/strip 热路；本会话两次 bench 158.2 / 159.9 确认噪声内）。xtask 新增 `scraper` 仅 dev 依赖、不进发布物（`publish = false`）。
- **Regression?** none。
- **落点 & 复现:** `cargo xtask diff-fetch && cargo xtask diff-report`。样本小且 curated——**方法学证据，不是"N 万页"那一跑**。后续：扩样 + 更干净的 truth 归一化（剥 `<math>`/ref、解实体）能把 91% 这个保守数顶上去；block 分类器 span 感知可降页级 Reported 噪声。

---

## [2026-06-27] Stage 2：HTML 实体解码（差分挖出的抽取质量 bug）

- **差分驱动**：层 2 差分挖出 wikrs 把 markup 漏进纯文本输出——precision 缺口的大头是 `&nbsp;` **没解码**（`9.02&nbsp;AU` 输出成字面词 "nbsp"），外加 `<ref name=>` / `[[File:|thumb|alt=]]` / `<math>` 源码泄漏。本次先收最干净最普遍的一个：实体解码。（投机性的"改 truth 归一化"被证伪——剥 truth 只会**降** precision，真正的洞在 wikrs 输出侧。）
- **Change:** 新增 `src/entities.rs`（`decode`：命名实体 nbsp/amp/lt/eacute… + 数字 `&#NN;`/`&#xHH;`；未知/畸形留字面，`AT&T` 不动；`&nbsp;` 归一成普通空格）。接两处：`render::plain` 对最终输出**整体解码一次**；`extract::strip` = `decode(strip_raw)`。渲染器 Unsupported 回退改用不解码的 `strip_raw`，避免双重解码。
- **效果（差分，免重抓——只 wikrs 渲染变了）**：mean precision **91.3% → 91.9%**，faithful **13/18 → 16/18**（Saturn 89.5%→91.3% 等 3 页越过 90% 阈值——nbsp 是普遍因素）。剩余低分页（Prime number/Euler 数学、Mount Everest refs）是 `<math>`/`<ref>` 源码泄漏，列为后续。
- **Tests:** TDD `entities` 4 单测（命名/数字/未知畸形/无 `&` 借用快路）。37 lib 绿、全量绿、snapshot 不变（样例文章仅 1 个 `&nbsp;`，输出一致）、`coverage_ratchet` 不退（解码在渲染期，不动诊断）。
- **Benchmark:** ast **~152 MiB/s**。**justified regression −4~5%**（vs 本会话前 ~159）：实体解码对渲染输出多扫一遍。先前 per-Text-node 解码掉到 ~146（−9%），改成**末尾整体解码一次** + `strip_raw` 免双解码后回到 ~152。strip ~117 持平。
- **Regression?** justified：−4~5% 换正确的实体解码（+3 faithful 页、修掉普遍的 `&nbsp;` 泄漏），值。

---

## [2026-06-27] Stage 2：差分扩样 `diff-sample`（随机 ns0 → 更诚实的代表性数字）

- **Change:** 新增 `xtask diff-sample`（`list=random&rnnamespace=0&rnfilterredir=nonredirects` 抽 N 个随机 ns0 标题，去重、排序、写带 header 的 pin 文件；随机 API 无 seed → pin 结果保可复现）。xtask 加 `serde_json`（已在 workspace lock，无新传递依赖）。入库 `tests/diff/titles-random.txt`（25 篇随机，仅名字）。
- **关键发现（代表性 vs curated）:** 25 篇随机 ns0 vs 18 篇 featured：precision **82.2% vs 91.9%**、coverage **36.6% vs 48.9%**、faithful **8/25 vs 16/18**。更关键——**页级桶不再坍缩**：随机页 **40% 静默 structural-diff**（10 页，precision<90% 且**无诊断**）/ 52% reported / 8% faithful。featured 样本（必含越界构造 → 全 reported）**掩盖了静默发散**。根因：`[[File:|thumb|alt=]]` / `<ref>` / `<math>` markup 在简单页泄漏，但**不触发 Unsupported** → 落进 Y（静默）桶。这正是项目要清零的桶——差分再次证明价值。
- **Tests:** TDD `parse_random_titles` 2 单测（正常 / 坏结构报错）。`cargo test -p xtask` 绿、clippy（`--workspace -D warnings`）干净、全量绿。
- **Benchmark:** 无 perf 相关改动（纯 xtask），ast 持平 **~152 MiB/s**。
- **Regression?** none。
- **下一步（差分点名的真账）:** File/thumb/ref/math 源码泄漏是**静默 structural-diff** 的主因，且简单页无诊断——比覆盖率更该追的正确性工作。

---

## [2026-06-27] Stage 2：堵 File/Category 链接泄漏（静默 structural-diff 40%→4%）

- **差分点名 → 修**：随机样本 40% **静默** structural-diff（precision<90% 且无诊断）逼出真账。
- **先试 File（猜）**：`make_link` 把 `[[File:X|thumb|alt=Y]]` 的参数当 label 文本渲染（leak `thumbalt=Y`）。修：File:/Image: 整条丢弃（镜像 Stage-1 `internal_text`）。**但随机样本几乎没动**（82.2%→82.3%，silent 仍 40%）——别猜，要看 misses。
- **dissect 最低静默页（Lázaro Mazón Alonso 53.7% [D]）→ 真凶是 Category**：misses 几乎全是 `[[Category:…]]` 名字（"category living people"、"1959 births category"…）。wikrs 把 Category 当普通链接渲染出名字，但 Parsoid 放底部分类栏、不进正文 → 短 stub 上 Category 占输出一大半 → precision 崩；长 featured 页正文多、占比小，所以被掩盖。
- **Change:** `is_media_target`→`is_nonprose_target`，并入 `category`（File/Image/Category 都丢；`[[:Category:…]]` 前导冒号=空首段，自然保留为可见链接）。同步 Stage-1 `links.rs`。无分配版（`eq_ignore_ascii_case`，热路每链接一次）。新增 `diff-report` 桶标签（F/D/R）定位静默页。
- **效果:** 随机 25 页：precision **82.2%→92.3%**、**静默 structural-diff 40%→4%（10→1 页）**、faithful **8→20/25**。featured 18 页：precision **91.9%→93.2%**、faithful 16→17/18。
- **Tests:** `drops_nonprose_links`（File/Image/Category 丢、`[[:Category:]]` 留、普通链接留）+ Stage-1 category 断言。38 lib 绿、全量绿、clippy 干净、snapshot 不变、`coverage_ratchet` 不退（丢链接不改诊断）。
- **Benchmark:** ast **~159 MiB/s**，change **+3.6%**（vs 实体解码后的 ~152）——丢 File/Category 链接 = 下游更少文本要渲染，正好抵消实体解码的 −4%；本会话净持平 ~159。
- **Regression?** none（反而回 ~159）。
- **剩余:** `<math>` LaTeX / `<ref>` 源码泄漏（但触发 U-HTML → Reported，非静默）+ 表格 cell 顺序的 shingle 假差。

---

## [2026-06-27] Stage 2：差分扩样到 N=120（可信声誉证据）

- **Change:** `diff-sample --count 120` pin 120 个随机 ns0 标题（入库 `tests/diff/titles-random.txt`，替换原 25 页 demo），fetch 全部、report。25 页是方法学证明，120 页是证据。无代码改动。
- **N=120 诚实数字（比 25 页样本略低，符合预期——更大样本更代表）:** mean precision **88.6%**、coverage 32.2%、faithful **82/120（68%）**、empty 1。页级桶：faithful 43.3% / **静默 structural-diff 9.2%（11 页）** / reported 47.5%。
- **fixes 泛化得到验证:** entity/File/Category 修复通用——120 页上静默 structural-diff 仍只 9.2%（修前 25 页是 40%）。最低分页几乎全是 [R]（体育/统计表格页，cell 顺序 shingle 假差，但**诚实 flagged**），非静默。
- **dissect 最低静默页（List of ambassadors… 58.5% [D]）→ 残余静默主因是 `<ref>` 泄漏**：misses 满是 "ref name"/"ref ref"（大使列表每条目带 ref）。**纠正上一条 worklog 的假设**——`<ref>` 不止在 Reported 页泄漏；某些 list/table 页**不触发诊断却泄漏**（[D] 静默）。下一个 leak 目标（机制待查：跨 block 多行 ref / table-fallback）。
- **Benchmark:** 无 perf 相关改动，ast 持平 **~159 MiB/s**。
- **Regression?** none。
- **下一步:** `<ref>` 静默泄漏（同 Category 的 investigate→fix）；表格 cell 顺序是 metric 精化（非 wikrs bug）。

---

## [2026-06-27] Stage 2：表格多行 ref 防误解析 + 澄清"ref 泄漏"是测试脚手架假象

- **Change:** `parse_table` 遇到**跨行 `<ref>`**（内含多行 `{{cite}}`，其 `|` 续行会被误当表格 cell）就 bail（→ U-TABLE → strip 回退），诚实 flag 而非静默把 citation markup 当 cell 解析（D2）。`has_multiline_ref` 仅在 `{|` 块跑，热路无感。
- **关键澄清（纠错）:** 之前 dissect 出"残余静默 = `<ref>` 泄漏" **是测试脚手架假象**——我的 python misses 脚本把缓存 wikitext 包成 dump XML（转义 `&`/`<`/`>`），dump 往返腐蚀了 `<ref>`→`ref`、`&nbsp;`→`nbsp;`。直接 `parse()` 原始串（`cargo run --example`）输出 `Intro. End.` **干净无泄漏**；`diff-report` 也是直接 parse 原始 `.wikitext`，所以它的 9.2% 从来不是 ref 泄漏。Category/File 的发现仍真实（那些构造无 `<>&` 可腐蚀），唯独 ref 被脚手架骗了。
- **真实的 9.2% 静默 structural-diff:** 可靠 dissect（直接 parse + shingle）显示主因是**表格 cell 的 3-gram shingle 顺序假差**——相邻 cell（人名 + 下一 cell 的 `{{age in years}} years` 的 "years"）拼成跨 cell 3-gram，与 Parsoid 渲染顺序不符。同词不同邻接。**是 3-gram metric 在表格上的局限，不是 wikrs bug。**
- **Tests:** `table_with_multiline_ref_is_flagged_not_silently_mangled`（多行 cite ref → U-TABLE；lead prose 留、cite markup 不泄漏）。新增 1 测试，全量绿、clippy 干净、coverage 528 不变（fixture 无此 pattern）、ratchet 不退。
- **Benchmark:** ast **~158 MiB/s**，持平。
- **Regression?** none。
- **教训:** 别用 dump-包装脚手架测含 `<>&` 的构造——直接 parse 原始串。残余静默是 metric 假差（表格 cell 顺序），要真降它得精化 metric（按行词集 / 句级匹配 / 排除已 flag 的表格），是 diff harness 的事、非 wikrs。

---

## [2026-06-27] Stage 2：差分 metric order-robust 化（静默 structural-diff 9.2%→0%）⭐

- **承上:** 上一条查明残余 9.2% 静默是**表格 cell 的 3-gram shingle 顺序假差**（相邻 cell 拼成跨 cell 3-gram，与 Parsoid 网格顺序不符），非 wikrs bug。本次精化 metric 剥掉这层假差。
- **Change:** `wikrs::diff` 加 `word_precision`（order-independent：wikrs 输出的 **distinct 词**有多少在原文词集里）。`is_faithful` 改为 **shingle-precision ≥ 90% OR word-precision ≥ 97%**——短语匹配（严，抓捏造）或几乎全词命中（顺序无关，表格重排不算发散）。`diff-report` 加 word-precision 行。
- **效果（重测，免重抓——只 metric 变了）:** 随机 120 页 **静默 structural-diff 9.2%→0%（11→0 页）**、faithful **82→106/120（68%→88%）**、**word-precision 97.7%**（shingle 88.6%——差的 9pt 全是表格重排）。featured 18 页：word-precision **99.7%**、faithful **18/18**、0 静默。**0% 静默是头条**：120 真实随机页上 wikrs **从不静默输出原文没有的内容**，剩下都诚实 flag（Reported）。
- **诚实性（关键）:** word-precision OR 对**抽取器**安全——wikrs 不重排正文（按源序输出 text span），唯一重排是表格 cell 扁平化；prose 的捏造仍由 shingle-precision 抓。所以 0% 静默是真的，不是把 metric 放水。
- **Tests:** `word_precision_rescues_reordered_table_cells`（同词乱序→faithful）、`genuinely_different_words_stay_divergent`（异词→divergent）。9 diff 单测、全量绿、clippy 干净、smoke 不变。
- **Benchmark:** 无 perf 相关改动（diff metric 不在 parse/strip 热路），ast 持平 ~158 MiB/s。
- **Regression?** none。

---

## [2026-06-28] Stage 2：parser 健壮性——三个 O(n²) DoS 路径修复（默认引擎）

- **差分外的安全网（TESTING.md 层 3）:** `robustness.rs` 原来只测 `strip`（Stage 1）。给 AST `parse()`（CLI 默认引擎）补上：对抗输入不崩、深嵌套不爆栈、2MB 线性。
- **测试找出真 bug:** `parser_does_not_panic_on_adversarial_input` 跑了 **141 秒**——parser 在对抗输入上 O(n²)。逐 case 计时定位三处：
  1. **撇号 run**（`'`×N）：tokenizer 每位置重数整个 run → O(n²)。修：`.take(3)`（只需知道 ≥3/2/1）。
  2. **未闭合嵌套链接/强调**（`[[a|`×N）：`parse_inline` 每个未配对 opener 都重扫 closer → O(n²)。修：某 closer 变体一旦前方无 occurrence，后续同类 opener 直接 O(1) degrade。
  3. **未闭合模板**（`{{`×N）：tokenizer 每个 `{{` 都 `template_end` 重扫 → O(n²)。修：首个未闭合 `{{` 后置 `no_template`（放 `{{` 分支条件**末尾**，非热路每字符）。
- **效果:** robustness **141s → 0.30s**，三处全线性，加 `assert(<30s)` 防回归。
- **Tests:** `parser_does_not_panic_on_adversarial_input`（13 对抗 case + 30s 上限）、`parser_survives_deeply_nested_links`（5万深不爆栈）、`parser_stays_linear_on_2mb_input`。41 lib 绿、全量绿、clippy 干净、`coverage_ratchet` 不退（纯提速、输出不变）。
- **Benchmark（已确认 ~10%，冷机 A/B）:** 用**未改动的 `strip` 当热控**（af0c5f0 不碰 strip），同机三组配置看 strip-归一化 `ast÷strip`：HEAD~1（改前）=**1.272** → 只换 tokenizer=**1.169** → HEAD（全改）=**1.110**，严格单调。两次单变量切换都指向「新代码更慢」，幅度可叠加（tokenizer ~8% + parser ~5% ≈ **~10%，区间 8–13%**）。这种按「含多少新代码」单调排序不是噪声造得出的——代价真实。绝对值：ast **~135 MiB/s**（strip ~121 的健康态下）。
- **机理（非算法）:** 仍是线性，代价来自 **codegen/寄存器压力**——热路 `inline()` 里 loop-carried 的 `no_template`、`parse_inline` 的 4 个 closer flag，编译器要全程保活，即便本输入上它们从不触发。先前怀疑是噪声/单一文件，A/B 证伪：两文件都贡献。
- **决策（D4）:** **接受**。DoS 安全是正确性/安全属性（非 span-awareness 那种边际打磨），换默认引擎消除 3 个 O(n²) 值这 ~10%；用户拍板「接受代价直接 push」。可选后续（冷机上做）：把 loop-carried flag 改成**循环不变量预判**（如 `inline()` 用一次性 `has_close`）试图抢回，但本会话噪声机上测不准，未做。

---

## [2026-06-28] Stage 2：修多行模板碎裂泄漏（brace-aware 分块器）

- **问题（差分发现）:** 代表性随机样本（120 随机 ns0 页）上，**10/120 页把原始 `{{…}}` 模板标记漏进输出**（干净输出永不含 `{{`），把精度拖到最低 **6.4%**（`2010-11 Maltese…Knock-Out`）。而且**伪装成 U-TABLE**：25 个 U-TABLE 页里 8 个根本没有真 `{|`，差点把我们误导去建表格解析器——precision-led 测量（`examples/diag_tally.rs` + `show_page.rs`，直读 `parse()`）抓住真凶。
- **根因:** `blocks()` 按空行切块，**不认 `{{`**。含空行的多行模板（大 infobox、`{{#invoke:…}}`）被空行切碎：首碎片是未闭合 `{{`，后续碎片是 `|param=` 行。`strip_inline_templates`（分类用）和 `strip_raw`（render 对 `Node::Unsupported` 的回退）都能处理**整段**模板，但都剥不掉未闭合的 `{{` → 首碎片误判 U-TABLE → `Node::Unsupported` → render 漏出模板体。
- **修复:** `blocks()` 加 brace 深度（新 `update_brace_depth`，**按序**扫 `{{`/`}}`，与 `template_end` 同逻辑），空行/标题行只在深度 0 时断块 → 多行模板保持单块、被干净剥除。只动 `src/parser.rs` 一个函数，`render.rs` 不动，保留 borrow/span 模型、线性。spec/plan：`docs/superpowers/{specs,plans}/2026-06-28-template-fragmentation-leak*`。
- **Tests（TDD 先红后绿）:** `blocks_keeps_multiline_template_whole`、`blocks_still_splits_normal_paragraphs`、`multiline_template_is_dropped_not_leaked`；robustness 加 `{{\n\n`×50k 守 brace 路径（线性、0.33s）。44 lib 绿、全量 9 target 绿、clippy 干净。
- **差分验收（120 随机，修前→修后）:** 泄漏 **10/120 → 0/120**；精度 **88.5% → 91.0%**；word-precision **97.7% → 99.3%**；fully-faithful **106 → 115/120**；faithful 桶 52.5% → 57.5%；**0% silent 保持**；coverage 32.2%→32.0%（模板封顶不动，符合预期）。最低精度页 Maltese（6.4%）跌出榜尾，INS_Prachand 20%→44.4% 翻成 [F]。
- **Benchmark（D4 闸）:** `wikrs_ast` **134.01 MiB/s**（strip 热控 125.11，健康；criterion δ 跨零=无变化）。brace 计数是每行一次的廉价扫描，零回归。
- **顺带（README 纠偏）:** 吞吐表更新为 af0c5f0 后的真实值（ast ~152→~134、~1.3×→~1.1×；af0c5f0 的 ~10% 回归当时漏更新 README），随机差分数字更新到修后值。

---

## [2026-06-28] Stage 2：`{|` 表格抽取（精准子集 + grid-bail 保 0% silent）

- **目标:** 抽取能干净解析的 `{|` 表格、缩小 U-TABLE bail。spec/plan：`docs/superpowers/{specs,plans}/2026-06-28-table-extraction*`。
- **组件 1 — `blocks()` 解黏表格:** 新 `update_table_depth`；top-level `{|` 自成块、累积到配对 `|}`（含内部空行），把表格从前后 prose 解黏。`{|` 检测用首字节 guard（`{`/空格/tab）避开正常 prose 行的 `trim_start`。
- **组件 2 — `parse_table` 鲁棒化:** (a) `table_logical_lines`：换行在 `<ref>…</ref>` 内不断行 → 多行 cite 不再碎裂成假 cell，删 `has_multiline_ref` bail；(b) header 行 `!!` 和 `||` 都切（之前只切 `!!`，混用时尾部 `||` 漏成文本）；(c) 表内空行跳过（解黏后空行会进表块）。
- **差分抓 bug（precision-led 又赢一次）:** 首轮验收 diff 抓到 **1 个 silent 页 + 2 个 `||` 泄漏**。`||` 泄漏 = header 混用 `!!`/`||`（修 b）；silent = `Ryley_Music`，但 word-diff 证明它**已验证 faithful**（仅 4 个 token `LGY 21/26/28`，Parsoid 对该网格 cell 渲染不同）——是 dense-grid 的度量边界，不是 garbling。
- **决策（D2，用户拍板）:** 加 **colspan/rowspan grid-bail**——跨格网格无法忠实摊平成 rows×cells，诚实 bail（U-TABLE）而非输出貌似合理却静默偏离的表，保住「0% silent」招牌。代价：bail 偏钝（连带让 ~12 个本可抽的表也 bail）。
- **Tests（TDD）:** 6 个新/改表格+blocks 测试（解黏、内部空行、`!!`/`||` 不泄漏、grid-bail、多行 ref 解析、重写旧 flag 测试）；robustness 加 `{|\n| x\n`×50k（线性）。修了一个 ratchet 回归（T85627 缩进表内空行 → `parse_table` 跳过空行）；ratchet **+1**（`Indented block & table` 现在干净解析，已 bless）。49 lib 绿、全量 9 target 绿、clippy/fmt 干净。
- **差分验收（120 随机，本特性前→后）:** U-TABLE **25→18 页**；coverage 32.0%→31.8%（持平，grid-bail 抵消增益）；精度 91.0%→**91.3%**；word 99.3%、**0% silent 保持**、**0 表格标记泄漏**；fully-faithful 115/120。
- **Benchmark（D4）:** `wikrs_ast` **~129 MiB/s**（strip 热控 ~123，机器偏热；与 ~134 冷基线在噪声内——表格代码对非表格行近零成本，trim 优化未改变 strip-归一化比值，证实非 per-line trim 成本）。无确认回归。
- **诚实结论:** 净收益不大（~7 个非网格表 un-flag、coverage 持平），但 precision-led 流程拦住了 silent 回归、grid-bail 保住了招牌；表格抽取的有界性又一次被证实。简单表抽、网格表诚实 flag。

---

## [2026-06-30] Stage 2：真实 dump 端到端验证（simplewiki 281,799 页）⭐

- **为什么:** 抽取质量在 120 页 curated 样本上爬到头（0% silent / 99.3% word-precision），diff 井抽干。两个卖点里「快」的证据**全建立在合成 dump**（同一篇文章 ×5000）上——README 自己标注「real heterogeneous dumps will differ」。本次拉真实 simplewiki dump（2026-06 快照，349 MB bz2 → 1.67 GB / 1.59 GiB XML，281,799 篇 ns0 文章）端到端验证速度与转化率。**纯测量，无 src/xtask 代码改动**（复用现成 `--stats` / `bench-compare`）。
- **速度（验证且强化）:** 单核端到端 **~150 MiB/s**（144–158，真实异构 XML），10 核 **~380 MiB/s**（368–388）。vs WikiExtractor（全量、同 `bench-compare` harness、同 `-o -` 单流）：wikrs **322 MB/s** vs WikiExtractor **10.2 MB/s = 31.7×**——比合成 22× 还高。**合成 22× 是保守不是虚高**：小输入被 wikrs 进程启动开销主导（16 MB 切片只 5.2×、8 MB 合成 47 MB/s 同理），全量 amortize 后真吞吐才显出来。
- **转化率（真实 = 91.9%，新信号）:** `--stats` 全量 281,799 页 **91.9% clean**（输出零残留 `{{}}`/`[[]]`/`{||}`），vs 合成 parserTests 98.1%。真实文章更脏，~8% leak tail 是 120 页样本从没暴露的真账。
- **leak tail 已定性（下一步目标）:** 逐页 tally——**`]]` 6.7%（18,923 页，绝对主因）**、`|}` 1.3%、其余全 <0.3%。`[[`/`]]` 不对称（0.3% vs 6.7%）点名是**链接闭合 bug 而非泛解析失败**。例页（April / Art / Air / Alan Turing）全是 **File/图片 caption 尾巴带闭合 `]]` 泄漏**——疑似 `[[File:…|thumb|…[[嵌套 wikilink]]…]]`，drop 逻辑匹配到**内层** `]]`，把 caption 尾 + 外层 `]]` 漏出。真实数据上的 #1 正确性 bug，bounded 可修。
- **Tests:** 无（纯测量、零代码改动）；现有全量测试未触动仍绿。
- **Benchmark:** criterion 引擎微基准不变（~134 MiB/s，未改代码）；本条新增的是**真实 dump 端到端**数字（上面），与单文件微基准互补。
- **Regression?** none（无代码改动）。
- **落点 & 复现:** dump gitignored 于 `target/realdump/`；`cargo xtask bench-compare target/realdump/simplewiki-articles.xml`、`./target/release/wikrs --input … --stats`。README「Benchmarks & test status」吞吐/转化率段已更新为真实 dump 数字。
- **下一步（真账点名）:** 修 `]]` File-caption 嵌套泄漏（TDD：red = 嵌套 wikilink 的 File caption 漏 `]]`；green 后重跑 `--stats` 看 91.9% 往上跳）——真实数据挖出的、影响 6.7% 页的 #1 目标。可选：上 enwiki 切片做更硬头条；差分从 dump 自身抽样。

---

## [2026-06-30] Stage 2：修 `]]` File-caption 嵌套泄漏（真实转化率 91.9%→97.9%）⭐

- **承上（真实 dump 点名的 #1 bug）:** simplewiki 全量 `--stats` 挖出 ~8% 页泄漏残留标记，主因 `]]` 占 6.7%（18,923 页）。dissect（`examples/show_page`，直读 `parse`）定位真凶——File/图片 caption 内嵌 `[[wikilink]]`（如 Air 页 `[[File:Fan.jpg|thumb|A [[wikt:fan|fan]] moves air.]]`）。
- **根因（systematic-debugging 定位到码级）:** `parse_inline` 的 `[[` 用 `find` 扁平匹配**第一个** `]]`——对普通链接对，但 File caption 内嵌 `[[…]]` 时，外层 File 链接被内层 `]]` 提前闭合：target 仍是 File→drop，但 `i` 跳到内层 `]]` 之后，caption 尾（" moves air."）当正文输出、外层 `]]` 落入 stray-closer 分支→字面泄漏。**0 诊断 = 静默泄漏**。
- **修复（scoped，保线性）:** 只给**非正文** media/category 链接（File/Image/Category）做深度匹配闭合——`link_close_matches`（一次 O(n) 栈扫，给每个 `[[` 配深度匹配的 `]]`）+ `link_target_is_nonprose`（peek 首 token 的 ns）。命中则把闭合位延到深度匹配的外层 `]]`，交由 `make_link` 整条 drop。**普通链接仍走扁平首闭**（深层嵌套递归保持浅、线性——不动既有 `parser_survives_deeply_nested_links` 的 prose 深嵌套）。深度表**仅在块含 `[[` 时才建**，无链接散文零开销。
- **Tests（TDD 先红后绿）:** `drops_media_link_with_nested_caption_link`（嵌套 caption→空、Body 保留、两层嵌套、`]]` 不泄漏）。robustness 加两条对抗流（`[[File:a|[[x]]`×50k 不平衡 + `[[File:a|[[x]] cap]]`×50k 平衡）守深度表线性。50 lib 绿、全量 9 target 绿、robustness **5.07s**（<30s，线性无 O(n²)）、clippy/fmt 干净、`coverage_ratchet` 不退。
- **真实验收（simplewiki 全量 281,799 页，修前→修后）:** clean **91.9%→97.9%**（+6.0pt）；`]]` 泄漏 **6.7%→0.66%**（18,923→1,862 页，−90%）。残留 0.66% 是诚实底噪（畸形源等），非本 bug。`|}`（1.29%）现为最大残留（表格闭合，下一目标）。
- **Benchmark（D4 闸）:** `wikrs_ast` **无变化**（criterion vs `before` baseline：change p=0.15>0.05，带 −1.1%/+2.3%/+5.1% 噪声内；"No change in performance detected"）。深度表按块建 + 无链接块 gate 吸收成本。cold 基线 ~134 MiB/s 持平。
- **Regression?** none（perf 噪声内、DoS 线性守住、ratchet 不退）。
- **下一步:** `|}` 表格闭合泄漏（1.29%，现最大残留）；可选 enwiki 切片做更硬头条。

---

## [2026-06-30] Stage 2：表格深度计数 brace-aware（`{{…|}}` 不再碎裂表格）

- **承上（`|}` 残留 dissect）:** `]]` 修完后 `|}` 成最大残留（1.29%）。dissect 最低页 Inch：表格 cell 含 `{{frac|1|12|}}`，其 `|}}` 里的字节 `|}` 被 **brace-盲** 的 `update_table_depth` 当成表格闭合 → 表格行中碎裂、内容当正文泄漏、真 `|}` 落单泄漏（0 诊断=静默）。
- **根因:** `update_table_depth` 只数 `{|`/`|}` 字节对，不认 `{{…}}`——模板里的 `|}` 假闭合表格。是**模板碎裂**问题，非表格解析器缺陷（正合 [[wikrs-utable-is-mostly-template-leak]]）。
- **修复（合并单扫 + brace-aware）:** `update_table_depth` + `update_brace_depth` 两次扫描合并成一个 `update_table_brace`——单趟左→右，同跟 `{{`/`}}` 和 `{|`/`|}`，**表格标记只在 brace 深度 0 计**；模板里的 `|}`/`{|` 不再假开/闭表格。brace 状态跨行。删掉旧 `update_table_depth`（无用）。
- **Tests（TDD 先红后绿）:** `table_cell_template_with_pipe_brace_stays_one_table`（`{{frac|1|12|}}` cell 不碎表、不泄漏 `|}`/`{{`/`||`）。51 lib 绿、全量 9 target 绿、robustness 线性、clippy/fmt 干净、`coverage_ratchet` 不退。
- **真实验收（simplewiki 全量，修前→修后）:** clean **97.9%→98.0%**（+0.1pt）；`|}` **1.29%→1.18%**（~307 页）。**收益小且诚实**：`|}` 残留 89%（3338 中 2966 页）**同时泄漏 `||`** = 整张原始表泄漏，多是 colspan/rowspan **grid 表**（按 D2 grid-bail 有意 flag，非本 bug）。`{{…|}}` 只是一小类。
- **为何仍值得:** 修的是**正确的机制**（模板 brace-awareness），零回归、顺手把两次扫描合一（略快、码更简）；`{{frac}}`/`{{convert}}` 在 enwiki 表格里密得多，那边收益应更大。
- **Benchmark（D4 闸）:** `wikrs_ast` **无变化**（criterion vs `before`：change p=0.66>0.05，−0.77% 点估计噪声内）。~134 MiB/s 持平。
- **Regression?** none。
- **决策（停）:** `|}` 残留主体是 grid 表（有意 bail），**不追**——正合 [[wikrs-utable-is-mostly-template-leak]]「修模板消费，别建表格解析器」。extraction-quality 清晰收官：`]]`（+6pt）真赢、`{{…|}}`（+0.1pt）正确小补、grid 表是有意的诚实 flag。

---

## [2026-06-30] 发布前 review 回应：文档诚实化 + 包内容清理（无代码改动）

- **背景:** 用户跑了一轮发布前 audit（publish 闸、包内容、CLI 行为、bench 漂移、文档数字）。逐条对码验证后修正如下；两条 pushback 见后。
- **P0 包污染（修）:** `.agents/`（本会话 harness 生成的 skill 脚手架）出现在 `cargo package --list`。加入 `.gitignore`；验证包清单已干净。
- **P1 bench 漂移（文档修 + 归因纠正）:** README 表仍写 ast ~134 MiB/s 且"faster than strip"；冷复测 ast **~119** / strip **~122** / parse_wiki_text ~306（参照系稳定 → 非机器负载）。**归因：af0c5f0 记录过的 −10% 从未落到 README 表**（README 正文自己写了 −10% trade、表格却没改，自相矛盾）；`before` baseline 在最近两个 commit **之前**就是 ~119，两个 commit 各自 criterion "No change"——**非近期回归**。表改 ~119/~122/~306，措辞改"≈ strip throughput"。
- **P2 数字统一（修）:** ratchet 基线实际 **529** 例（533 行 − 4 注释），README 528→529、49.0%→49.1%；CHANGELOG ~40%→~49%、"1.5× faster than strip"→"≈ strip"；TESTING.md 陈旧的"~82%/40% silent 是下一目标"更新为已修后的 99.3%/0%；顺手修自己上一 commit 留下的 97.9/98.0 不一致。
- **Pushback（两条，已说明理由）:** (1) `publish = false` **保留**——Amazon IP 闸未清，这是防误发的正确保险，翻转是真发布时刻的最后一步；(2) bench 漂移非最近提交所致（证据同上）。
- **Tests:** 无代码改动；`cargo package --allow-dirty --list` 复核干净。
- **Benchmark:** ast ~119 MiB/s（本次冷复测，即 README 新值）。
- **Regression?** none（纯文档/打包卫生）。
- **遗留（已确认、转入后续 commit）:** CLI `filter_map(Result::ok)` 静默吞 dump 错误；CLI 全量 collect 非 constant-memory（实测 max RSS **1.93 GB** on 1.67 GB dump）。均已复现，按用户拍板走"真修"。
