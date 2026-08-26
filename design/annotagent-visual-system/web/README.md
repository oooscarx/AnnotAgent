# Web integration

推荐顺序：

1. 将 `public/brand/` 合并到项目现有 `web/public/brand/`。
2. 将 `src/styles/annotagent-tokens.css` 与 `annotagent-theme.css` 放入现有样式目录。
3. 在应用入口最先导入 tokens，再导入 theme。
4. 根据现有组件体系移植 `AnnotAgentLogo.tsx`、`StatusBadge.tsx` 和 `AnnotationLegend.tsx`。
5. 如果项目使用 Tailwind，只合并 `tokens/tailwind.preset.cjs`，不要替换已有配置。
6. 不要同时保留三套颜色常量。旧 token 应迁移或成为兼容 alias。

正式 UI 应优先使用 SVG。PNG 仅用于 PWA、社交卡片和需要固定位图的场景。
