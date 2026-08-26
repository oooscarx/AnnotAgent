# Visual QA Checklist

## 品牌

- [ ] 侧栏、登录页或 About 页使用正式 SVG，不使用 moodboard 裁图。
- [ ] 核心页显示 AnnotAgent；RoboCup 项目显示 Skill badge 或 product lockup。
- [ ] favicon、PWA icon、OG image 路径正确。
- [ ] Logo 未拉伸，深浅背景变体正确。

## Token

- [ ] 业务组件不新增散落的品牌 hex 值。
- [ ] 状态、边框、背景、字体使用 token。
- [ ] annotation label 通过配置映射到 slot，而非组件中写死 RoboCup 类别。

## GUI

- [ ] Dashboard、Project、Review、Settings、Skills 页面统一。
- [ ] hover、focus、disabled、loading、error 状态完整。
- [ ] 1280×720、1440×900、1920×1080 可用。
- [ ] 浏览器缩放 200% 后主要操作仍可达。
- [ ] 键盘导航和 focus ring 正常。
- [ ] reduced motion 生效。

## TUI

- [ ] 256-color/truecolor 终端均可读。
- [ ] 选中、警告、失败不只靠颜色。
- [ ] 小终端降级布局不 panic。
- [ ] 原有快捷键和控制逻辑未改变。

## 回归

- [ ] API、运行状态、标注编辑和 review 行为未被视觉重构破坏。
- [ ] 前端 typecheck/build/test 通过。
- [ ] Rust fmt/clippy/test 通过。
- [ ] 未提交 API Key、字体文件、大型无关二进制或 moodboard 到运行时 bundle。
