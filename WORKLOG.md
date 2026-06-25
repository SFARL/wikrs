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
