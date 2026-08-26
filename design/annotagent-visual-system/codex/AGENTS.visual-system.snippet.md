## AnnotAgent Visual System

- 视觉系统的 canonical source 是 `design/annotagent-visual-system/tokens/tokens.json`、`tokens/tokens.css` 和 `brand/logo/svg/`。
- `reference/` 中的 AI moodboard 只用于气质参考；禁止从 PNG 吸色、裁 Logo 或把它们加入运行时 UI。
- AnnotAgent 是 Core 品牌；RoboCup 是 Skill 扩展。Core 组件不得硬编码 `ball`、`robot`、`field_line` 等类别。
- 组件不得散落新增品牌 hex。使用 CSS variables、共享 token 或现有 theme abstraction。
- 功能状态和 annotation 类别不能只靠颜色表达；同时使用文字、图标、形状、线型或 pattern。
- 接入视觉系统时保持 API、Agent Runtime、标注数据模型、快捷键和任务行为不变。
- 不添加字体文件，不把 API Key 写入代码、日志或 Git。
- 修改 Web 后运行 typecheck/build/test；修改 TUI/Rust 后运行 fmt/clippy/test。只报告真实执行结果。
