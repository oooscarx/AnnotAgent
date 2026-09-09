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

## UIAPI-002 TIME_WAIT 重启阻塞

确认 dd98168 裸 bind 会在无 LISTEN 的 TIME_WAIT 上报 Errno48。28d5ec3 的 SO_REUSEADDR 修复现补 listen 探测与 3 项真实 socket 回归；已有服务仍拒绝、无 SO_REUSEPORT/安全豁免。带静态 dist 的真实 fixture Ctrl-C 后同库同端口立即重启通过，checkpoint/预算/SSE 不变。仅用自有 53155/53156，未操作前端 8792/8793 或用户 8787。见 UIAPI-002_TRACE.json；独立完整 patch 可用于 dd98168，不依赖 UIAPI-001 场景。

## UIAPI-003 bbox/停止浏览器证据

已在 fixture/support 范围增加 bbox seed 和手动 Stop 准备命令。bbox 通过 Workflow PATCH/CAS 选择现有 VLM Detection、真实 Sample/Geometry Safety、真实 HumanRequest answer 保存；留下独立待回答请求（当前 feedback sequence=1，下次为2），源图640×400。没有新增生产业务或读取收费模型。

验证：完整 seed 278 条 HTTP 请求通过；bbox Sample failed_count=0、归一化坐标 f32 回读正确、原 revision 重试不重复、冲突拒绝；浏览器 answer_example 在另一 TEST 场景提交/重试成功。带 bbox 的同库重启不变。手动 Stop 场景在两秒后仍 reserved，Stop 初始 stopping，最终 outcome_unknown / call in_doubt，resume 不可用。外部30秒延迟不等于本地stopping持续30秒。fixture all-targets严格Clippy与fmt通过；生产Rust代码未改。

证据：UIAPI-003_TRACE.json。自有测试服务已停止，测试数据保留；未操作前端 worktree、8787、真实密钥、安装、删除、push 或 merge。

## UIAPI-004 集成修复

Queue preview/POST 现在对 pending human 返回 409 `human_input_pending`（Schema clarification 为 `schema_clarification_pending`），附 admitted=false 和 answer_human_then_retry_same_command。授权事务复查并回滚新 grant；已有冻结授权保留，回答后原 ID/Consent 可重试，仍须原 scope/有效期/FIFO/预算成立。已有 call 回执只回放，未知结果不重发。无迁移、无自动调度改造。

真实 HTTP 验证 preview 拒绝、preview 后新增 human 导致 POST 拒绝且授权/预算不变、回答后原 Consent 成功、重复 POST 仅一个 call；存储测试验证旧冻结授权跨 SQLite 重开恢复。Storage/Application/Server 共 397 passed、0 failed、3 原有 ignored，相关 all-targets 严格 Clippy 通过。详见 HTTP_ADAPTER.md UIAPI-004 与 UIAPI-004_TRACE.json。最终 SHA 在 UIAPI-004 交付回复固定。

复现：先按 INTEGRATION_ENVIRONMENT.md 启动全新 TEST seed，再执行 `python3 crates/annotagent-e2e-fixture/support/http_queue_human_check.py --enable-fixture --manifest <TEST workspace>/manifest.json`。脚本消费 bbox pending 问题，每次回归用新 seed。未读真实密钥、未付费、未碰 8787/前端 worktree；自有测试进程已正常停止，隔离数据保留。提交后继续接口支持状态，只响应具体集成问题。

## UIAPI-008 调用进度、原因与 streaming 调查

已在 codex/agent-ui-backend 增量修复，基线334f671；核验 main83d2adb 的 Rust/migrations 与该基线一致，无合并前端或 main。call receipt 增加 started_at/completed_at/duration_ms/stage/failure，持久真实 reservation、Provider 调用、完整响应与结算边界；安全 failure 只含阶段/类别/HTTP状态。handler中断/重启仍为 in_doubt，同ID不重发。Builder operation是独立DTO，此次没有扩展其字段。

迁移0058仅新建metadata表；旧call保留created_at但缺失end/failure为null，不能追回此前被丢弃的原因。未访问用户报告的robocup-ball数据库或调用。Provider没有现成文本/tool delta协议，本轮未新增streaming；stream:true在发送前拒绝，新增读取body阶段取消。详见 UIAPI-008_PROGRESS.md、UIAPI-008_EXAMPLES.json、UIAPI-008_TRACE.json。

验证：563 passed、0 failed、3原有ignored（Core/Provider/Storage/Application/Server）；全workspace all-targets严格Clippy、fmt通过。隔离HTTP seed278请求，42个call回执观察/23个call身份，真实provider_request→settled/in_doubt、typed cancellation与计时验证通过。测试启动器已退出自有进程，数据保留。未接触8787/8788、真实密钥/服务/数据库、前端worktree；无push/merge。最终提交SHA在交付回复固定，继续接口支持状态。

## UIAPI-010 Model Profile CAS delivery

PATCH adds optional expected_revision and transaction-atomic compare/append. Stale edits return safe 409 expected/current revision; every successful HTTP PATCH advances revision, including metadata/no-op edits. Old revision and Published frozen snapshots remain unchanged. Legacy omitted-field requests remain accepted with no claim of stale-browser protection. No migration or frontend changes.

Isolated tests: Storage/Server 243 passed, 0 failed, 2 existing ignored; includes independent-connection two-writer race, reopen/old revision/Published snapshot preservation, and HTTP contract/legacy requests. Workspace all-target Clippy with -D warnings, fmt check and diff check passed. Commands and exact contract: docs/contracts/agent-ui-v1/UIAPI-010_MODEL_CAS.md. No paid calls, keys, real workspace mutation, service restart, push or merge. Final SHA is in delivery response.

UIAPI-009 remains blocked/unimplemented. Prior 8cccca5f4f45ba4203ae77c4d74c79a29fa86323 delivered a concrete gap specification only (UIAPI-009_GAP.md); no scope establishment or runtime acceptance is claimed.
