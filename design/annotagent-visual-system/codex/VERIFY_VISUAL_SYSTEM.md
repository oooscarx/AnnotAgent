# Codex 接入后人工复核

1. 打开 `design/annotagent-visual-system/preview/index.html`，确认视觉基准。
2. 对比真实 GUI：Logo、侧栏、panel、状态、annotation overlay 是否遵循 token。
3. 搜索新增硬编码颜色：

```bash
rg -n "#[0-9A-Fa-f]{6}" web crates apps
```

允许出现的位置应集中在 token、SVG 和必要兼容层。

4. 检查 moodboard 没进入 bundle：

```bash
rg -n "ai-moodboard|reference/" web/src web/public
```

5. 检查 Core GUI 没硬编码 RoboCup 类别：

```bash
rg -n "ball|robot|field_line|penalty_mark" web/src
```

匹配只能来自 RoboCup Skill 配置、测试 fixture 或领域页面，不能散落在通用组件。

6. 手动检查 1280×720 与 200% zoom。
7. 只接受 Codex 实际执行过的测试结果。
