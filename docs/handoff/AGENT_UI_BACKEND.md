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

## UIAPI-002 独立端口修复

根因是旧 Python free_port 未设置 SO_REUSEADDR，与实际 listener 的地址复用行为不一致；无 LISTEN 不等于不存在 TIME_WAIT。现使用 SO_REUSEADDR + bind/listen，保持活动 listener/8787 拒绝及子进程最终绑定检查。3 个 socket 回归及真实带 web-dist Ctrl-C/同库同端口重启通过。

如果集成仍是 dd98168，`docs/contracts/agent-ui-v1/UIAPI-002_PORT_FIX.patch` 只包含完整 free_port 修复，可独立应用而不引入 UIAPI-001 新场景。若已集成 28d5ec3，则按提交顺序取得本增量即可。HTTP_ADAPTER 已再次明确 workspace.builder_operations 是 {items:[...]}。

## UIAPI-003 bbox 与手动停止交付

本增量只改现有外部模型 fixture、Python seed/support 与文档，生产 Server/Runtime/Geometry Safety 不变，无数据库迁移。完整交付 SHA 在 UIAPI-003 回复中固定。

默认启动命令不变，manifest 新增 `bbox`（candidate/source artifact、640×400、当前 feedback revision/sequence、pending_request_id、answer_url/answer_example）和 `manual_stop`（已授权未开始的start/stop请求）。bbox 终端几何安全候选可能在 review_candidates，不能当作正式已接受结果。

`http_stop_scene.py --enable-fixture --manifest <TEST manifest>` 可重复创建浏览器专用场景；加 --verify 才自动执行停止验证。仅 TEST 外部模型延迟30秒，保证可在途发Stop；本地取消可能立即结算，stopping只取真实POST回执，最终in_doubt/outcome_unknown稳定持久，不允许伪造响应或强行维持stopping。

完整seed278条HTTP及同库重启通过，bbox保存/幂等/冲突、手动Stop实际状态链验证通过，详见 UIAPI-003_TRACE.json 与 INTEGRATION_ENVIRONMENT.md 的 UIAPI-003 字段/步骤。接口支持状态继续保持。

## UIAPI-004 集成修复

Queue preview/POST 现在对 pending human 返回 409 `human_input_pending`（Schema clarification 为 `schema_clarification_pending`），附 admitted=false 和 answer_human_then_retry_same_command。授权事务复查并回滚新 grant；已有冻结授权保留，回答后原 ID/Consent 可重试，仍须原 scope/有效期/FIFO/预算成立。已有 call 回执只回放，未知结果不重发。无迁移、无自动调度改造。

真实 HTTP 验证 preview 拒绝、preview 后新增 human 导致 POST 拒绝且授权/预算不变、回答后原 Consent 成功、重复 POST 仅一个 call；存储测试验证旧冻结授权跨 SQLite 重开恢复。Storage/Application/Server 共 397 passed、0 failed、3 原有 ignored，相关 all-targets 严格 Clippy 通过。详见 HTTP_ADAPTER.md UIAPI-004 与 UIAPI-004_TRACE.json。最终 SHA 在 UIAPI-004 交付回复固定。

复现：先按 INTEGRATION_ENVIRONMENT.md 启动全新 TEST seed，再执行 `python3 crates/annotagent-e2e-fixture/support/http_queue_human_check.py --enable-fixture --manifest <TEST workspace>/manifest.json`。脚本消费 bbox pending 问题，每次回归用新 seed。未读真实密钥、未付费、未碰 8787/前端 worktree；自有测试进程已正常停止，隔离数据保留。提交后继续接口支持状态，只响应具体集成问题。

## UIAPI-008 调用进度、原因与 streaming 调查

已在 codex/agent-ui-backend 增量修复，基线334f671；核验 main83d2adb 的 Rust/migrations 与该基线一致，无合并前端或 main。call receipt 增加 started_at/completed_at/duration_ms/stage/failure，持久真实 reservation、Provider 调用、完整响应与结算边界；安全 failure 只含阶段/类别/HTTP状态。handler中断/重启仍为 in_doubt，同ID不重发。Builder operation是独立DTO，此次没有扩展其字段。

迁移0058仅新建metadata表；旧call保留created_at但缺失end/failure为null，不能追回此前被丢弃的原因。未访问用户报告的robocup-ball数据库或调用。Provider没有现成文本/tool delta协议，本轮未新增streaming；stream:true在发送前拒绝，新增读取body阶段取消。详见 UIAPI-008_PROGRESS.md、UIAPI-008_EXAMPLES.json、UIAPI-008_TRACE.json。

验证：563 passed、0 failed、3原有ignored（Core/Provider/Storage/Application/Server）；全workspace all-targets严格Clippy、fmt通过。隔离HTTP seed278请求，42个call回执观察/23个call身份，真实provider_request→settled/in_doubt、typed cancellation与计时验证通过。测试启动器已退出自有进程，数据保留。未接触8787/8788、真实密钥/服务/数据库、前端worktree；无push/merge。最终提交SHA在交付回复固定，继续接口支持状态。

## UIAPI-009 审计/具体缺口交付

现有 API 无持久 workspace history cutover；Run 有分页但按 updated_at 排序，Pipeline/Trash 无分页，management 无 scope 绑定。已交付 UIAPI-009_GAP.md：明确标记未实现的 GET/preview/一次性确认契约、冻结ID排除成员、SQL分页、Trash与级联保护、原引用继续可读及隔离验收清单。此为用户允许的 concrete gap specification，不能宣称切换已完成。没有运行/伪报新功能测试，也未读写真实数据库；仅源码审计与文档检查。UIAPI-009 仍阻塞，后续须实现文档所列事务与API，前端不能用浏览器时间替代。

## UIAPI-011 Builder executable projection

已补 PipelinePlanCandidate 的可空 label_pipeline 持久化、Registry revalidation 和 materialization；保留 Schema-bound受控模板、单label目标、授权Provider/native模型选择，绑定同步到composition。已有 Sample guard/审批/Geometry Safety不变，旧操作回执不改写、不自动重跑。

验证：Core/Application/Storage/Server 515 passed、0 failed、3原有ignored；全workspace all-targets严格Clippy与fmt通过。专项测试保存重读typed候选，materialize保留Profile/ModelInstance ID及Geometry Safety；禁用segmenter后候选仍变为不可用。真实隔离HTTP seed278请求通过，实际classification与VLM Detection Builder草稿均保留LabelPipeline。native是测试manifest合同验证，不冒充真实安装权重推理。

契约/边界：UIAPI-011_BUILDER.md、UIAPI-011_TRACE.json。无SQL迁移；旧candidate缺字段仍为null，不伪造旧DAG projection。现有refinement生成器只对精确单label目标参与，本轮不扩多label调度。集成端负责新的精确授权与真实VLM+SAM验证；此后端未读改真实workspace/旧Draft，未安装/付费/操作8787或8788/推送/合并前端。最终SHA在交付回复固定。

## UIAPI-010 Model Profile CAS delivery

PATCH adds optional expected_revision and transaction-atomic compare/append. Stale edits return safe 409 expected/current revision; every successful HTTP PATCH advances revision, including metadata/no-op edits. Old revision and Published frozen snapshots remain unchanged. Legacy omitted-field requests remain accepted with no claim of stale-browser protection. No migration or frontend changes.

Isolated tests: Storage/Server 243 passed, 0 failed, 2 existing ignored; includes independent-connection two-writer race, reopen/old revision/Published snapshot preservation, and HTTP contract/legacy requests. Workspace all-target Clippy with -D warnings, fmt check and diff check passed. Commands and exact contract: docs/contracts/agent-ui-v1/UIAPI-010_MODEL_CAS.md. No paid calls, keys, real workspace mutation, service restart, push or merge. Final SHA is in delivery response.

UIAPI-009 remains blocked/unimplemented. Prior 8cccca5f4f45ba4203ae77c4d74c79a29fa86323 delivered a concrete gap specification only (UIAPI-009_GAP.md); no scope establishment or runtime acceptance is claimed.

## UIAPI-011 round 2: salvage scope repair

Conversation supplies Schema-only target and admitted Registry through input, but outer target=None. Discovery-limit refresh replaced that with global project input, skipping deterministic refinement synthesis. Refresh now retains admitted Schema/goal/task/label/constraints and intersects fresh Registry identities with the admitted sets. Fresh availability still governs; no model identity is invented or newly authorized.

The isolated regression saves a real Conversation Schema, runs a scripted four-turn planning-only Builder without path-discovery calls, and checks actual segmentation binding, LabelPipeline, Schema preservation and static validation. Removing the preservation call reproduces the missing-segmenter failure. Contract and integration boundary: docs/contracts/agent-ui-v1/UIAPI-011_SALVAGE_SCOPE.md. Real VLM/SAM inference remains the integration owner's verification; no paid call or service update was performed here.

Per the integration owner's priority message, UIAPI-009 is paused with uncommitted implementation/tests/migration0059 preserved and excluded from this delivery. UIAPI-010 CAS remains separately committed as 210091a17b1d4ff7b5fe0a306cb9100f1b548aea. Final isolated test totals and delivery SHA are reported in the delivery response.

Isolated delivery-only worktree verification: Application 158 passed / 0 failed / 1 existing ignored; workspace all-target Clippy with `-D warnings`, fmt and diff checks passed. Pre-fix regression failed specifically because the segmenter node was absent.

## UIAPI-011 round 3: manual drafting budget and native inspection

Schema-bound FromScratch/ImproveExisting now persist deterministic Registry candidates before Provider turns. Manual current-Draft state no longer disables model-turn/step-budget fallback. Fresh scoped Registry revalidation and static validation still apply. When there is an in-progress draft, fallback materializes a separate new ID and records the preserved ID in planning events; no authored graph is overwritten. RepairDraft/ResolveBindings are excluded.

inspect_models_batch now declares and resolves admitted ModelProfile plus expert/model-instance IDs. Profile payloads stay unchanged; expert payloads include kind/model_id/manifest and an explicit inspect_model_contracts/model_binding hint, never bind_model_profile for a native instance. Unknown/unadmitted models fail closed.

Isolated delivery-only verification: Application 159 passed, 0 failed, 1 existing ignored; workspace all-target Clippy -D warnings, fmt and diff checks passed. Eight-turn manual drafting regression verifies separate statically valid segmentation alternative and preserved partial graph. Disabling budget fallback reproduces the failure (no separate alternative). Four-turn discovery, native-instance inspection/materialization and unavailable-model tests pass. Contract: docs/contracts/agent-ui-v1/UIAPI-011_BUDGET_FALLBACK.md.

No paid calls, real artifacts/weights, service updates, Schema output-tool changes, history_scope/0059, frontend edits, push or merge in this delivery. Integration owner must verify actual VLM/SAM artifacts; static planning is not execution evidence. Final SHA is fixed in delivery response.

## UIAPI-012 durable installation commands

Implemented optional client command UUID, transaction-atomic reservation before dispatch, exact five-selection-plus-installation-root scope, same-command replay of original operation and changed-scope409. Existing active Bundle/Plugin duplicate guard now persists across lost responses/process state. Progress/terminal receipts persist; command/direct operation GET are passive. Orphan running receipts project unknown and cannot implicitly restart. Last32 list entries do not bound command lookup retention. Compatibility and unknown-state handling: docs/contracts/agent-ui-v1/UIAPI-012_INSTALL_COMMANDS.md.

Isolated delivery-only verification: Storage/Server246 passed,0failed,2existingignored; actual HTTP concurrent202/200 same ID, lost-response replay, changed-scope409, process-state recreation, passive orphan/unknown lookup, and separate-connection atomic winner. Full workspace all-target Clippy -D warnings, fmt and diff checks pass. Additive migration0060 is independent of pending0059. No Web changes, real data, service restart, paid calls/large weights, push or merge. Final SHA is in delivery response.

UIAPI-009 remains unfinished; its uncommitted history_scope/0059 work is preserved and excluded from this delivery. UIAPI-010 CAS integration acceptance is acknowledged as complete from the frontend's verified report.

## UIAPI-013 owned Workflow Draft static validation

Delivered GET `/api/workflow-drafts/{id}?project_id=...` and POST `/api/workflow-drafts/{id}/validate` with required project_id/expected_revision. Static response carries the actual Draft identity/revision/content_hash and Core WorkflowValidationReport; wrong owner404, stale409, invalid request type422. Metadata-only catalog construction bypasses credential resolution and Provider/HTTP Worker construction while reusing existing Core/grammar/geometry checks. It performs no inference, plugin execution, sample-test, publication or writes. A valid static report does not assert live model readiness or publication approval. Full adapter contract: docs/contracts/agent-ui-v1/UIAPI-013_STATIC_VALIDATION.md.

Delivery-only TEST checkout: Application159 unit +1 integration, Server60 passed; 3 existing ignored. Full workspace all-target Clippy -D warnings and fmt/diff checks passed. Regressions cover owner/revision/type failures, real type mismatch, no network, malformed endpoints, no sample/Run, unchanged Draft/Published snapshots, and metadata-only versus runtime authentication behavior. No migration or Web changes. No real workspace mutation, service restart, paid calls, push or merge. Commit SHA is supplied in the delivery message. Pending UIAPI-009 changes remain excluded.

## UIAPI-014 typed sample-feedback localization repair

Existing authorized RepairDraft now prepares a controlled typed local search on the existing copy for selected poor_boundary/wrong_target terminal bbox feedback. Reuses frozen Schema and existing VLM/native/profile bindings; Expand→Crop→local VLM→Project→coverage→existing segment/mask/geometry→mandatory Review. No domain/model-brand substitution, threshold relaxation, added allowance, sample invocation or publication. Candidate and actual feedback provenance persist. Old template replacement is rejected; an unchanged, statically valid prepared graph survives planning-budget termination as draft_ready_for_human_review with the actual stop reason. Failed Provider/cancellation remains failure/cancellation. Exact API/Journey chain and quality limits: docs/contracts/agent-ui-v1/UIAPI-014_LOCALIZATION_REPAIR.md.

Isolated Application/Runtime210 passed,1 existing ignored; workspace all-target Clippy -D warnings, fmt/diff checks passed. SQLite feedback/copy + scripted Builder regression preserves original sample/Draft/Schema/model bindings, rejects revoked model/duplicate local repair/template overwrite and verifies budget finalization without inference. No new runtime/API/migration, Web edits, paid calls or user services. Actual image quality/SAM dispatch remains for frontend-authorized integration verification. This increment excludes history_scope/0059 and UIAPI-012/013; commit SHA is supplied in delivery response.


## UIAPI-009 implemented: immutable workspace history scope

See `docs/contracts/agent-ui-v1/UIAPI-009_HISTORY_SCOPE.md` for the executable contract and isolated acceptance tests. Passive GET, preview plus explicit privileged idempotent establishment, persisted identity membership, SQL-filtered scoped pagination and management preview/action guards are implemented. Direct references and Published/annotation data remain unchanged. Migration0059 only adds scope tables; no real workspace scope was established. Frontend must send the returned scope ID explicitly on both lists and management requests.


## UIAPI-014 second-round repair admission

Implemented exact Applied-source Builder and running-Sample ledger admission with other images still pending, preserving existing grants/limits/stop and sample leases. Saved typed local recovery is revalidated and reused without another pass. Real HTTP three-image TEST regression passed; contract and commands: `docs/contracts/agent-ui-v1/UIAPI-014_REPAIR_ADMISSION.md`. This increment contains no publication/0061 changes; those remain isolated and uncommitted.


## UIAPI-014 native frozen versions and exact publication

Implemented owned `GET /api/projects/{project_id}/workflows/{workflow_id}/versions/{version}` returning the actual complete frozen version. Existing Draft publish accepts exact `{command_id,project_id,expected_revision,expected_content_hash}` and atomically saves publication plus replay receipt; no new inference/Run. Additive migration0061; old Published snapshots remain unchanged. Full API/field/error/migration contract: `docs/contracts/agent-ui-v1/UIAPI-014_PUBLICATION.md`. This is separate from repair admission commit220576f.


## UIAPI-014 sealed repair Journey Sample follow-up

Sample pending-human admission is narrowed to the exact active, sealed repair Journey/source, in addition to the existing running-operation identity, grant, lease and scope checks. Ordinary Sample cannot inherit the exception from an Applied repair copy. No new HTTP fields/routes: use existing explicit repair `journey-preview`/consent/execution. Real TEST HTTP covers ordinary Sample denied with zero calls, then sealed Journey Sample through the actual vision adapter with three images/zero failures and two unchanged pending requests. See `docs/contracts/agent-ui-v1/UIAPI-014_REPAIR_ADMISSION.md` for contract, replay command and retained trace. This increment contains no migrations, publication or frontend changes.

Follow-up verification: `cargo test --offline -p annotagent-storage -p annotagent-application -p annotagent-server`: **418 passed, 3 existing ignored, 0 failed**. Strict all-target clippy (`-D warnings`) and `cargo fmt --all -- --check` passed. The owned TEST fixture was stopped; its database and HTTP trace remain available.


## UIAPI-015 publication handoff (renumbered)

Executable delivery remains `7c7c8df0ff234477aff67ae48ab53b6ef91897bd`; reverified 8 isolated publication tests passed. Exact API and existing clone limitations: `docs/contracts/agent-ui-v1/UIAPI-015_PUBLICATION.md`. This handoff adds no runtime changes.
