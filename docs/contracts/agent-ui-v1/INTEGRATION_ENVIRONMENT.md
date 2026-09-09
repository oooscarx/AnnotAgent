# 隔离真实 HTTP 联调环境

用途：前端 Http Adapter 的真实服务联调。复用 `annotagent-server::router`、LocalApplication/SQLite、既有启动恢复与 `annotagent-e2e-fixture` 外部模型 HTTP transport；Python 仅管理两个进程并调用 API，不实现应用运行时。生产二进制没有新入口，也不自动开放 Mock。

## 启动 / 测试

在 `codex/agent-ui-backend` 所在 checkout 执行（本机 `/Users/oscar/Documents/my_workspace/AnnotAgent-backend`）：

```sh
python3 crates/annotagent-e2e-fixture/support/http_fixture.py --enable-fixture
```

默认系统分配两个 loopback 空闲端口。若前端需固定代理目标：

```sh
python3 crates/annotagent-e2e-fixture/support/http_fixture.py --enable-fixture --port 8792 --provider-port 8793
```

端口占用直接失败，不杀进程；明确禁止 8787。需要 Python 3.9+、现有离线 Cargo cache；启动器只调用 `cargo test -p annotagent-server --lib --offline --no-run --message-format=json` 和 `cargo build -p annotagent-e2e-fixture --offline`。target 固定当前 checkout/target，不读取原工作区数据库。

可加 `--web-dist /absolute/path/to/existing/dist` 同源托管前端已有构建，只读该目录，不构建、不改动前端 worktree。否则是 API-only；前端自行配置同源 `/api` 代理，保留 cookie/CSRF，不放宽 CORS。

自动真实 HTTP 验证后退出自己启动的进程：

```sh
python3 crates/annotagent-e2e-fixture/support/http_fixture.py --enable-fixture --smoke
CARGO_TARGET_DIR="$PWD/target" cargo test -p annotagent-server --lib --offline
CARGO_TARGET_DIR="$PWD/target" cargo clippy -p annotagent-server --all-targets --offline -- -D warnings
cargo fmt --all --check
```

显式 ignored 的 `tests::integration_http_fixture` 是前台测试服务，不是普通单测；不要运行所有 `--ignored`（仓库其他 ignored 测试不属于此次联调）。不调用 CLI 的系统密钥路由。服务采用 InMemorySecretStore，测试字面量 credential 为 `TEST-integration-only`，只发往本启动器 loopback provider。所有 HTTP 响应都有 `x-annotagent-fixture: external-model-only`，终端及 manifest 明确 TEST 标记。

## 数据库、重启与证据

每次新启动创建系统临时目录 `TEST-agent-ui-*`，验证 marker `FIXTURE_ONLY`。实际数据库为该目录 `.annotagent/history.db`，图片/Settings/导出均在该目录。不会删除数据，Ctrl-C 仅停止启动器自己的两个子进程。

启动成功打印 `manifest.json` 绝对路径，包含 base_url/provider_url、项目/Conversation/Task/模型 ID、pending_request_id、plan_task_id、stop.request_url、run_id、export 和证据路径。不要硬编码示例 UUID。`HTTP_TRACE.json` 记录真实请求体/响应（省略 credential/CSRF token 和上传二进制）；`server.log/provider.log` 保留日志。manifest 的 backend_sha 是当次 checkout HEAD。

用打印的 workspace 重启同一数据库：

```sh
python3 crates/annotagent-e2e-fixture/support/http_fixture.py --enable-fixture --workspace /absolute/temp/TEST-agent-ui-xxxx
```

复用原端口和模型地址；只恢复该 fixture 精确地址的字面量测试 credential 到内存。启动仍经过真实 `recover_answers`，不直接写业务状态。冻结的 Provider 地址不能变更，因此 restart 禁止更换 provider-port。API 端口可用 `--port` 改为其他空闲端口。已有已完成结果、队列、反馈不重建、不重新 seed。

无用户编辑的 smoke 环境可立即验证重启不重复工作：

```sh
python3 crates/annotagent-e2e-fixture/support/http_fixture.py --enable-fixture --workspace /absolute/temp/TEST-agent-ui-xxxx --smoke
```

比较 calls/budget/queue/answers/thread/stop/Run events 与初始 snapshot，验证 SSE 仍能从原 event ID 补发，输出 RESTART_TRACE.json。交互编辑过的 fixture 用普通启动即可；其 snapshot 不再等于 seed，不能把这种差异当恢复缺陷。本验证覆盖已结算状态的进程重启与无重复派发，不冒充所有在途崩溃点覆盖；既有 B3 Batch/outbox 恢复测试仍是对应故障点证据。

## 可供前端操作的真实场景

- 主 Task：真实 Journey/schema/Builder/sample、Processing/child Run、正式 native 导出；用户消息与对象回执可读，留下一个 pending HumanRequest。另有已回答问题，其重复提交及冲突已经验证。
- Plan Task：只保存 Send(mode=plan)，无授权、无自动推理，可由前端获得 exact preview 后批准。
- Stop Task：模型 transport 延迟期间保存停止命令，观察 stopping→outcome_unknown；有原选中目标、回执、预算。没有假设远端退款，也不允许通用恢复未知调用。
- Queue：真实 supplement FIFO，一项已授权完成、一项取消，重发原批准得到相同回执；无自动 scheduler。
- Settings：安全六组读取、预算 CAS 正常和 stale 冲突；Provider probe 仅对本地 fixture 明确授权。
- SSE：正式 Processing 生成的真实 Run，持久 events 补发与错误 cursor 检查；终端投影来自 result-summary。

原生导出需要正式数据 ready；仅 sample 成功时 readiness 仍会拒绝，不能通过假 accepted 数据绕过。fixture 的确定性分类样例经真实 Processing 提交后可导出；不是模型质量验证。HTTP_TRACE 中模型结构化输出为 TEST transport 生成，应用的授权、DB、停止、恢复、导出均为真实代码。

前端继续操作仍遵守 HTTP_ADAPTER.md 的 exact consent、CSRF、revision 与对象归属规则；fixture 没有权限豁免。模型选择下一请求生效，不能更改已经冻结的视觉 binding。没有通用自然语言 assistant 消息来源，卡片须标记模型结构化决策或系统回执。

## UIAPI-001 新增的精确控制场景

启动命令不变，种子增加两个 TEST Project，仍在同一隔离 SQLite，使用原有 `e2e-slow-sample` 外部 HTTP fixture。无需安装或引入真实 Provider。

- `manifest.saved_plan`：已存 Builder proposal 的 Task、workspace URL、operation/session/draft ID 与 JSON 路径；与只有 Send 的 `plan_task_id` 区分。
- `manifest.controls.interrupted`：真实 Processing 在途时发送 Stop，保留 `request_url,initial,final`；最终 `normalized_state=interrupted`，底层 Batch cancelled，不提供通用 resume。
- `manifest.controls.resumable`：真实 3 图、单并发 Batch；pause 后完成 1 图，再通过 HTTP resume/pause 完成第 2 图，留下 1 图未处理。`resume_action.available=true`，可直接按其 method/url 继续；已完成 child Run 和累计预算保留。
- `manifest.stop`：原有 schema 在途取消后的 outcome_unknown 场景。stopping 是真实短暂过程，`initial` 记录该时刻的实际 HTTP 响应，不能让生产 API 永久停在伪造 stopping；重跑 seed 可再次观察。

所有状态均由真实 API/worker 产生，无 SQL 插入业务状态、route.fulfill 或第二套调度器。新增 PNG 只是在已有合成测试图中插入有效 TEST 元数据，形成 3 个可区分的测试输入；不会改写原图或用户数据。seed 需要等待真实延迟与必要的 mutation rate-limit 退避，通常约一分钟。重启验证也比较新增场景的 Batch checkpoint、预算及 resume_actions。

## UIAPI-002：TIME_WAIT 与端口占用

`free_port` 使用 loopback + SO_REUSEADDR + bind/listen，允许已关闭连接的 TIME_WAIT 地址复用，仍拒绝现有 listener；不使用 SO_REUSEPORT、不杀进程、不关闭安全中间件。探测结束即释放 socket，不能消除探测到子进程绑定之间的竞争；实际 Rust/Tokio listener 绑定失败仍应报错。不要通过延迟数十秒或清理进程绕过问题。

回归命令：`python3 crates/annotagent-e2e-fixture/support/test_http_fixture_ports.py`。它在自动端口制造真实 TCP TIME_WAIT，先确认旧裸 bind 失败，再确认新探测与 listener 能立即重启；另检查活动 listener 拒绝及 8787 禁止。

28d5ec3 已加入 SO_REUSEADDR；UIAPI-002 独立增量增加 listen 探测和回归测试。尚停在 dd98168 的集成可使用本目录 `UIAPI-002_PORT_FIX.patch`（仅 free_port，包含完整修复，不依赖 UIAPI-001 新场景），然后运行本次交付的回归脚本；无需复制未提交文件。已有 28d5ec3 时正常集成本增量即可，不重复应用完整 patch。

## UIAPI-003：bbox 保存和浏览器停止证据

默认启动现在增加 `manifest.bbox` 和 `manifest.manual_stop`，无需真实 Provider/模型安装。新 fixture 需要本次 `annotagent-e2e-fixture` 二进制；启动器会离线编译。

`bbox` 是独立 TEST Project，使用现有 VLM Detection 节点和同一个本地 OpenAI-compatible transport。默认 bbox 模板含通用 detector，因此 seed 通过正常 Workflow PATCH（If-Match revision）明确设置 VLM operation/Registry binding，再获取新 Sample preview 并单独批准；没有改生产模板或直接写 SQL。

|manifest.bbox 字段|用途|
|---|---|
|`task_root`|GET `.../workspace`、`.../human-requests`|
|`sample_url, sample_test_id, draft_id`|读取真实保存的 Sample 和终端投影|
|`candidate_id, source_artifact_id, terminal_candidate, projection_group`|真实候选身份与来源；几何待审核候选位于 `review_candidates`，不是正式已接受标注|
|`image_id, content_hash, image_dimensions`|真实图像引用、hash、像素尺寸，本合成图为 640×400|
|`feedback_url, feedback_revision_id, feedback_sequence`|seed 已真实保存一次 bbox 反馈并验证重复提交；当前 sequence=1|
|`pending_request_id, answer_url, answer_example`|留下的新未回答请求；示例答案 sequence=2，归一化框 `[0.1,0.15,0.12,0.2]`|

浏览器打开 `sample_url` 对应图像/候选，读取 HumanRequest 的 expected_feedback_sequence 和当前 feedback 后，通过 answer_url 提交 `{answer:...}`。`answer_example` 是可用的 TEST 请求体，UI 编辑时更新 rect/label/note；重试需保留原 revision_id、created_at 和完整内容。归一化顺序为 x/y/width/height，不是 x1/y1/x2/y2；像素框 `[64,60,76.8,80]` 对应示例归一化值。Rust f32 回读允许小数舍入，不比较 JSON 数字串字节相等。候选 ID（例如 detection-0）不是全局 UUID，必须连同 Task/Sample/image/Artifact 引用使用。

停止证据的最小步骤（所有写请求继续使用真实 session/CSRF）：

1. `manual_stop.status=prepared_not_started`。按 `manual_stop.start.method/url/body` 发起已存 TEST Journey。
2. 轮询 `manual_stop.wait_for_reserved_url`，等待 schema call.status=reserved。仅命名的 TEST 模型在外部响应前延迟 30 秒，提供稳定的在途窗口。
3. 在这 30 秒内按 `manual_stop.stop.method/url/body` 发 Stop。保存 POST 响应的 normalized_state/selected_target；真实初始回执为 stopping。
4. GET `manual_stop.observation_url` 和 `manual_stop.workspace_url`，观察最终 outcome_unknown 与 calls[].status=in_doubt。Queue 可照常 GET `task_root/message-queue`；不会因读取自动授权或重发未知调用。

**stopping 是短暂状态，不保证持续 30 秒。** 延迟的是外部模型；本地取消可能立即中断等待，两秒后的 GET 可能已经 outcome_unknown。前端应捕获真实 POST 回执与后续快照，不把旧回执伪装成当前状态、也不要求 fixture 延迟本地取消。最终未知状态持久、不可通用 resume，可用于浏览器稳定验收。

原场景使用过或 consent 超过 `expires_at` 后，保持 fixture 运行，准备全新的待启动场景：

```sh
python3 crates/annotagent-e2e-fixture/support/http_stop_scene.py --enable-fixture --manifest /absolute/temp/TEST-agent-ui-xxxx/manifest.json
```

输出新的真实 Task/consent/start/stop 请求与独立 `UIAPI-003_STOP_<task>.json`。不自动执行；加 `--verify` 才自动开始、停止并检查真实状态链和 in_doubt。该命令仅接受带 marker 的 loopback fixture，检查响应 TEST header；不接触 8787。保留原 `manual_stop` 场景不变，终端 JSON 中使用新的 scene 字段。过期/撤销不能换 ID 重发原模型调用，新 scene 是明确创建的新测试。
