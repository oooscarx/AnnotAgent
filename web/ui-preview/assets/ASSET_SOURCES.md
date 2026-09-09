# 素材来源与使用边界

- `brand/mark-*`：保留用户仓库 AnnotAgent 标志的路径轮廓，移除渐变和装饰高光，仅改为单色。来源 commit `b694f061de35c7dfd713ef6838ef73d9a02afb8e`，`web/public/brand/core/annotagent-mark.svg`。不是新品牌，不复制 Codex/Claude 标志。原项目权利说明继续适用。
- `icons/*.svg`、`empty/*.svg`：本次从基本几何原创绘制，提供给 AnnotAgent 集成使用。无外部 icon 库。
- `demo/still-life-*`：本次原创 SVG 静物示意图及其 PNG，专门用于界面样稿，不是真实摄影，不是 VLM/SAM 输出，不用于准确率评估。
- `demo/annotations.fixture.json`：手工设置的界面示例坐标，明确 `fixture_only=true`。
- 所有页面使用系统字体栈。不附带字体文件，不要求联网下载字体。
- Empty SVG 仅在紧凑空状态使用，禁止变成大面积装饰插图。
- 成品仓库必须用用户图片和真实结果替换 demo fixture；原图不叠加主题滤镜。
