# Accessibility

- 主文本与背景对比至少满足 WCAG AA。
- `#00B3A4` 不用于白底小号正文；白底正文使用 `#0F766E`。
- 主按钮使用 `#2563EB` 和白字。
- 状态必须同时显示文字，不能只显示红黄绿圆点。
- annotation overlay 同时使用 label、形状、线型或 pattern。
- 键盘焦点使用统一 focus ring，不能移除后不提供替代。
- 交互目标建议至少 36×36px；关键编辑控制点需要可放大命中区。
- 尊重 `prefers-reduced-motion`。
- 图标按钮必须有 `aria-label` 和可见 tooltip。
- Canvas/SVG 编辑器需要提供 annotation 列表作为可访问的等价入口。
