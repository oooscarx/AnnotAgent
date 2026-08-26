# 当前项目已知未完成项

以下是视觉系统接入前已有的项目状态。视觉接入任务不得声称顺便解决了这些内容，除非确实实现并完成对应测试：

- Dataset Coordinator 尚无持久化 batch checkpoint、断点自动续跑、跨并发图片共享的全局预算账本和 batch 级控制句柄。
- HTTP 当前启动单图 run；多图并发由 CLI 提供。
- Native/COCO/LabelMe 标注导入尚未实现；已有文件夹图片导入及五种导出。
- GUI 尚不支持删除单个顶点、bbox 角点 resize、空画布新建标注和 before/after 并排对比。
- TUI 的 `/init`、`/export` 仍需使用 CLI，且未实现面板焦点导航和取消二次确认。
- 不支持无 Tool Calls Provider 的 JSON-only 降级。
- 完整真实 Qwen 六任务运行未验证完成。
- 用户曾指出 API Key 出现在对话中。不得从对话、日志或历史复制任何 Key；只使用环境变量，并提醒用户轮换旧 Key。
- 远程已配置但未 push。视觉接入任务不得修改 remote 或执行 push，除非用户另行明确要求。
