# 最终契约差异与集成请求

- CR-B01 已实现：轻量 navigation/task-navigation、exact Task workspace/thread。Thread 仅返回真实有归属用户记录；calls/Builder 等业务对象由 snapshot 提供。A 用明确 tool/system/object 来源显示，不能拼出假助手回复。
- CR-B02 已验证/补强：既有 Plan tool permission、精确批准与 FIFO；新增实际默认 Agent ID 冻结和并发检查。
- CR-B03 已实现：精确 Run SSE 补发、去重 ID、cursor gap、resync_required。尚无 Conversation 的统一全事件日志；journal 分页和 snapshot 重拉仍需要保留。
- CR-B04 已实现：六组安全 Settings 视图与未来预算 PATCH CAS。旧 PUT 兼容；不新建业务设置系统。
- CR-B05 部分：核心 owner/not_found 和 feedback stale 有 code；复杂 legacy 校验仍可能 generic invalid_request，不应自动重试。
- CR-B06 保留边界：无任意 Folder/OS 浏览、无 generic cancelled/unknown resume；仅真实 checkpoint 能继续。不会把未知收费自动重发。
- CR-B07 用量范围：现有 Task/Project call ledger 与 model active-probe usage 是不同统计范围；没有统一跨全部 Runtime/Provider 的金额/Token range-pagination DTO。A 不得将 probe 合计显示为全系统费用。历史无可信价格时显示未知。
- CR-B08 Queue 是持久、有序、显式授权 POST 驱动。恢复 saved authorization 后用相同原请求继续；不会因 GET/重连后台自动批量派发，也不新增常驻 scheduler。

这些是实际支持边界，不修改 SHARED_CONTRACT。若集成要求跨所有实体的统一有序事件或统一会计分页，需要另行设计现有账本的聚合，不在 JSX 中补虚构状态。

## 联调支持交付（2026-09-09）

- CR-B09 已完成：显式 TEST HTTP 启动器，复用现有外部模型 fixture + 真实 Server/SQLite/CSRF/授权/停止/恢复。生产 Registry 不新增 Mock。命令见 INTEGRATION_ENVIRONMENT.md。
- CR-B10 已完成：HTTP_ADAPTER.md 按 UI 字段逐项映射，明确 Send GET wrapper、Queue 内层 sequence、Run history wrapper、Stop 顶层回执及实际 credential POST。批准不是通用 token，revision/command ID 分实体保存。
- CR-B11 确认保留：Thread 只有真实用户消息；模型结构化决策与系统回执不是通用助手聊天消息。前端无需也不得编造完成话术。
- 本轮没有产品功能扩展、队列 scheduler/统一账本/Agent 架构改动；完成交接后只响应前端的具体集成问题。唯一合并和接线由视觉通过后的前端 Agent 在独立集成分支执行。

- UIAPI-001 已交付：精确 paused/resumable 与 interrupted 测试场景；saved_plan 身份映射；首次实际 Send 建 Task 的 Adapter 说明。没有新增业务引擎或修改真实 Runtime 语义。见 UIAPI-001_TRACE.json。

- UIAPI-003 已交付：真实 bbox terminal review candidate + 未回答 HumanRequest，manifest 提供候选/图像尺寸/feedback revision/answer 示例；实际保存与重试、同库重启已验证。外部 TEST 模型提供 30 秒在途窗口，另有可重复准备的手动 Stop 场景和验证命令。真实 stopping 回执最终到 outcome_unknown/in_doubt；不承诺 stopping 持续时长，不修改生产取消行为。

- UIAPI-004 已修复：Queue preview 与实际 admission 共用人工输入门禁，409 提前表达拒绝；新授权事务拒绝不留下 grant。旧冻结授权回答后可按原 ID/Consent 恢复（有效期/原 scope/FIFO 仍约束），已有未知回执不重发。真实 HTTP 与重开 SQLite 回归见 UIAPI-004_TRACE.json；不新增派发引擎。

- UIAPI-008：补持久 Schema 调用阶段、本地计时和安全类型化失败原因；保留未知结果与同ID不重发。调查确认 Provider 无模型文本增量协议，本轮只保留整包结果，不增加伪 streaming。详见 UIAPI-008_PROGRESS.md。

## UIAPI-010 Model Profile CAS

PATCH `/api/model-profiles/:id` accepts optional `expected_revision`; atomic stale edits return 409 `model_profile_revision_conflict` with expected/current revision. Every successful HTTP edit appends a revision, including metadata-only edits. Success remains an unwrapped ModelProfile. `revision` is still not a request field. Exact examples, compatibility and scope: [UIAPI-010_MODEL_CAS.md](UIAPI-010_MODEL_CAS.md).
