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
