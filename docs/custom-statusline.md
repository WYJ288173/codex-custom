# Claude 风格 Codex 状态栏

此分支提供固定语义颜色、响应式多行布局和账户 Token 汇总。官方单行状态栏仍是默认行为；通过以下配置启用定制布局：

```toml
[tui]
status_line_layout = "claude"
status_line_use_colors = true
status_line = [
  "model-with-reasoning",
  "context-remaining",
  "current-dir",
  "total-input-tokens",
  "total-output-tokens",
]
```

`status_line = []` 可完全隐藏状态栏。Claude 布局固定展示模型/推理等级、目录、Git 分支、上下文、会话输入/输出 Token、Today/Week/Month/Total，以及 5 小时和每周额度。

终端宽度不小于 140 列时使用两行；低于 140 列时使用三行并缩写中间目录。每一行在超窄终端中独立安全裁剪。

## 命令入口

- `codex`：`~/.local/bin/codex` 中的定制 release 构建。
- `codex-official`：优先调用当前 Node 环境中的官方 npm 包，缺失时调用 Codex.app 内置 CLI。

安装或重装：

```sh
./scripts/install-custom-codex.sh
```

更新到明确的官方 tag：

```sh
./scripts/update-custom-codex.sh rust-vX.Y.Z
```

更新脚本要求当前位于干净的 `feature/claude-style-statusline` 分支；只有重放补丁、格式检查、定向测试和安装全部成功后才快进功能分支。

## 用量与离线行为

Today、Week、Month 和 Total 复用 Codex app-server 的 `account/tokenUsage/read` 接口，每五分钟最多异步刷新一次。请求不会扫描会话正文。读取失败时保留本进程最后一次成功值；本进程尚无成功结果时显示 `—`。

## 回退

直接运行 `codex-official` 可临时回退。若要让官方版本重新成为 `codex`，删除 `~/.local/bin/codex`，或把 `~/.local/bin` 移到官方 npm bin 目录之后。
