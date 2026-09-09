# Agent UI v1 — 实际 HTTP bindings

B0 基线：`c41b281b49252d520117029d39611865133798af`。以下为源码核验，尚未声称新增能力通过测试。

`{C}=/api/projects/{p}/conversations/{conversation}`；`{T}={C}/tasks/{task}`。`p` 是现有项目 route ID，稳定 UUID owner 由服务端解析，绝不以 display_name 判断归属。

|逻辑操作|实际 method/path|请求/响应|核验结论|
|---|---|---|---|
|listNavigation|GET /api/projects?project_limit=100&project_offset=0|DashboardPageQuery → projects + paging + run summaries|已有项目分页；缺轻量树、Task 分页|
|getTaskWorkspace|GET /api/projects/{p}/conversations; GET {C}/tasks; GET {C}/task-selection|conversation_id nullable; ConversationTask[]; ConversationTaskSelection|每项目一个 Conversation、多 Task；缺聚合快照|
|listThreadItems|GET {C}/messages?after=0&limit=100; GET {C}/messages/{message}|ConversationMessage{conversation_id,sequence,input:{id,text,image,reference}}[]|真实 user journal；工具/结果须从各业务对象读取；缺 exact task thread|
|sendMessage|POST {C}/send; GET {C}/send/{message}|ConversationSendInput → ConversationSendReceipt|message.id 是 command_id；相同 ID 不同 payload 拒绝；mode 记录不等于授权|
|previewApproval / approveAction|GET {T}/schema-preview; POST {T}/schema-proposals; GET {T}/builder-preview; POST {T}/builder-operations; GET {T}/journey-preview; POST {T}/journey-consents|各 preview 返回 scope_hash/model/request/预算；显式 consent DTO 见源文件|已有精确 scope/expiry/budget；需核验 Plan 的直接调用边界|
|interruptOperation|POST {C}/stop-requests; POST {C}/stop-requests/{message}/select; GET {C}/stop-requests/{message}|ConversationMessageInput(reference=stop_request) / {target:{kind,id,task_id}} → {request,observation,dispatch_error}|已有 frozen targets；cancel_requested ≠ 停止完成；以 observation 为准|
|resumeOperation|POST /api/runs/{run}/resume; POST /api/batches/{batch}/resume; POST {T}/human-requests/{id}/resume; POST {T}/journey-consents/{id}/execution|按既有实体恢复 DTO；run/batch control 无通用 command DTO|已有 checkpoint 与预算；不支持任意 cancelled Operation 原地复活|
|queue|GET {T}/message-queue?after=0; POST {T}/message-queue/{message}/cancel; GET {T}/message-queue/{message}/schema-preview; POST {T}/message-queue/{message}/schema-proposals|ConversationQueuedMessage[]; cancel {}; Consent → ConversationCallReceipt|已有持久 inbox/冻结 send/单次授权；核验 FIFO 派发与恢复|
|list/selectAgentModel|GET /api/providers; GET /api/model-profiles; GET/POST {C}/agent-model; GET/PUT /api/agent-model-bindings|ConversationAgentModel{revision,model_profile_id}; SelectConversationAgentModel{request_id,expected_revision,model_profile_id}|已有 next request CAS；不可改 Workflow 视觉绑定|
|uploadImage|POST /api/projects/{p}/image-upload?name=TEST.png; GET /api/projects/{p}/images|raw image bytes → imported project image metadata|已有显式导入；不调用 Provider|
|getArtifacts|GET /api/runs/{run}/result-summary; GET /api/runs/{run}/debug-summary; GET {T}/sample-operations; GET {T}/processing-operations|RunResultSummary / debug lineage / persisted operations|已有终端投影；中间阶段不得计作重复目标|
|answerHumanRequest|GET/POST {T}/human-requests; POST {T}/human-requests/{id}/answer|ConversationHumanRequestInput / HumanAnswer → 保存的反馈及恢复状态|已有 revision/outbox/stale 防护；答案先保存再派发|
|settings.general|GET/PUT /api/settings|Settings + configured/persisted flags；展示偏好前端本地|已有 legacy 设置；缺 revision CAS 与安全白名单摘要|
|settings.providers|GET/POST /api/providers; GET/PATCH/DELETE /api/providers/{id}; PUT /api/providers/{id}/credential; POST /api/providers/{id}/check; POST /api/providers/{id}/active-probe|安全 profile DTO；凭据 write-only；probe 显式 consent|已有本地 session/CSRF/privileged confirmation；读取不 probe|
|settings.agentModels|GET /api/model-profiles; GET/PUT /api/agent-model-bindings; GET/POST {C}/agent-model|registry model capabilities + preferences|已有；模型不可用理由来自兼容检查|
|settings.visionPlugins|GET /api/plugins; GET /api/model-instances; GET /api/model-bundles; POST /api/model-installations|plugin/model asset 独立状态；安装显式请求|已有安装/许可/引用保护；本轮不安装|
|settings.privacy|GET /api/projects/{p}/management/usage; POST /api/projects/{p}/management/preview; POST /api/projects/{p}/management/actions|ManagementPreview / ManagementRequest / ManagementReceipt|已有引用保护、回收站、占用；缺 workspace 安全摘要|
|getUsage / settings.budget|GET {T}/budget; GET/POST /api/projects/{p}/conversation-call-limit; GET /api/model-profiles/{id}/usage|Conversation budget / ProjectCallLimit + expected_revision / usage|共享 calls budget；金额未知不得转成 0；没有统一范围用量分页|
|subscribe|GET /api/events?run_id={run}; GET /api/runs/{run}/events; GET {T}/exports/events|RunEvent / persisted history / export SSE|全局 SSE 仅 live，未设置 event ID；需补 scoped replay 与 gap 提示|

权限与错误：所有写操作沿用本地 same-origin/session/CSRF；敏感路径需 single-use privileged confirmation（详见 security.rs）。现有错误为 `{error,status}`，Registry 部分带 `code/references/suggested_action`；多数 Conversation stale/foreign 错误仍是 400，Adapter 不得当成功。需要新增稳定错误字段的缺口见 CHANGE_REQUESTS。未知 HTTP 结果重试保持原 message.id/request_id/operation_id 和完整 payload。部分旧 Runtime control 并无 command_id，不能宣传通用 exactly-once。

批准以具体 endpoint 的 consent 为准，不存在 authorize_all 布尔值；未知费用需明确 allow_unknown_cost。读取不得新增授权或执行。状态来自各实体：stop 的 request.status 是命令状态，observation 才是执行观察；in_doubt 映射 outcome_unknown；队列独立展示。

DTO 权威源：`crates/annotagent-storage/src/conversation_{send,tasks,agent_model,stop,human_requests,queued_planning,project_budget}.rs`；`crates/annotagent-server/src/conversation_*.rs`；路由完整清单见 ROUTES.json。

已有回归测试（B0 仅定位，执行结果后补）：conversation_http_journal_is_owned_idempotent_and_csrf_protected、conversation_agent_model_http_is_passive_owned_and_versioned、followup_queue_is_atomic_ordered_owned_and_cancellation_is_terminal、registry_api_keeps_credentials_write_only_and_records_confirmed_probes、planning_only_http_advisor_does_not_create_a_sample_test、run_api_uses_stable_project_ownership_across_duplicate_names_and_rename、processing_retry_recovers_created_batch_without_scope_reauthorization_or_execution、batch_api_exposes_durable_progress_and_controls。

## B1 已实现的只读投影与兼容扩展

- `GET /api/navigation?limit=1..100&cursor=<owner UUID>` → `{workspace_id,items:[{project_id,project_owner_id,title,group_id:null,conversation_id:null|UUID}],next_cursor}`。稳定 UUID 顺序，仅读 Project Schema 和 Conversation 身份，不加载 Artifact/history。无游标时首页；失效游标需重新读取。
- `GET {C}/task-navigation?limit=1..100&cursor=<sequence>` → `{items:[{task_id,source_message_id,schema_revision,created_at,sequence,title,project_owner_id,conversation_id}],next_cursor}`。现有 source message sequence 的 keyset 分页。
- `GET {T}/thread?limit=1..100&cursor=<sequence>` → `{items:[{id,role:"user",source:"persisted_user_message",sequence,task_id,conversation_id,project_owner_id,message}],next_cursor}`。只返回有明确任务归属的真实用户消息；未绑定任务的旧消息继续从 Conversation journal 读取，不能编造绑定。模型工具回执在 snapshot.calls/builder_operations 中；不得伪装为助手自然语言。
- `GET {T}/workspace` → exact task、calls、active operations、pending/history HumanRequests、queue 首页、Builder/Processing/Journey objects、budget、actions 与 thread_url。`consistency=individually_committed_records`：跨多个已有存储查询的投影，不承诺跨实体原子快照。
- `GET /api/settings?view=agent-ui` → `{revision,sections:{general,providers,agent_models,vision_plugins,data_privacy,usage_budget}}`。安全白名单，六组已有接口引用；无密钥、host path、endpoint 参数。
- `PATCH /api/settings` → `{expected_revision:string,budget:Budget}`，保存未来 Run 默认预算；返回安全设置视图。冲突 `409 settings_revision_conflict/current_revision/suggested_action=reload_settings`。不覆盖已冻结运行/Task ledger。旧 PUT 接受可选 expected_revision，原客户端兼容；都共享进程内写锁。预算 PATCH 无通用 command_id，未知结果通过 GET 比较已保存版本，不能换预算盲重试。
- `GET /api/events?run_id=<UUID>&last_event_id=<Event UUID>`，也接受 `Last-Event-ID` header（优先）。已有 event_id 为 SSE id；按持久 sequence 分页，250ms 检查已保存新事件。无 cursor 从该 Run 首事件重放。不存在或跨 Run 游标返回 `409 event_cursor_gap`，带 snapshot_url/history_url。流中存储错误/游标缺口发 `resync_required` 后关闭。无 run_id 的旧全局流仍是 live-only，不能请求恢复。
- Stop 输出仍是原 ConversationStopRequest 的顶层字段（不是 request wrapper），另带 observation/dispatch_error/normalized_state/resume。`cancel_pending→stopping`、`unknown→outcome_unknown`、`cancelled→interrupted`；原 `finished` 只表示已经结束，normalized_state=null，需查原实体的终态。

B1 HTTP 验证：navigation_snapshot_thread_keep_real_ownership_without_execution、safe_settings_are_passive_and_budget_patch_rejects_stale_revision、sse_reconnect_replays_exact_run_and_reports_cursor_gap；完整 server lib 55 passed / 1 ignored。

## B2 控制语义与冻结修复

SendReceipt 新增可选 `resolved_agent_model_id`：新消息在发送时被动解析并保存实际规划模型 ID；与 observed preference 的比较和 message/queue 写入同一事务。改变偏好会让竞态发送拒绝，旧 command 重试还原原 receipt。不会修改任何视觉绑定。显式后续请求不得覆盖已冻结模型；旧/未配置回执无此字段，仍需模型设置与精确批准，不回填伪造历史。注册 Profile 的 revision/Provider/config/预算仍由既有批准 scope_hash 冻结。

Task 导航新增 `state/state_scope=task_activity`。这是当前活动观察（含待人工、未批准队列、在途规划/Sample/Batch），不是一个覆盖所有对象终态的项目状态。快照新增 `sample_operations/resume_actions`；只有真实 paused Batch 或已保存待继续 HumanRequest 在已结算调用条件下提供 available resume。Queue、HumanRequest、每个 Operation 必须独立展示。

所有 legacy bad_request/not_found/forbidden 输出新增通用 code、suggested_action、current_revision。已类型化的 Conversation/Task 不存在与跨所有权分别返回 `not_found/owner_mismatch`，保持历史 400 兼容。Human feedback sequence 竞态返回 `409 stale_revision/current_revision`。其他复杂旧校验仍可能为 `400 invalid_request`，不要把 message 文案用作授权判断。

Plan 工具曝光同时检查 planning_only 和 maximum_dry_runs；强制 model tool-call 仍经过原有 Plan permission denial。现有 Builder 始终 planning_only；Sample/Publish/Batch 必须走独立精确授权。

并发队列测试在真实 SQLite reservation 上启动两个线程：仅一个 Admitted，另一个 Existing。FIFO、累计 calls、取消、restart、旧 source working-copy 校验继续复用现有事务。派发是显式 POST 驱动，页面 GET 不派发，未承诺后台无人值守自动 worker。

Trace：TRACES.json 为测试产生的数据；停止中/未知/已中断预算不变；Batch restart/resume 100 图片只有 100 个 child Run，累计账本与历史用量一致；queue 已完成项返回 Existing，未执行 cancelled 项重启不复活。Mock 费用不能作真实模型计费或精度证据。

Schema：SCHEMAS.json 覆盖常用命令/投影；复杂 Builder/Journey/Human DTO 的精确定义见 DTO_INVENTORY.json 与对应 Rust serde attributes。EXAMPLES.json 为 TEST 安全示例，scope_hash 必须实际 preview 获取，示例授权不可用于生产。
