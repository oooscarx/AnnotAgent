# README 事实表

初始工作区/文档分支基线：`eaeae4cf8ee92bded9afb5577d2c9b70bc112de0`。
产品交付基线：`d3220cb`（main）。本轮已读集成快照：`1b1d9de16d5181e249967d4e8e47b977a4b645d3`。
**最终对外集成 SHA 尚未由 Frontend 1 确认；此稿可评审，不是发布批准。**

下表所有路径相对仓库根目录。核验以源码阅读、留存文件与本轮实际截图为限，不将他人测试称为本轮重跑。

| 拟用措辞 | 对应源码/证据 | 核验与边界 | 来源 SHA | 能否公开 |
| --- | --- | --- | --- | --- |
| 从原始图片到训练数据包的视觉数据 Agent | crates/annotagent-core/src/dataset_delivery.rs；crates/annotagent-export/src/training_package.rs | 定位与实现范围一致；不保证全自动 | d3220cb | 是，附 Alpha/预设限制 |
| 图片范围、标注目标和训练用途需补齐 | core/dataset_delivery.rs；web/src/agent-ui/DeliveryIntent.tsx | 三个 Task-owned slots，补齐/保存不等于授权；不声称自然语言总能自动提取 | d3220cb；1b1d9de | 是 |
| LLM 规划与视觉执行分开 | crates/annotagent-core/src/model_profile.rs；application Builder | 注册模型能力与独立绑定存在；模板与 Runtime 保底不能冒充 LLM 生成 | eaeae4c；d3220cb | 是 |
| 真实足球样例执行过 Qwen 与 EfficientSAM | docs/execution/PROVIDER_FAILURE_RECOVERY.md（历史 recovery 分支） | 保存样例执行记录；非当前版本端到端验收；无精度基准 | 6819b4d | 附来源与限制 |
| 样例反馈、对象审核、整图确认分开 | crates/annotagent-application/src/task_delivery.rs；web/src/agent-ui/DeliveryReview.tsx | TEST 图保留未确认状态；完整真实人工保存截图未获得 | d3220cb；7d17054 | 是，不能宣称真实案例审核完成 |
| 完整包仅 Ultralytics YOLO detection v1 | crates/annotagent-core/src/dataset_delivery.rs；crates/annotagent-export/src/training_package.rs | 读到唯一预设；28 条目 TEST ZIP 已只读 hash/CRC 检查；未跑官方训练器 | d3220cb；7d17054 | 是 |
| 标注文件有 Native/COCO/YOLO/LabelMe | crates/annotagent-export/src/lib.rs | 受 Schema/格式兼容性限制，不等于均支持完整包 | eaeae4c；d3220cb | 是 |
| 自动打包需准确预授权与服务端触发 | Backend 1e2bac5；mainline HTTP_BINDINGS | 工程师报告新增：最后整图审核或审核后授权可入队一次；尚待最终集成确认 | 1e2bac549f52ed98c2f9fbec9ed9f288f3e05251 | 暂不写成对外已完成 |
| 停止不保证远端退款/立即中断 | crates/annotagent-server/src/conversation_stop.rs；后端 handoff | settled checkpoint 可在条件满足时继续；in_doubt 不自动重试 | eaeae4c；Backend ML-012 回复 | 是 |
| 数据在本地，远程 Provider 接收调用输入 | docs/SECURITY.md；provider/openai_compatible.rs | 本地保存不等于不外传，原图元数据未承诺清除 | eaeae4c；d3220cb | 是 |
| JSON 上下文可归档导入 | docs/contracts/agent-ui-v1/UIAPI-018_CONTEXT_ARCHIVE.md | 独立分支已实现，但未纳入当前集成主稿承诺；归档不会恢复授权/预算/执行 | eaeae4c | 暂不列为本次集成能力 |
| Rust 主控，Web 交互 | Cargo.toml；runtime/engine.rs；web/src/agent-ui/http.ts | 已读代码 | eaeae4c；d3220cb | 是 |
| Rust 1.98.0、Node 22.12+、npm 源码启动 | rust-toolchain.toml；web/package-lock.json；apps/annotagent/src/main.rs | 本轮 npm ci/build 通过；未从本分支启动 Rust 服务，不声称端到端启动通过 | eaeae4c | 命令可列，限定验证范围 |
| MIT | Cargo.toml | 声明 MIT，但根目录无 LICENSE 正文 | eaeae4c；d3220cb | 只报告缺失，不补选许可 |

P0 已交首屏及标题结构；P1 已生成实际 README 和素材；P2 最终版本截图/同任务真实正式审核仍待证据；P3 本地近似预览与文档提交可以完成，发布仍须集成方确认。
