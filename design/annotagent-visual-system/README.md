# AnnotAgent Visual System 1.0

这是 **AnnotAgent Core** 的完整视觉系统包，并包含 RoboCup Perception Skill 的领域扩展与示例项目资产。AnnotAgent 是产品；RoboCup 不是产品壳，也不拥有全局导航或通用空状态。

## 最重要的规则

1. **Core 品牌叫 AnnotAgent。** Skill 是可注册扩展；RoboCup 只是其中一个示例。
2. **代码中的唯一视觉真相**是 `tokens/tokens.json`、`tokens/tokens.css` 和 `brand/logo/svg/`。
3. `reference/` 中两张 AI 生成品牌板只用于气质参考。不要从图片吸色，不要把图片裁下来当正式 Logo，也不要照着其中可能失真的小字实现界面。
4. 功能状态不能只靠颜色表达。必须同时使用文字、形状、图标或线型。
5. 不包含任何字体文件。推荐 Inter 和 JetBrains Mono；未安装时使用系统 fallback。

## 目录

- `brand/logo/`：Core Logo、favicon、PWA 图标和 OG 卡片；RoboCup lockup 是 Skill/Application 示例资产。
- `brand/icons/`：原创、统一笔画的标注与 Agent 图标。
- `brand/palette/`：颜色定义和快速预览。
- `brand/robocup/`：RoboCup Skill badge、示例资产及 label 到通用 annotation slot 的映射。
- `tokens/`：JSON、CSS、TypeScript 与 Tailwind preset。
- `web/`：可复制到 React 项目的主题 CSS、Logo 与状态组件。
- `tui/`：Ratatui 色彩映射和 ASCII 标识。
- `preview/`：直接打开 `preview/index.html` 查看静态预览。
- `docs/`：品牌、UX、无障碍、RoboCup 扩展和视觉验收说明。
- `codex/`：安装脚本、Codex Skill、AGENTS 片段和完整接入 Prompt。

## 推荐安装

在解压后的视觉系统目录执行：

```bash
python3 codex/install_visual_system.py --repo /path/to/annotagent
```

它只会：

- 将本包复制到目标仓库的 `design/annotagent-visual-system/`；
- 安装 repo-scoped Codex Skill 到 `.agents/skills/annotagent-visual-system/`；
- 不会修改现有源码、`AGENTS.md`、Git remote 或密钥。

然后从仓库根目录启动新的 Codex 会话，显式调用：

```text
$annotagent-visual-system
```

并让它读取：

```text
design/annotagent-visual-system/codex/CODEX-PROMPT.md
```

## 快速人工接入

Web 运行时必须按产品层级投放：

```text
web/src/styles/annotagent-tokens.css
web/src/styles/annotagent-theme.css
web/public/brand/core/
web/public/brand/skills/robocup/
```

TUI：

```text
tui/annotagent_theme.rs
```

正式 Logo：

```text
brand/logo/svg/annotagent-lockup-light.svg
brand/logo/svg/annotagent-lockup-dark.svg
brand/logo/svg/robocup-annotagent-lockup-light.svg
```

## 商用提醒

本包提供视觉设计，不提供名称或商标法律清查。公开商业化前仍应检查 `AnnotAgent` 名称冲突、RoboCup 相关标识使用规范以及目标地区商标情况。
