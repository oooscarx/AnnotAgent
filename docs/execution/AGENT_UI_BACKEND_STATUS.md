# Agent UI backend status — B3 验证完成

BASE：c41b281b49252d520117029d39611865133798af。
分支：codex/agent-ui-backend。独立 worktree：AnnotAgent-backend。

- B0 d8d6627：源码核验后先提交 HTTP_BINDINGS / EXAMPLES / CHANGE_REQUESTS。
- B1 f6e23c2：轻量导航与 exact Task thread/snapshot、安全 Settings 六组视图、预算 CAS、Run SSE replay。
- B2 1df67f8：实际默认 Agent ID 冻结、Plan 工具过滤、真实停止归一化、双标签页队列派发、错误语义与 checkpoint action。
- B3：最终格式化、示例 DTO 校验、全量验证、Trace、迁移及交接完成。最后交付提交包含此状态文件；HEAD 在最终回复中报告。

验证：`cargo test --workspace --offline` 741 passed / 0 failed / 6 ignored；最终 HTTP lib 57 passed / 1 ignored；全 workspace build 通过；相关 crates 严格 Clippy -D warnings 通过；cargo fmt --all --check、git diff --check、JSON 与文件所有权检查通过。

测试使用独立 target/tempfile DB/InMemorySecretStore/mock Provider；HTTP in-process，测试 Provider 随机隔离端口。不接管 8787，不读真实 Key，不调用付费模型、不下载权重。没有前端/设计/包文件更改，没有自动合并、push、remote 修改、reset/rebase/amend。

交付：docs/handoff/AGENT_UI_BACKEND.md；完整契约、Schema、DTO、四条实际测试 Trace、迁移与测试矩阵在 docs/contracts/agent-ui-v1。

保留边界：无全系统金额/Token 统一分页，无 Conversation 统一事件游标；Queue 显式授权 POST 驱动；非原子多实体 snapshot；历史未配置模型不回填；generic cancelled/unknown 不自动恢复；部分旧错误仍 generic invalid_request。不会用假状态补这些能力。仅声明后端接口/服务验证结果，不声明新 UI 已集成。
