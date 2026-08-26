# RoboCup Visual Extension

RoboCup 不替换 AnnotAgent Core 品牌。它增加：

- `RoboCup AnnotAgent` lockup；
- soccer skill badge；
- RoboCup 标签到通用 annotation slot 的映射；
- field / ball / robot 等领域词汇；
- 产品 descriptor。

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

颜色不是业务逻辑。后端 Schema 和 Skill Manifest 仍是类别真相，GUI 只做展示映射。
