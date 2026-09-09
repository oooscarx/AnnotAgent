# 兼容与迁移

本轮无需新增 SQL migration；复用既有 schema 初始化/迁移。新索引 API 为只读投影。

- Send receipt JSON 添加可选 resolved_agent_model_id；serde default 接受历史缺失字段。新发送原子保存解析的默认 Agent ID；旧回执不回填。不改原 command payload，不改变旧幂等比较。
- Settings revision 为规范序列化 Settings 内容的 SHA-256 字符串，重启稳定。PATCH 要求 expected_revision；旧 PUT 仍可不传，单进程内全部设置写入共享锁。多进程同时写同一 workspace 不在支持范围，应继续独立数据库/单实例运行。
- Settings 安全读取显式 `?view=agent-ui`，旧 DTO 兼容保留。业务新 UI 应用安全视图；不要把 legacy settings_path 或完整 provider config 放入导航。凭证沿用独立 Registry credential endpoint。
- 任务仍由 Conversation 下的既有 conversation_tasks 标识；每 Project 一个 Conversation。旧未绑定 journal message 不强行归入 Task。
- stable_project_id 延用现有基于规范项目目录的 UUIDv5；改 display_name 不改变身份，搬动 workspace/项目物理目录不属于本轮支持的重命名，不应由 UI 实现。
- Run SSE 的 UUID cursor 使用已有 run_events.event_id；排序用既有数据库 sequence。支持跨进程重启重连，无新事件存储。老全局 SSE 仍 live-only。
- Stop 旧字段保留；normalized_state 与 resume 为附加字段。finished 不推断业务成功，null 要从被引用 Operation 读取实际终态。
- 部分错误添加稳定 code；复杂 legacy invalid_request 仍是 400，Human feedback sequence 冲突为 409。前端按 code 展示并重新读取当前 scope，不按文案自动批准。
- 不做数据导出/清理/Registry迁移，不改 Geometry Safety，不修改任何旧结果或已发布 Workflow。
