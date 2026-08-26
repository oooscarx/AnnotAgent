# 将 AnnotAgent Visual System 接入现有仓库

你现在负责把仓库中的 GUI、TUI、品牌资产和文档统一到 **AnnotAgent Core + RoboCup Skill** 视觉系统。

这次任务是一次**视觉系统接入与前端一致性改造**，不是重写产品功能。请直接检查仓库并修改代码，不要只给建议或截图。

## 一、必须先读

按顺序阅读：

1. `design/annotagent-visual-system/README.md`
2. `design/annotagent-visual-system/brand/BRAND-GUIDELINES.md`
3. `design/annotagent-visual-system/docs/UX-GUIDELINES.md`
4. `design/annotagent-visual-system/docs/ACCESSIBILITY.md`
5. `design/annotagent-visual-system/docs/ROBOCUP-EXTENSION.md`
6. `design/annotagent-visual-system/docs/VISUAL-QA-CHECKLIST.md`
7. `design/annotagent-visual-system/codex/CURRENT-PROJECT-STATE.md`
8. `design/annotagent-visual-system/tokens/tokens.json`

然后检查：

- Git status；
- Web 技术栈、构建工具、已有组件库和样式方式；
- TUI crate、布局和当前颜色常量；
- favicon、PWA、README、截图和产品标题的现状；
- 现有测试与 lint 命令。

不要覆盖用户未提交的无关修改，不要执行 reset、clean、force checkout、remote 修改或 push。

## 二、架构原则

产品关系：

```text
RoboCup AnnotAgent
= AnnotAgent Core
+ RoboCup Skill
```

必须做到：

- Core 品牌、通用组件和 design tokens 使用 `AnnotAgent`；
- RoboCup 项目通过 Skill badge、product lockup 和 label mapping 扩展；
- 通用 GUI 组件不能硬编码 `ball`、`robot`、`field_line` 等类别；
- 类别颜色来自 Skill manifest、project schema 或 label mapping；
- 不把 RoboCup 足球/机器人图形塞入 Core Logo；
- 不创建第二套相互冲突的主题系统。

## 三、Canonical Source

实现时只把以下内容视为设计真相：

```text
design/annotagent-visual-system/tokens/tokens.json
design/annotagent-visual-system/tokens/tokens.css
design/annotagent-visual-system/brand/logo/svg/
design/annotagent-visual-system/brand/icons/svg/
```

`reference/*.png` 是 AI moodboard，只用于理解风格：

- 禁止从 PNG 吸色；
- 禁止裁剪其中的 Logo；
- 禁止把 moodboard 放进正式 UI；
- 禁止照抄其中可能不准确的小字、数据或界面结构。

## 四、先写接入计划，再实施

创建：

```text
docs/VISUAL_SYSTEM_INTEGRATION.md
```

记录：

- 现有 Web/TUI 样式入口；
- token 接入位置；
- 将修改的页面与组件；
- 旧颜色常量如何迁移；
- 保持不变的功能边界；
- 验证命令。

写完后继续实现，不要停下来等待批准，除非仓库存在无法安全合并的重大歧义。

## 五、资产接入

将正式资产放到仓库已有约定路径。如果没有约定，使用：

```text
web/public/brand/
```

至少接入：

- `annotagent-mark.svg`
- `annotagent-lockup-light.svg`
- `annotagent-lockup-dark.svg`
- `robocup-annotagent-lockup-light.svg`
- favicon.svg / favicon.ico
- apple-touch-icon.png
- pwa-192.png
- pwa-512.png
- og-card.png

规则：

- SVG 优先；
- PNG 只用于 PWA、social card 或必须使用位图的地方；
- 不重复存放多份无来源说明的 Logo；
- 如果现有项目已有 `manifest.webmanifest`，只更新图标引用，不破坏其他字段；
- 更新 HTML title、meta theme-color、favicon 和必要的 Open Graph metadata；
- README 标题使用 `RoboCup AnnotAgent`，内部架构仍说明 `AnnotAgent Core`。

## 六、Token 与主题

将 `tokens/tokens.css` 接入现有全局样式入口。

如果项目已有 theme abstraction：

- 将 AnnotAgent token 映射到现有变量；
- 保留必要兼容 alias；
- 不并行维护两套完全重复的颜色常量。

如果项目使用 Tailwind：

- 合并 `tokens/tailwind.preset.cjs` 的 extend 内容；
- 不替换整个 Tailwind config；
- 不把所有组件强行改写为 Tailwind。

要求：

- 浅色工作区 + 深色导航栏是默认视觉；
- 已有 dark mode 时补齐 token；没有时不要为本任务强行增加完整 dark mode；
- 品牌渐变只用于 Logo 和少量 hero；
- 功能按钮和状态使用纯色；
- 不新增散落品牌 hex；
- annotation slot 与 semantic status 分离。

## 七、Web GUI 接入范围

统一以下页面的视觉语言，但保持行为与 API 不变：

1. Dashboard；
2. Project / image list；
3. Run progress；
4. Annotation Review；
5. Agent Trace；
6. Settings；
7. Skills；
8. 空状态、错误状态和 loading 状态。

### App shell

- 深色左侧导航；
- 主工作区使用安静的浅灰背景；
- panel 使用白色 surface、细边框和低强度阴影；
- 标题、breadcrumb、主要动作位置稳定；
- 不用大面积玻璃拟态或过度圆角。

### Review workspace

优先级：

1. 画布；
2. Annotation 列表；
3. Validator evidence；
4. Review action；
5. Agent trace；
6. usage/cost；
7. 品牌装饰。

接入 annotation overlay：

- 使用通用 `annotation-1..8` slots；
- RoboCup 映射从 Skill/project 配置读取；
- 选中标注使用实线、控制点和 label；
- 未选中标注降低不透明度；
- overlay 添加对比 halo；
- 颜色之外同时使用 label、shape、line style 或 pattern；
- 不改变当前几何编辑行为。

### 状态

统一：

```text
Draft
Running
Auto accepted
Needs review
Rejected
Failed
```

状态必须有文字。不要只留一个神秘色点让用户猜测。

### 组件

可以参考并适配：

```text
design/annotagent-visual-system/web/src/components/
design/annotagent-visual-system/web/src/styles/
```

不要机械复制。如果仓库已有 Button、Badge、Panel、Tabs，优先给现有组件接 token。

## 八、TUI 接入范围

参考：

```text
design/annotagent-visual-system/tui/annotagent_theme.rs
```

要求：

- 抽取一个统一 Theme；
- 深色 navy 背景；
- primary 用于选中；
- teal 用于成功工具/验证轨迹；
- warning、danger 保留语义；
- 文本同时显示状态；
- 保持现有事件循环、快捷键和命令行为；
- 不因为换颜色重写 TUI 架构；
- 小终端不 panic；
- 如已有 ANSI 256 fallback，保留并映射。

更新 TUI 标题为：

```text
RoboCup AnnotAgent
AnnotAgent Core · RoboCup Skill
```

但内部 crate 和 CLI 仍使用 `annotagent`。

## 九、无障碍与响应式

必须验证：

- `#00B3A4` 不作为白底小正文颜色；使用 `#0F766E`；
- 主按钮蓝底白字；
- focus-visible 可见；
- 图标按钮有 accessible name；
- keyboard navigation 不退化；
- `prefers-reduced-motion` 生效；
- 1280×720、1440×900、1920×1080 可用；
- 200% 浏览器缩放时主要操作可达；
- annotation 列表可作为 Canvas/SVG 的等价文本入口。

## 十、禁止范围

本任务不要顺便实现：

- Dataset Coordinator checkpoint；
- 多图 HTTP batch runtime；
- Native/COCO/LabelMe import；
- JSON-only Provider fallback；
- 真实 Qwen 六任务验证；
- Git remote 修复或 push；
- 全新的前端框架；
- 字体文件下载或提交；
- 业务 API 重写；
- Agent Runtime 重构。

现有 GUI 编辑缺口只有在视觉接入所必需且能完整测试时才处理。否则保留在 Known Limitations，不要声称完成。

## 十一、安全

- 不读取或复用对话中的 API Key；
- 不把 Key 写入源码、设置快照、日志或前端 bundle；
- 不修改 `.env` 中真实值；
- 检查新增文件不包含 secret；
- 不执行 push。

## 十二、验证

首先读取仓库的 `AGENTS.md`、README 和 package scripts，使用真实命令。

至少执行适用项：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --all-features
```

前端在实际 package manager 下执行：

```bash
npm run typecheck
npm run test --if-present
npm run build
```

如仓库使用 pnpm/yarn/bun，使用现有 lockfile 对应工具，不要擅自生成第二种 lockfile。

启动 GUI 并进行 smoke test，至少验证：

- favicon/title；
- Dashboard；
- Project；
- Review 页面；
- annotation overlay；
- Agent Trace；
- Settings；
- Skills；
- TUI 启动和颜色；
- pause/cancel 等原行为无回归。

无法人工操作浏览器时，明确说明未完成的交互验证，不要编造。

## 十三、完成标准

完成时必须满足：

1. 正式 Logo 与图标接入；
2. token 成为视觉真相；
3. GUI 主要页面统一；
4. TUI 使用统一 Theme；
5. RoboCup 通过 Skill extension 表达；
6. Core 组件无 RoboCup 类别硬编码；
7. 无障碍与响应式检查完成；
8. 文档更新；
9. 测试真实执行；
10. 不改变功能与 Git remote。

## 十四、最终回复

只报告：

1. 实际修改文件；
2. token/asset 接入方式；
3. GUI 变化；
4. TUI 变化；
5. Core 与 RoboCup Skill 的视觉边界；
6. 无障碍措施；
7. 执行命令与真实结果；
8. 未验证或未完成内容；
9. 值得人工检查的页面。

不得使用“基本完成”“应该可以”“理论上通过”。视觉系统不是量子态，打开页面之后总该有个确定结果。
