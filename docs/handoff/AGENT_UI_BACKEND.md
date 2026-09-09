# Agent B 后端交接

BASE SHA：c41b281b49252d520117029d39611865133798af。
分支：codex/agent-ui-backend；worktree：/Users/oscar/Documents/my_workspace/AnnotAgent-backend。
已提交实现 HEAD：1df67f8（B2）；B3 交付本文件、最终格式化和示例测试。最终分支 HEAD 用 `git rev-parse codex/agent-ui-backend` 获取，亦在交付消息中报告。

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

如集成阶段需要空白隔离 HTTP 服务（此命令未执行；不产生 mock 运行）：

```sh
backend_fixture=$(mktemp -d /tmp/annotagent-agent-b.XXXXXX)
CARGO_TARGET_DIR="$PWD/target" cargo run --offline -p annotagent -- serve --workspace "$backend_fixture" --port 8792
```

8792 被占用时选其他空闲端口，不杀已有进程；该 CLI 的非测试构建不启用 built-in mock Registry，因此真实推理验证仍用上述 fake-provider Rust 测试，不用 CLI 自动 probe。HTTP API 不依赖前端 dist，本轮无需安装 Node 包或构建/修改 web。

验证结果：全 workspace 741 passed、0 failed、6 原有 ignored（包含示例 DTO 校验）；最终 HTTP server lib 57 passed、1 ignored。全 workspace build、严格相关 Clippy、全 workspace fmt check 通过。详见 TEST_MATRIX.md。

## 停止/继续/队列证据

TRACES.json 的两条 HTTP Stop Trace 证明请求持久化后 stopping，worker 本地结算后 interrupted 或 outcome_unknown；相同命令还原同一选中目标，预算前后均保留 1 次已保留调用。停止不意味着远端退款。

Queue Trace：两个并发 reservation 只有一次 Admitted、一次 Existing；先处理原 FIFO 项，第二项取消后重启不复活；累计 used_calls=2。

Batch Trace：暂停后关掉应用再重开，继续相同 Batch checkpoint，终态 100 completed、0 remaining，恰好 100 个 child Run，累计 usage 与持久 ledger 相同；此为 mock 测试，不是实际模型精度/计费证据。

迁移无需新 SQL schema；可选 JSON 字段与兼容策略详见 MIGRATION.md。

## cross_boundary_requests

仅 A 在用户确认视觉并授权后集成。本分支没有 merge/cherry-pick 前端，没有 push/remote 修改，没有 reset/rebase/amend，没有修改 web、设计、前端 types/package 或用户原工作区文件。请 A 在 Adapter 中处理真实 ID、字段来源、scope、上述错误/分页/事件差异；不得用演示状态弥补真实缺口。
