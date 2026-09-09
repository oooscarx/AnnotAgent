# B0 change requests

1. CR-B01：新增只读轻量导航、task snapshot/thread 投影；复用现有 UUID 和 journal，不创建第二套实体。
2. CR-B02：Plan mode 仅已记录，核验直接执行权限；保持独立精确 consent，不让 send mode 成为授权。
3. CR-B03：全局 live SSE 不保证补发；复用持久 run events，补 event IDs/replay/gap。
4. CR-B04：Settings legacy DTO 暴露路径且没有 CAS；补安全输出/受控写入，前端适配新增字段。
5. CR-B05：旧 Conversation 的 400 不区分 stale/foreign；补稳定 code，保持旧 error/status。
6. CR-B06：任意 cancelled/unknown 的通用 resume、任意文件夹、统一金额用量分页不是既有能力；不伪造。后续报告可支持的具体恢复对象和限制。

不改共享契约；集成由 A 在用户批准后接线。
