# Agent B 后端交接

BASE SHA：c41b281b49252d520117029d39611865133798af。
分支：codex/agent-ui-backend；worktree：/Users/oscar/Documents/my_workspace/AnnotAgent-backend。
可集成后端代码固定为 **b8955cabcca72d87b1ed27a5cb132789ca535b60**（含 B3 ce46c6f 与本轮真实 HTTP fixture）。后续交接证据提交仅改 docs；集成代码以此 SHA 为准，不跟随移动 HEAD 猜版本。

当前阶段：接口支持。后端不继续扩展产品功能，只处理前端具体联调问题；前端视觉通过后在独立集成分支负责唯一合并与接线。此次没有 merge 或 push。

最新交接入口：

- `docs/contracts/agent-ui-v1/INTEGRATION_ENVIRONMENT.md`：可重复启动、独立数据库/端口、重启验证与同源要求。
- `docs/contracts/agent-ui-v1/HTTP_ADAPTER.md`：实际字段、actions/reason、revision、command ID、consent、分页、错误及 Thread 来源。
- `docs/contracts/agent-ui-v1/INTEGRATION_TRACE.json`：固定 SHA 的真实 HTTP 示例与停止/继续/队列/SSE/导出证据。

最短启动：在 backend checkout 执行 `python3 crates/annotagent-e2e-fixture/support/http_fixture.py --enable-fixture`。自动两个 loopback 空闲端口、TEST 临时 workspace/.annotagent/history.db；保留数据，Ctrl-C 只停自有子进程。加 `--smoke` 自动验证后退出；加 `--workspace <已打印的 TEST 路径>` 重启同库。禁用 8787，不读取系统/真实 credential、不调用付费模型、不下载权重。fixture 响应带明确 header，生产没有 Mock 入口。

最新验证：真实 HTTP seed + 同库重启通过，Server lib 57 passed / 0 failed / 2 ignored；严格 server all-targets Clippy、fmt、diff check 通过。第二个 ignored 是显式前台 fixture server，并非漏测业务。无 SQL schema 迁移；只有 test-only 入口、Python 支持脚本和契约文档。

支持边界：paused Batch 从持久 checkpoint 继续；同进程 paused Run 可 resume control；当前人工答案先保存再本地修订/按原精确 consent 继续。cancelled/unknown 不能通用 resume。Queue 为显式授权 POST 驱动，GET/重连不派发。Conversation 靠 snapshot+journal+operation GET 刷新；Run SSE 独立持久补发。Settings task ledger、project limit、probe usage 范围不同，无统一全系统费用。

消息来源：`T/thread` 仅 SQLite 中真实归属用户消息。Schema/Builder/Feedback 是模型结构化决策，执行卡片是系统回执；缺少通用自然语言 assistant 回复，请前端明确来源，不编造成功消息。

B0：d8d6627（先交实际映射）。B1：f6e23c2（导航/快照/安全设置/事件恢复）。B2：1df67f8（实际默认模型冻结、控制回执与并发恢复验证）。

## 实际接线资料

`docs/contracts/agent-ui-v1/HTTP_BINDINGS.md` 是逐项映射；ROUTES.json 是当前路由注册；SCHEMAS.json 是常用 JSON Schema；DTO_INVENTORY.json 是复杂 DTO 的精确 Rust 定义；EXAMPLES.json 是 TEST 示例；TEST_MATRIX.md、TRACES.json、MIGRATION.md 与 CHANGE_REQUESTS.md 记录验证和边界。

A 的最小接线顺序（仅用户授权集成后）：

1. 用 GET /api/navigation 展示扁平 Project，展开时读现有 Conversation 的 task-navigation；route ID 与 owner UUID 分开，禁止按 display_name 合并。
2. 选定 Task 读 workspace + thread。调用 response 中现有样例、人工请求、Builder/Processing/Call DTO；工具来源明确标记。用现有 messages endpoint 查看未归属 Task 的旧历史。
3. 发送保留 message.id 与 frozen owner/image/schema/mode；读取 send receipt。先调用 exact preview，再独立批准；不要把 Execute 按钮当作全授权。
4. Model Picker 读 Registry 和 agent-model CAS；SendReceipt.resolved_agent_model_id 已冻结实际默认模型。视觉 Workflow binding 不变。
5. Stop 必须保存 stop-requests，并在多目标时显式 select。以 observation/normalized_state 显示 stopping/interrupted/unknown；GET 从不 signal。resume_actions 只列已有 checkpoint 的真实能力。
6. Queue 显式读取与取消；exact consent 的 POST 执行才可能派发。未知结果重复原 command/payload，不能创建新 call 规避账本。
7. Settings 用 `GET /api/settings?view=agent-ui`；未来预算用 PATCH expected_revision；Provider、默认 Agent、插件、清理和 call-limit 沿用各独立受控接口。
8. Run 事件用 GET /api/events?run_id=UUID，保存 SSE id，Last-Event-ID 重连；遇 gap/resync_required 读 snapshot/history。Conversation 事件仍需要 journal 与 task snapshot 重拉。

## 已有、新增与限制

复用 Conversation/Builder/Journey/PlanCandidate/Queue/Stop/HumanRequest/Provider/Model Registry/Settings/Runtime/Geometry Safety、结果与历史。没有第二套核心实体或调度引擎。

新增：轻量导航分页、exact task thread/snapshot 与恢复动作投影；默认 Agent 解析结果原子冻结；安全 Settings 六组视图及预算 CAS；Run 持久 SSE replay；结构化部分 owner/stale 错误；双标签页唯一派发与停止 Trace 测试。

限制：task snapshot 是多个已有提交记录的聚合，不是跨实体原子读；导航状态是 activity projection，queue/requests/operation 状态需独立显示。旧或未配置模型回执不伪造历史冻结。无统一 Conversation 事件游标、无全系统会计分页、无任意 cancelled/unknown 原地复活、无后台自动授权/队列轮询执行。复杂旧错误仍可能 generic 400。旧设置兼容 PUT 的无 revision 模式保留，新 UI 不应使用它覆盖预算。物理目录搬迁不在稳定 ID 支持范围。

人工答案使用现有 revision/outbox：先存储答案再继续，原 revision_id 重试，不要求重画。终端结果用 result-summary，中间 evidence 用 debug-summary；不能将多个阶段的框累计成多个对象。

## 隔离运行与验证

已使用本 worktree target、tempfile 数据库、InMemorySecretStore 和 fake/mock Provider；HTTP Router 测试在进程内执行，fake Provider 的监听端口由系统分配。没有绑定 8787，也没有启动共享 8792 服务。没有真实密钥、付费模型、大权重或用户 workspace 的读写。

可重复的测试命令（从本 worktree 执行）：

```sh
CARGO_TARGET_DIR="$PWD/target" cargo test --workspace --offline
CARGO_TARGET_DIR="$PWD/target" cargo build --workspace --offline
CARGO_TARGET_DIR="$PWD/target" cargo clippy -p annotagent-storage -p annotagent-application -p annotagent-server --all-targets --offline -- -D warnings
cargo fmt --all --check
```

联调阶段改用上方已实测启动器；原空白 CLI 命令不再作为 fixture 交接方案。HTTP API 不依赖 web dist，本轮没有安装 Node 包或修改 web。

验证结果：全 workspace 741 passed、0 failed、6 原有 ignored（包含示例 DTO 校验）；最终 HTTP server lib 57 passed、1 ignored。全 workspace build、严格相关 Clippy、全 workspace fmt check 通过。详见 TEST_MATRIX.md。

## 停止/继续/队列证据

TRACES.json 的两条 HTTP Stop Trace 证明请求持久化后 stopping，worker 本地结算后 interrupted 或 outcome_unknown；相同命令还原同一选中目标，预算前后均保留 1 次已保留调用。停止不意味着远端退款。

Queue Trace：两个并发 reservation 只有一次 Admitted、一次 Existing；先处理原 FIFO 项，第二项取消后重启不复活；累计 used_calls=2。

Batch Trace：暂停后关掉应用再重开，继续相同 Batch checkpoint，终态 100 completed、0 remaining，恰好 100 个 child Run，累计 usage 与持久 ledger 相同；此为 mock 测试，不是实际模型精度/计费证据。

迁移无需新 SQL schema；可选 JSON 字段与兼容策略详见 MIGRATION.md。

## cross_boundary_requests

仅 A 在用户确认视觉并授权后集成。本分支没有 merge/cherry-pick 前端，没有 push/remote 修改，没有 reset/rebase/amend，没有修改 web、设计、前端 types/package 或用户原工作区文件。请 A 在 Adapter 中处理真实 ID、字段来源、scope、上述错误/分页/事件差异；不得用演示状态弥补真实缺口。

## UIAPI-001 增量交付

在完整 dd98168 交付之上增加独立测试支持提交（SHA 在 UIAPI-001 回复中固定）；Rust 代码版本不变。frontend 若仅集成 ce46c6f，需要先取得 b8955ca/dd98168 的 HTTP Server fixture，再取得本增量；不要复制未提交文件。

启动命令不变。manifest 新增 `saved_plan`、`controls.interrupted`、`controls.resumable`。后者为真实 paused Batch，已验证一次 resume 不重复已完成图，保留 2 completed / 1 pending 和 available action 给前端 E2E。stop 字段仍为 outcome_unknown 场景；stopping 是实时短暂状态，initial Trace 是实际响应，不能静态伪造。

244 条真实 HTTP 请求及同库重启检查通过；UIAPI-001_TRACE.json 提供完整 trace 路径、脚本 hash、Plan 身份、Stop 回执、resume URL 与预算。新增 `HTTP_ADAPTER` 末节明确 `builder_operations.items[].session.builder_proposal`、工具记录与空任务首次 Send 的创建语义。没有通用助手文本；页面 mount 不得写虚构 journal。
