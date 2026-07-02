> *Internal dev-history document (Chinese). For English, see [DESIGN.md](DESIGN.md) and [TESTING.md](TESTING.md).*

# 项目交接：Rust wikitext 提取/解析引擎

> 冷启动交接文档。决策链和结论都已压入，换环境后无需重新解释来龙去脉。

## 一句话定位

用 Rust 写一个 wikitext 处理工具：**保底是"又快一个量级的 WikiExtractor"**（几乎确定能做到，靠 Rust 的速度），**上行是"又快又准、保留结构、对病态输入报警的现代 wikitext 引擎"**（声誉项目，靠解析质量）。目标是开源赚声誉，不是直接变现；变现是声誉之后的可选项。

## 为什么是这个（决策链）

- 反复验证过：凡"现有成熟软件 + 干净 benchmark"的方向都已被 Rust 化（HTML 提取、chunking、SQL parser 全是红海）。所以不进红海。
- 真空白只存在于"有需求但实现烂到没人愿意碰"的地方。wikitext 处理正是：需求真实高频（每个用 Wikipedia 做训练/RAG 的人都要剥文本），但实现是传奇级屎山，劝退了二十年的人。
- thesis：AI 让"重写难代码"的成本第一次降下来，而"难"正是这块的护城河来源。

## 关键技术事实（必须记住）

1. **大模型怎么读 Wikipedia**：不是拿 HTML。拿 Wikimedia 官方 **XML dump**（dumps.wikimedia.org），正文是 wikitext，再用工具剥成 plain text。事实标准工具是 **WikiExtractor**（Python，慢，正则硬剥，遇复杂模板/表格丢信息且静默出错）。这就是对标对象和它的弱点。

2. **wikitext 无法被干净解析，这是结构性死结，不是工程努力能消除的**：
   - 唯一完整规范就是 MediaWiki 那 6200 行 PHP 正则屎山。追求 100% 兼容 = 复刻它所有 bug = 死结，会烂尾。**不要追求和 MediaWiki byte-level 一致。**
   - 死结根源：模板系统是文本宏处理器（类似 C 预处理器），模板展开**不保证产出自包含 DOM**（有模板只吐 `<table>` 开始标签、或一个 `<tr>`）。所以"先解析后展开"和"先展开后解析"都不成立，两者纠缠。
   - 连官方 Parsoid（全职团队 + 十几年）都没用干净单趟架构吃下模板，最后退回去调 PHP preprocessor。

3. **前人都停在哪**：
   - `parse_wiki_text`（最认真的民间 Rust 尝试，0.1.5，2018，已停更）：主动划界，假设"模板从不改变周围格式"，本质是只读提取器，不是引擎。
   - `wikitext-parser`、`mediawiki_parser`：都是半成品，只解析子集，对错误输入大多解析错。
   - **结论：Rust 生态有一堆解析器碎片，没有一个活跃、正确、好用的。空白是真的。**

## 正确的产品边界（核心战略）

**不复刻 MediaWiki 的 legacy bug。** 改为："在一个诚实声明的支持范围内做到高正确率 + 范围外明确报错（而非静默出错）"。这个"诚实划界"本身就是最强的技术叙事——证明你懂这个坑的深度，不是又一个天真以为能重写 MediaWiki 的人。

## 分层交付路线（下行有底，上行有空间）

- **第一阶段 / 保底档（先做这个发布）**：wikitext → 干净 plain text。即"Rust 版 WikiExtractor"。卖点是**速度**（处理全量英文 dump 从几小时 → 几十分钟，可 benchmark，语言级优势几乎确定能拿到）。即使解析质量只和 WikiExtractor 打平，光速度就有人用、能拿 star。**先把这层做出来拿第一波反馈，再往上叠。**
- **第二阶段 / 进阶档**：产出结构化 AST，保留表格/链接锚文本/结构，对处理不了的怪异输入报警而非静默丢弃。这才是真正的声誉项目。
- **第三阶段（可选）**：AST → HTML 渲染。

## 测试策略（项目命门，第一周就要搭）

目标不是"逼近 MediaWiki 100% 一致"（死结），是"声明范围内高一致 + 范围外报错"。四层：

1. **地基：MediaWiki 官方 `parserTests.txt`** —— 几千条"wikitext → 期望 HTML"配对，机器可读，公开。直接当测试集 + 当"支持范围清单"。能过的 = 支持范围，过不了的 = 明确声明不支持。
2. **规模验证：真实 dump 差分测试** —— 同批页面，你的输出 vs ground truth（本地 MediaWiki/Parsoid，或调 Wikipedia REST API 拿官方 HTML），做**结构化 DOM diff**（归一化后比结构和文本，忽略无意义格式差异）。产出"X% 完全一致 / Y% 结构差异 / Z% 主动报错"——**这三个数字是 README 里最有说服力的声誉证据。**
3. **安全网：fuzzing**（`cargo-fuzz`）—— 喂畸形 wikitext，确保不崩溃/不死循环/不爆内存（对标 MediaWiki 要求：2MB 恶意输入，最坏执行时间线性而非平方）。这也是 Rust vs Python/PHP 的安全故事。
4. **回归保护：snapshot 测试**（`insta`）—— 防止改坏已经对的东西。

核心卖点公式：**"在英文维基 N 万随机页面上 X% 结构一致" + 对剩下不一致的清醒解释**。

## 命名（待定，需先查占用）

候选：

- **`wikrs`**（首选）—— wiki + rs 后缀，一眼是 Rust wiki 工具，不锁死在提取或解析任一层，有成长空间。
- **`mwx` / `mwparser`** —— mw=MediaWiki，精准命中圈内人搜索。
- **`unwiki`** —— "拆掉 wiki 包装"，有性格有记忆点。

> **动手前先去 crates.io + GitHub + 域名查这几个名字是否被占。**

## 第一步施工建议

做"保底档 MVP"：

1. 定义输入（Wikimedia XML dump / 单页 wikitext）
2. → 输出（干净 plain text）
3. → 对标 WikiExtractor 的具体行为（剥哪些、保留哪些）
4. → 设计第一个 benchmark 让"快一个量级"立住（同一个 dump，你 vs WikiExtractor，wall-clock + 吞吐 MB/s + 内存）。

## 诚实提醒

- 别一开始就奔完整引擎去，会淹死你，变负声誉墓碑。先发布"又快的文本提取"，验证有人用，再叠结构化。
- 保底价值靠"Rust 比 Python 快"（确定），不靠"我解析得多对"（有风险）。所以解析这块比预期难时，速度这维兜得住底。
- 这个项目的护城河就是"难到劝退所有人"——谁啃下"正确 + 活跃维护"谁赢，前人恰恰倒在这三个词上。你得是看到 6200 行多趟解析会兴奋而非退缩的人。
