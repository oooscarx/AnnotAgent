# Http Adapter 字段对照（联调支持）

本文件按实际响应字段接线；复杂 consent 使用 preview 返回值，不从 DTO 文本猜字段。`C=/api/projects/{project_id}/conversations/{conversation_id}`，`T=C/tasks/{task_id}`。所有 UUID、hash、revision、cursor 原样保存；project_id 是 route 字符串，project_owner_id 是真实所有权 UUID，title 仅展示。

## 身份、消息和状态

|UI 字段/动作|HTTP 与 JSON 路径|处理规则|
|---|---|---|
|Project key / route / label|GET `/api/navigation`：`items[].project_owner_id / project_id / title`|owner 作稳定 key；route 用于 URL；`group_id=null`，不虚构目录|
|Conversation|navigation `items[].conversation_id`；GET `/api/projects/{p}/conversations` 的 `conversation_id`|null 表示尚未创建；显式 POST 同一路径创建；每项目一个 Conversation|
|Task key / title / version|GET `C/task-navigation`：`items[].task_id / title / schema_revision`|`state` 仅 `state_scope=task_activity` 的观察；不代替子操作状态|
|用户消息|GET `T/thread`：`items[].id / sequence / message.input.text / message.input.image / message.input.reference`|`role=user, source=persisted_user_message`；此端点没有 assistant 消息|
|未归属旧消息|GET `C/messages?after=0&limit=100`：数组 `[].input`|不能按当前选中 Task 伪造归属|
|任务快照|GET `T/workspace`：`task.input.id, task.input.source_message_id, task.input.schema_revision`|`consistency=individually_committed_records`，不是跨所有对象原子 revision|
|系统执行卡片|workspace `calls, builder_operations, sample_operations, processing_operations, journey_consents, human_requests, queue`|保留真实对象 ID 和来源；不能转为“助手说已完成”|
|动作按钮|workspace `actions.{send,stop,approve,resume}.{available,reason}`|reason 同时用于解释可用条件及禁用原因；`approve.available=false` 表示先选 exact preview，并非所有批准不支持|
|具体继续按钮|workspace `resume_actions[]` 的 `kind,id,checkpoint_ref?,available,reason,method,url`|只调用 available 的精确动作；仍以 POST 服务端复核结果为准|
|发送命令 ID|POST `C/send` 的 `message.id`|没有额外通用 command_id；未知结果原 ID+原完整 payload 重试或 GET `C/send/{id}` → `{input,receipt}` 或 null|
|发送回执|`task_id, disposition, message, mode?, agent_model?, resolved_agent_model_id?`|Send 只保存/归属/排队，不自动执行。缺少 resolved ID 表示旧或未配置记录，不能显示当前模型为历史模型|
|模式|Send `mode=plan\|execute`|模式不授予 Sample/Publish/Batch 权限；这些都需独立 consent|
|Agent 模型选择|GET `C/agent-model` → `revision,model_profile_id`；POST → `request_id,expected_revision,model_profile_id`|null 代表使用默认；只影响下一请求；发送可附 `agent_model` 作为 CAS。当前 Workflow 视觉绑定不会改变|

分页：navigation 的 `cursor` 是上页 `next_cursor` owner UUID；task-navigation/thread 的 `cursor` 是整数 message sequence。默认从头读，`limit=1..100`，`next_cursor=null` 为结束。Queue 用 `after=<sequence>`，每页最多 100，返回数组，无 next_cursor：取末项 `receipt.message.sequence` 继续直到空页。Conversation journal 同样 `after`，不要把 navigation cursor 传给 journal。Snapshot queue 仅第一页。去重按实体 ID；翻页不是同一时间点的锁定快照。

## 精确批准、停止、人工和队列

|UI 需求|实际字段/端点|不可省略的语义|
|---|---|---|
|审批 token|无统一 `approval_token`；schema/queue 使用 `call_id,model_id,scope_hash,request_hash`（queue）、`previous_grant_id,maximum_calls,expires_at,allow_unknown_cost`|必须从当前对应 preview 获取完整 scope；hash 不是 CSRF token，也不是长期授权|
|Builder 批准|GET `T/builder-preview`；POST `T/builder-operations`：`selection,repair,previous_grant_id,scope_hash,expires_at,allow_unknown_cost`|selection 含 operation/model/schema ID 和 revision，不能修改后复用 hash|
|Journey 批准|GET `T/journey-preview?...` → `consent`；POST `T/journey-consents` 原 consent；POST `T/journey-consents/{consent.id}/execution` `{}`|明确确认 unknown cost 后设置 `allow_unknown_cost`；含 schema_proposal 时该内层也须确认。保存 consent 与开始执行是两步|
|Journey 展示|GET execution：`record,dispatch,schema,builder,sample,answer_delivery`（部分可 null）|`dispatch.status=settled` 只表示协调器交接/结算；仍需看 `sample.status` 与 `sample.assistance.status`，不能提前展示样例完成|
|Stop command|POST `C/stop-requests`：`id,text:"停止",image:null,reference:{scope:"stop_request",task_id}`|id 即用户停止消息 ID；多目标时 POST `.../{id}/select` 的 `{target:{kind,id,task_id}}`，必须来自 frozen targets|
|Stop 回执|顶层 `message,status,targets,selected_target,observation,dispatch_error,normalized_state,resume`|没有 `request` 包装层；`status=cancel_requested` 只是请求已保存；GET 同一路径只观察，不 signal|
|停止显示|`normalized_state=stopping / interrupted / outcome_unknown / idle / null`|`observation.state=cancel_pending / cancelled / unknown / finished` 分别映射前 3 状态及 null；finished 需查原操作，不能等同 success|
|停止信号失败|`dispatch_error != null`|命令已保存；可重发原命令重试 signal，不能换目标或宣称停止完成|
|Queue 项|GET `T/message-queue`：`input.message.id,receipt.message.sequence,status,receipt,planning_call_id,cancelled_at`|Send supplement 自动持久入队；不是常驻自动调度；取消 POST `.../{message}/cancel` `{}`|
|Queue 批准/派发|GET `.../{message}/schema-preview`；POST `.../{message}/schema-proposals` exact consent|FIFO 是服务端 reservation 约束；不得跳过未处理队首。仅规划 schema，不批准视觉执行|
|Queue 丢响应恢复|GET `.../{message}/schema-authorization` → 原 consent 或 null|有原 consent 时重发原 POST；已完成返回原 receipt、在途不重复调用、in_doubt 不自动重发；取消项重启不复活|
|人工问题|GET `T/human-requests`：`input.{id,question,reason_code,sample_test_id,image_id,content_hash,outcome_id?,addition_id?,expected_feedback_sequence,resume_checkpoint_ref},status,deferred,answer`|用原样本/图像/对象引用打开画布；deferred 与 terminal 分开|
|人工答案命令|POST `.../{input.id}/answer`：`{answer:{revision_id,sample_test_id,image_id,sequence,reason,outcome_id?,addition_id?,corrected_value?,corrected_label?,note,created_at},journey_consent_id?}`|sequence = expected_feedback_sequence+1；revision_id 即幂等键；重试保持时间、文本、坐标等全部字段|
|人工保存后继续|响应是保存的 HumanRequest，可含 `journey_resume`|答案先持久化再继续；resume 失败不代表答案丢失。没有 journey consent 只作本地修订，不授权模型调用|
|正式处理|GET `/api/projects/{p}/processing-preview?draft_id=...&sample_test_id=...&limit=...`；POST `/processing-operations`|POST `{request_id,selection:{draft_id,sample_test_id,limit},expected_revision,authorization_fingerprint}`；token 是实际 preview 的 fingerprint，冻结工作流/图像/预算|
|导出|GET `/api/projects/{p}/export-readiness`；POST `/export` `{format:"native",background:true,conversation:{id,conversation_id,task_id}}`|`conversation.id` 是 export command ID；样例通过不等于正式数据可导出；必须展示 readiness 的真实阻塞|
|导出轮询/下载|GET `T/exports/{id}` → `{active,job:{id,error,result,...}}`；`job.result.delivery`|active=false 仍可能失败；仅 result.delivery 有效时请求 `/api/projects/{p}/exports/{delivery.id}/download`；历史 GET `T/exports?before=<UUID>&limit=100` 是数组|

## 安全与错误

先 GET `/api/session`，保留 HttpOnly session cookie，取 `csrf_token`。写请求带 `x-annotagent-csrf`。敏感写入先 POST `/api/session/privileged-confirmation` `{action:"PATCH /api/settings",confirmed:true}`，取 `confirmation_token` 作为 `x-annotagent-privileged-confirmation`；一次性且 action 精确到 method/path，不含 query。Provider credential 是 **POST** `/api/providers/{id}/credential`（非 PUT），仅写入；active-probe 是显式 POST，不能在页面刷新时触发。

同源 UI 用相对 `/api` 路径。跨端口开发 UI 需要前端自己的同源代理，或把既有构建传给隔离启动器 `--web-dist`。服务未增加 CORS 豁免；不要关闭 CSRF/Origin 校验来接线。

|HTTP / code|Adapter 行为|
|---|---|
|400 `owner_mismatch` / `not_found`（部分旧路由保留 400）|停止该对象请求，重读导航/选择；不能改用当前任务兜底|
|409 `settings_revision_conflict` + current_revision|重新 GET 安全 Settings；显示冲突，不默默覆盖|
|409 `send_model_selection_changed`, admitted=false|尚未入账；刷新模型偏好后让用户明确重新发送|
|409 `stale_revision` + current_revision|人工反馈基线已改变；重读反馈/问题，不能把新 revision_id 当重试|
|400 `invalid_request` / 422 反序列化拒绝|显示 error（422 可能为文本）；无稳定业务 code 时保留原响应，不按英文文案自动执行|
|403 安全拒绝|检查 same-origin、cookie、CSRF 和精确一次性 token；不是模型调用失败|
|429 `mutation_rate_limited`|中间件尚未执行 mutation；退避，敏感动作重取单次 token。其他未知结果不能盲目换 ID|
|409 `event_cursor_gap`|按响应 snapshot_url/history_url 补快照并建立新游标|

## 刷新、继续与用量范围

Conversation 没有统一事件游标。选中/重连/操作返回后重读 task workspace、thread/journal 增量，以及当前操作 GET；在途按需轮询。不要假设 Run SSE 覆盖 Conversation/Queue/HumanRequest 全部变化。

Run 历史 GET `/api/runs/{run}/events` 返回 `{events:[...]}`，不是裸数组。

Run SSE：`GET /api/events?run_id=UUID` 无游标从持久首事件开始；`Last-Event-ID: <Event UUID>` 优先于 query `last_event_id`，补发严格晚于该事件的记录。保存 SSE `id` 去重；流内 `resync_required` 后重读快照。全局 `/api/events` 仅 live，不支持补发。Export SSE `T/exports/events` 是另一套整数 sequence，事件 `export_snapshot/export_changed`，不能与 Run UUID 游标混用。

继续边界：同一进程管理的 paused Run 可恢复 control；重启后不能声称任意 Run 可以复活。持久 paused Batch 走既有 checkpoint，复用已完成 child Run 与剩余预算。HumanRequest 只有保存的当前答案且未 deferred、无未知调用时才提供继续；本地修订 applied 后再次 resume 是幂等本地结果，后续推理仍需授权。cancelled、in_doubt/outcome_unknown 不能通用 resume；远端已接收而本地无法确认时只能等待/核对原回执，不重新消耗预算赌重试。Stop 不保证取消远端计费。

Settings 六组路径均在 `sections` 下：general 是浏览器偏好；providers 为安全列表引用与配置布尔值；agent_models 为 Registry/defaults 引用且 effective=next_request；vision_plugins 为现有安装实体引用；data_privacy 为本地 scope 和清理需 preview 的提示；usage_budget.future_run_budget 是未来 Run 默认预算。PATCH `/api/settings` 发 `{expected_revision:response.revision,budget:response.sections.usage_budget.future_run_budget}`。revision 是不透明 Settings hash，不是数字。Task budget 是既有调用账本；Project conversation-call-limit 是另一组 revision CAS；`/api/model-profiles/{id}/usage` 为 active-probe usage，**不是全系统模型费用**。没有统一金额/Token 时间范围分页；未知费用保留 null/未知。

## Thread 的证据来源

用户消息来自 SQLite Conversation journal，经 source_message/send receipt/reference.task_id 验证真实归属。模型回复目前保存为结构化业务决策和证据（例如 Schema decision/rationale、Builder tool trace、Feedback decision、Sample report），不持久化成通用 assistant chat message。`T/thread` 当前确实只有 user。系统状态来自 calls/operations/stop/export 等持久回执，不来自模型自由文本。前端可以展示带来源标签的业务卡片；不能生成“任务已完成”等假助手回复，也不能把 fixture 的 TEST 模型内容称为真实商业模型回答。

## UIAPI-001：已存 Plan、文本与空新任务

`mode=plan` 的 Send 回执表示用户选择的模式，**不是生成好的 Plan**。当前没有通用 `/plans` chat 实体。已存可审核方案沿用 Builder Session/Workflow Draft：

|UI 内容|确切对象字段|
|---|---|
|Builder 历史容器|`workspace.builder_operations.items[]`，不是 `builder_operations[]`|
|操作 ID / 状态|`item.operation.id / item.operation.status`|
|引用实际方案|`item.operation.evidence.draft_id / draft_revision / draft_content_hash / session_id`（evidence 可 null）|
|方案内容|`item.session.builder_proposal.draft`；包含 `id,revision,nodes,edges,annotation_schema` 等现有 Draft 字段|
|方案解释和限制|`item.session.builder_proposal.rationale[] / warnings[] / alternatives[]`，以及 `estimated_model_calls_per_image,estimated_cost_tier,estimated_latency_ms`；是保存方案的结构化字段，不能宣称完整聊天助手原文|
|候选计划|`item.session.plan_candidates[]`；允许为空，不能用 `builder_proposal` 伪造一个 candidate ID；`selected_candidate_id` 可 null|
|下一步提示|`item.session.next_action`；系统/Builder 的动作提示，不是模型回答|
|执行记录文字|`item.session.steps[].result.display_summary`，以及 `tool_name,sequence,success`；是工具回执|
|模型调用证据|`item.session.model_calls`、`workspace.calls` 与业务 endpoint 的 decision；按来源呈现结构化记录，不拼接成 assistant chat message|

Session/evidence/proposal 可能尚未生成，字段必须检查 null；对象 ID/状态真实存在时再显示卡片。Sample、Publish、Processing 是独立事实：Builder 的 `outcome=draft_ready_for_human_review` 和 `published=false,samples_tested=false` 不能解释为执行完成。后续 Draft 被编辑时，区分该操作保存的 `evidence.draft_revision` 与当前工作副本 revision，批准前重新取 exact preview。

空新任务的建立顺序：

1. 页面 mount、导航、新任务按钮可以展示本地空白 Composer；只做 GET，不创建假用户消息、不调用 append/send 自动填充 journal，也不生成可持久化的伪 Task UUID。
2. 用户首次实际发送时，先使用 navigation/GET Project Conversation 得到真实 conversation_id；尚无 Conversation 时显式 POST `/api/projects/{p}/conversations`，读取响应 `conversation_id`。
3. GET `/api/projects/{p}/goal` 取得 `revision`；生成并保存此次实际用户命令的 message.id，POST `C/send`：`{message:{id,text,image:null},schema_revision:goal.revision,mode:"plan"}`。创建新 Task 时省略 task_id；需要绑定图像时使用已导入的真实 image 引用。
4. POST 响应的 `task_id` 才是 Task 路由/导航 key；`message.input.id`、`message.sequence` 是已存用户 journal 的身份。需要重试时保留原 ID/完整 payload，GET `C/send/{message.id}` 返回 `{input,receipt}` 或 null。后续明确发给现有任务的 supplement 带真实 task_id。
5. 重新 GET task-navigation/thread/workspace。只发送不会产生助手回复或已存方案；用户批准对应 planning preview 后，服务端才生成结构化方案。

fixture manifest 中 `plan_task_id` 是仅 Send 的待批准任务；`saved_plan` 则指向主 Task 已保存的 Builder proposal。二者必须分别验收，不要求前端凭空渲染 Plan 文本。

UIAPI-002 再确认：`workspace.builder_operations` 的 JSON 形状是 **`{"items":[...]}`**，初始空值为 `{"items":[]}`，不是数组；使用 `workspace.builder_operations.items`。每项仍是 `{operation,schema_id,schema_revision,session}`，具体 Plan 字段见上一节。

## UIAPI-003：bbox 与 Stop 的验收字段

HumanRequest 本体不承诺有 `kind` 字段。bbox 类型从它引用的 Sample terminal candidate 的 `outcome.value.kind=bounding_box` 获取，原框为 `outcome.value.rect`；用 request.input.outcome_id 精确匹配候选，不能从当前模型名推断。候选可能在 `report.samples[].projection.review_candidates[].candidate`，也可能在 final_candidates，不能强行只读最终已接受列表。

答案：`corrected_value={kind:"bounding_box",rect:[x,y,width,height]}`，各值归一化到源图；request 的 expected_feedback_sequence+1 用于 answer.sequence。`revision_id` 是此次答案的幂等 ID；并发基线改变要重读，不能生成新 revision_id 绕过冲突。保存只产生实际反馈/修订，不等于通过 Geometry Safety 或正式接受标注。

Stop POST 的 `normalized_state=stopping` 与后续 GET 的 `outcome_unknown` 是两个真实观察时刻。fixture 的外部 30 秒延迟只保证可在途发 Stop，不承诺 stopping 动画持续时长；在途取消后 calls[].status=in_doubt 才是未知结果账本状态。页面不能把 model delay 当作假的服务端状态计时器。

## UIAPI-004：人工输入门禁与原授权恢复

`GET T/message-queue/{message_id}/schema-preview` 和 `POST .../schema-proposals` 在 Task 有 pending HumanRequest 或 pending image-class review 时返回 HTTP **409**：

```json
{"status":409,"code":"human_input_pending","error":"invalid conversation operation: Task is waiting for human input; no model call was admitted","admitted":false,"suggested_action":"answer_human_then_retry_same_command"}
```

未回答的 Schema clarification 同样阻塞，code 为 `schema_clarification_pending`。deferred 但仍 pending 的问题也不能跳过。Adapter 应显示阻塞并刷新 `T/workspace` / 对应问题；成功 preview 才有可供审阅的 scope。`admitted:false` 只表示此次拒绝没有进入模型调用，不表示 Task 没有历史调用。Send supplement 仍可以保存到队列，不代表允许立即规划。

服务端在 preview、POST scope 校验、新授权的数据库事务及实际 call reservation 复用同一门禁。事务拒绝会同时回滚新 grant/queue authorization，不消耗调用预算。preview 是当前快照，不是未来执行保证；所有其他 owner、FIFO、精确 scope、预算和过期检查继续生效。

历史版本已留下 `authorized` 且没有 call 的项，以及授权写入后才出现人工问题的竞态，保留冻结 grant，不撤销、不自动派发。恢复步骤：

1. GET `T/message-queue/{message_id}/schema-authorization` 保存原 Consent（含 `call_id, model_id, scope_hash, request_hash, previous_grant_id, maximum_calls, expires_at, allow_unknown_cost`）。
2. 通过对应真实 answer API 保存答案，刷新 workspace 确认所有人工门禁已解除；仅关闭弹窗或 deferred 不算回答。
3. POST 原 `schema-proposals`，使用**同一 call_id 和完整原 Consent**。仍要求授权未过期、未撤销、原 scope/model/provider 未变、预算/FIFO 可用。不能把新 preview 的 previous_grant_id 混进旧 Consent；过期或范围改变需要重新审阅，不能自动换 ID。
4. 若该 call 已有 receipt，精确重试返回原 receipt，不再调用模型；`in_doubt` 仍是未知结果，不能借重试重发。若尚无 receipt，回答后原授权可首次 reservation；同 ID 再次提交仅恢复同一结果。

回归：`http_queue_human_check.py --enable-fixture --manifest <fresh TEST workspace>/manifest.json`。该脚本会回答 seed 的 bbox 问题并创建一次真实测试补充；每次使用新 seed。它验证 pending preview、preview/POST 间新增问题、无授权/预算变化、回答后原 Consent 成功、重复 POST 只有一个 call。存储回归额外验证已冻结授权在人工回答及 SQLite 重开后仍能以原 ID reservation，重复 reservation 为 Existing。无 SQL migration，无自动清理历史授权。

## UIAPI-008：真实调用阶段与原因

现有 calls/workspace/Schema POST 回执增量字段为 `started_at,completed_at,duration_ms,stage,failure`（均可 null）。stage 仅记录 reserved/provider_request/response_received/settled；failure 是安全 `{stage,category,http_status}`。completed_at 是本地结算，不能将 in_doubt 当成功；未知结果不自动重发。完整枚举、旧记录兼容及流式调查见 [UIAPI-008_PROGRESS.md](UIAPI-008_PROGRESS.md)。本轮无文本 delta/SSE 新接口。

## UIAPI-010 Model Profile CAS

PATCH `/api/model-profiles/:id` accepts optional `expected_revision`; atomic stale edits return 409 `model_profile_revision_conflict` with expected/current revision. Every successful HTTP edit appends a revision, including metadata-only edits. Success remains an unwrapped ModelProfile. `revision` is still not a request field. Exact examples, compatibility and scope: [UIAPI-010_MODEL_CAS.md](UIAPI-010_MODEL_CAS.md).
