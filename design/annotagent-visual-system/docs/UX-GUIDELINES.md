# AnnotAgent UX Guidelines

## 信息架构

推荐固定三栏：

1. 左侧：Project / Image / Review Queue。
2. 中间：图像画布与标注编辑。
3. 右侧：Annotation、属性、Validation Evidence。
4. Agent Trace 作为可折叠底栏或右栏 Tab，不抢占主画布。

## 视觉优先级

1. 当前图片和标注几何。
2. 当前任务、Review 状态和阻塞问题。
3. Validator evidence 和修复建议。
4. Agent trace、Token 和费用。
5. 品牌装饰。

装饰排在最后，因为用户打开标注工具通常不是为了欣赏渐变。

## Agent Trace

每行只显示：时间、类型、名称、摘要、状态、usage。不要展示或声称展示隐藏思维链。

建议类型：MODEL、TOOL、VALIDATOR、REFINER、REVIEW、COMMIT。

## 编辑器

- 选中标注：实线 + 控制点 + label。
- 未选中标注：较低不透明度。
- hover：提高对比，不改变几何。
- 错误：使用 issue badge 与证据面板，不把整张图染红。
- before/after：使用相同缩放与明确图例。
- 工具栏常用动作保持固定位置，避免页面切换后漂移。
