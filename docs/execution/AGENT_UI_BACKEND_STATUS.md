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


## 2026-09-09 前后端联调支持

状态：交接完成，进入接口支持；仅接受前端具体集成问题，不扩展产品功能。

固定后端代码 SHA：b8955cabcca72d87b1ed27a5cb132789ca535b60。新增仅 test-only Server 入口、已有外部模型 fixture 的启动/HTTP验证脚本、Adapter/环境文档；生产路由与运行时行为未改。

真实 HTTP 已覆盖导航分页、user thread、Plan 无隐式执行、精确 Journey 授权、模型下一请求冻结、Processing/终端 Run、人工答案保存/重复/冲突/继续、Queue 授权派发/取消/重复、Settings 六组/CAS、native 导出下载、Stop stopping→outcome_unknown、Run SSE 重连/gap。重启读取证实 calls/budget/queue/answers/thread/stop/events 不变，已完成工作不重复执行。固定 SHA 证据见 INTEGRATION_TRACE.json。

检查：server lib 57 passed、0 failed、2 ignored（前台 fixture 与既有 checkpoint 子进程）；严格 Clippy、fmt、diff check 通过。全 workspace B3 的 741 passed 记录仍保留，本轮没有将旧全量结果冒充重跑。

未改 web/CSS/types/design/package；未碰前端集成 worktree；无 merge、push、remote 修改或真实数据操作。无真实钥匙/付费模型/大权重/8787 服务操作。详细启动、测试、DB/port、残留边界与消息来源见最新 handoff。

## UIAPI-001 测试交付

已在独立后端提交补齐 support：真实 interrupted Stop、3 图 paused Batch（实际 resume 复用第 1 个 child Run，再暂停于 2 completed / 1 pending）、明确 available resume、saved_plan JSON 路径。原有种子的 Plan-only Send 与已存 Builder proposal 分开标记。HTTP_ADAPTER 补充 assistant/Plan 字段与首次真实 Send 创建任务流程。

验证：244 条真实 HTTP 请求通过；同库重启预算/checkpoint/动作一致；占用端口拒绝检查通过。证据 UIAPI-001_TRACE.json。此轮未改 Rust 业务、web、包或设计；仅 Python 测试支持及文档。没有重跑无改动的全量 Rust 测试，也没有以旧结果声称本轮重跑。未 push/merge。
