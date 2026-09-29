# Codex CLI Claude 风格状态栏设计

日期：2026-06-29
状态：待用户最终审阅

## 目标

为 Codex CLI 提供 Claude Code 风格的彩色状态栏，同时保留 Codex 原生数据能力。状态栏应：

- 清晰展示模型、推理等级、当前目录和 Git 分支。
- 展示上下文使用率及字符进度条。
- 展示当前会话输入、输出 Token。
- 展示 Daily、Weekly、Monthly、Total Token 累计量。
- 展示 5 小时和每周额度使用率。
- 在宽窗口中使用两行布局，在窄窗口中自适应为三行且不丢失指标。
- 不阻塞 Codex 输入、输出或工具执行。

## 非目标

- 不修改 Codex 的模型调用、审批、沙箱或网络行为。
- 不读取或索引会话正文。
- 不在第一版实现任意外部状态栏命令协议。
- 不要求 tmux、zellij 或其他终端复用器。
- 不向官方 npm 安装目录直接写入定制文件。

## 用户体验

### 宽窗口：两行

适用于终端宽度不小于 140 列。

```text
GPT-5.4 · high │ ~/developer/codex │ main
Ctx 31% ▓▓▓░░░░░░░ │ Session ↑842K ↓96K │ Today 1.2M Week 4.8M Month 18.6M Total 42.1M │ 5h 18% · Limit/week 42%
```

### 窄窗口：三行

适用于 80–139 列。目录先缩写 HOME 和中间父目录，然后换行；所有指标继续展示。

```text
GPT-5.4 · high │ ~/d/codex │ main
Ctx 31% ▓▓▓░░░░░░░ │ Session ↑842K ↓96K │ 5h 18% · Week limit 42%
Today 1.2M │ Week 4.8M │ Month 18.6M │ Total 42.1M
```

低于 80 列时沿用三行结构并安全裁剪单个超长字段；不得产生 panic 或破坏输入区布局。

### 颜色语义

| 内容 | 颜色 | 目的 |
| --- | --- | --- |
| 模型、Git 分支、Daily | 绿色 | 身份及正常状态 |
| 当前目录、Monthly | 青色 | 位置及长期统计 |
| 上下文及进度条 | 紫色 | 上下文容量 |
| 会话输入 | 蓝色 | 输入流量 |
| 会话输出、Weekly | 黄色 | 输出及中期统计 |
| Total 或达到告警阈值的指标 | 红色 | 累计量或风险提示 |
| 分隔符、额度说明 | 灰色 | 降低次要信息权重 |

颜色通过 Ratatui `Span` 和 Codex 已有调色板渲染，不直接向终端拼接 ANSI 转义序列。无色终端应自然降级为可读文本。

## 架构

### 代码基线

- 从官方 `openai/codex` 仓库建立本地定制分支。
- 初始基线与当前安装的 Codex CLI `0.142.3` 对齐。
- 源码目录为 `~/developer/codex-custom`。
- 定制改动限制在 TUI 状态栏、用量聚合和对应配置/测试代码。

### 状态栏模型

新增内部状态栏视图模型，统一承载：

- 模型与推理等级。
- 当前目录和 Git 分支。
- 当前上下文窗口及使用率。
- 当前会话输入、缓存输入、输出和推理 Token。
- 5 小时与每周额度使用率。
- Daily、Weekly、Monthly、Total 聚合结果。
- 账户用量状态：加载中、可用、失效。

渲染器只消费该快照，不直接读取磁盘或执行 Git 命令。现有 Git、模型、上下文和额度数据继续复用 Codex 已有状态来源。

### 账户用量聚合器

复用 Codex `/usage` 已有的 app-server 请求：

```text
account/tokenUsage/read
```

响应中的 `daily_usage_buckets` 提供每日 Token 桶，`summary.lifetime_tokens` 提供 Lifetime 总量。聚合器按响应的日期字段计算：

- Daily：当天桶的 Token。
- Weekly：从当前自然周周一到当天的桶之和。
- Monthly：从当前自然月 1 日到当天的桶之和。
- Total：`summary.lifetime_tokens`。

### 缓存

账户用量结果保存在 TUI 内存快照中，首次启动及每 5 分钟异步刷新一次。状态栏启动后立即显示原生会话指标；账户用量未就绪时显示 `…`。

第一版不新增磁盘缓存，避免引入额外格式、失效和隐私面。离线或接口失败时保留进程内最后成功快照；若本次进程从未成功读取，则显示 `—`。

### 错误处理

- 账户用量请求超时或失败：保留最后成功快照并记录调试日志。
- `daily_usage_buckets` 缺失：Daily、Weekly、Monthly 显示 `—`，Total 仍使用 Lifetime。
- `lifetime_tokens` 缺失：Total 显示 `—`，其他桶统计继续显示。
- 日期字段无效：跳过该桶，不影响其他数据。
- 后台请求异常：状态栏不得 panic，也不得阻塞输入或模型流式输出。
- Git 信息不可用：省略 Git 分支及其相邻多余分隔符。

## 安装与回退

### 命令入口

- 定制 release 二进制安装为 `~/.local/bin/codex`。
- `~/.local/bin/codex-official` 是稳定包装器，调用官方 npm 安装版；若其不可用则回退到 Codex App 内置 CLI。
- 确保 `~/.local/bin` 位于交互式 shell 的 `PATH` 前部。
- 定制版 `codex --version` 包含 `custom` 标识。

### 升级

提供仓库内更新脚本，顺序为：

1. 获取官方目标 tag。
2. 在临时分支重放定制补丁。
3. 运行格式化、静态检查和测试。
4. 构建 release 二进制。
5. 先写入临时文件，再原子替换 `~/.local/bin/codex`。
6. 任一步失败时保留当前可用版本并输出明确错误。

不启用静默自动升级，避免上游 TUI 变化未经验证即覆盖本地版本。

## 测试

### 单元测试

- 宽窗口两行布局。
- 窄窗口三行布局及目录缩写。
- 超窄窗口安全裁剪。
- 字段缺失时的分隔符处理。
- 每个字段对应的 Ratatui 样式。
- K/M 数值格式及边界舍入。
- Daily 桶选择及 Weekly、Monthly 求和。
- 周一、月初和无效日期边界。
- Daily buckets 或 Lifetime 缺失时的降级显示。

### 集成与回归测试

- 使用固定账户用量响应 fixture 验证 Daily、Weekly、Monthly、Total。
- 运行 Codex TUI 并验证当前会话 Token 与上下文变化能触发刷新。
- 验证账户用量请求期间输入和模型流式输出不被阻塞。
- 验证账户用量刷新失败时保留最后成功快照。
- 验证原有 `/statusline` 字段配置及隐藏状态栏行为不被破坏。
- 验证 `codex-official --version` 可作为回退入口。

### 人工验收

- 在至少 80、100、140 和 180 列宽度下检查排版。
- 在深色和浅色终端主题下检查对比度。
- 在 Git 与非 Git 目录中检查字段变化。
- 运行长会话，确认数字更新时无明显闪烁或布局抖动。
- 对照已确认的 Visual Companion Mockup 检查颜色、顺序和换行。

## 交付物

- `~/developer/codex-custom` 中的定制源码分支。
- 定制 `codex` release 二进制。
- `codex-official` 回退包装器。
- 更新与安装脚本。
- 账户用量 fixture、单元测试和集成测试。
- 使用及升级说明。

## 参考

- OpenAI Codex 官方状态栏支持字段选择、排序和主题派生颜色，但不支持本设计的固定语义色、进度条和多行响应式布局。
- OpenAI Codex issue `#20012` 记录了彩色状态栏方案。
- OpenAI Codex issue `#20043` 记录了 Claude Code 式外部状态栏需求及当前限制。
