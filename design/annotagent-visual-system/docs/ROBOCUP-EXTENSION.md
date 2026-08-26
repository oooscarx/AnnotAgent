# RoboCup Skill Visual Extension

RoboCup 不替换 AnnotAgent Core 品牌，也不是一个全局产品模式。它只在启用该 Skill 的 Project、Skill 目录、领域 Review 数据和示例材料中增加：

- 可选的示例 Application lockup；
- RoboCup Perception Skill badge/icon；
- RoboCup 标签到通用 annotation slot 的映射；
- field / ball / robot 等领域词汇；
- 示例 Project descriptor。

映射见 `brand/robocup/robocup-label-map.json`。

默认：

| 标签 | Slot | 视觉 |
|---|---|---|
| field_region | slot6 | 绿色半透明 polygon + 斜线 pattern |
| field_line | slot2 | 青色 polyline |
| ball | slot4 | 琥珀色 bbox/circle + 文本 |
| robot | slot1 | 蓝色实线 bbox |
| person | slot3 | 紫色虚线 bbox |
| penalty_mark | slot5 | 玫红 crosshair |

颜色不是业务逻辑。后端 Schema 和 Skill Manifest 仍是类别真相，GUI 只做展示映射。全局 Header、Dashboard、空状态、favicon 和通用 TUI 只使用 Core 资产。

运行时交付路径是 `web/public/brand/skills/robocup/`。Core 资产必须留在 `web/public/brand/core/`；示例 lockup 可以保留，但不能替代 AnnotAgent 主 Logo。
