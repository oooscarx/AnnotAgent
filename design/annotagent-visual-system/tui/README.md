# TUI integration

`annotagent_theme.rs` 是独立 Ratatui theme 示例，不引用 AnnotAgent 业务类型。接入时：

1. 将文件移动到现有 TUI theme 模块。
2. 在应用状态到 `StatusTone` 之间建立一处映射。
3. 保留现有快捷键、事件循环和面板行为。
4. 颜色不可成为唯一状态信号；标题或 badge 同时显示文字。
5. 对不支持 truecolor 的终端提供现有 fallback，或映射到 ANSI 256 色。

不要为了套主题重写整个 TUI 状态机。那是视觉接入，不是借机发动一次架构政变。
