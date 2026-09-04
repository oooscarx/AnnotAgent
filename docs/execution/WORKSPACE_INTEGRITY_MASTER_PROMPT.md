# AnnotAgent Project-Scoped Workspace Integrity Alpha

你现在是 AnnotAgent 仓库的长期主实现工程师。

本次任务不是增加新模型、插件、Agent Tool、视觉效果或页面数量，而是系统性修复以下问题：

> AnnotAgent 的 Project、Run、Batch、Review、Workflow Draft、Image 和 Artifact 所有权没有在 URL、API DTO、数据库标识和前端状态中保持一致，导致项目内操作跳入全局页面、刷新恢复错误、跨项目对象混淆、最终结果与中间 Artifact 混合、部分控件实际不可用，以及测试固化错误信息架构。

本次任务名称：

```text
AnnotAgent Project-Scoped Workspace Integrity Alpha
```

最终目标是：

```text
全局索引负责跨项目发现
Project Workspace 负责项目内完整工作
对象详情使用稳定 ID 表达所有权
URL 是可分享的持久上下文
服务器是业务状态真值
前端不伪造状态或结果
界面只暴露真实可执行能力
```

必须实际检查代码、复现问题、修改实现、执行测试、修复回归，并按 Milestone 创建独立本地提交。

不要只修改 CSS。

不要只增加 Breadcrumb。

不要继续用 `?project_id=` 假装 Project 子页面。

不要用新的局部 state 修补刷新问题。

不要在修复过程中增加新的模型、Skill、Provider 或 Plugin 功能。

---

# 一、不得破坏的现有架构

以下能力如果在当前源码中真实存在并有测试，必须保留：

* AnnotAgent 全局产品身份；
* Project-centric Guided Workspace；
* Label Schema 与 Workflow 分离；
* Workflow Draft；
* Static Validation；
* Sample Test / Dry Run；
* Published Immutable Workflow Version；
* Run 固定 Workflow Version；
* Dataset Batch；
* Checkpoint；
* Pause、Resume、Cancel；
* Artifact lineage；
* Cache；
* Replay；
* Review；
* Annotation Revision；
* Correction Memory；
* Geometry Safety；
* Improve Automation；
* Provider Registry；
* Model Profile；
* Rust Expert Model Plugin；
* `.annotmodel` Model Bundle；
* Native、COCO、YOLO、LabelMe 导入导出；
* TUI；
* GUI；
* SQLite 审计与历史；
* Token 和费用统计。

本轮不要重写：

```text
Agent + Skill 架构
Geometry Safety
Model Bundle
Rust Plugin Host
模型推理逻辑
RoboCup Ball Validator
```

只有在它们违反稳定所有权或状态恢复时，才修改其 DTO、路由或引用方式。

---

# 二、开始前必须核验仓库

首先执行：

```bash
git status --short --branch
git log --oneline -20
```

记录：

```text
当前分支
工作区状态
领先远程提交数
最近提交
```

随后安装并运行当前锁定依赖：

```bash
npm --prefix web ci
```

执行基线：

```bash
cargo fmt --all --check

cargo clippy \
  --workspace \
  --all-targets \
  --all-features \
  -- \
  -D warnings

cargo test --workspace --all-features
cargo build --workspace --all-features

npm --prefix web run typecheck
npm --prefix web test
npm --prefix web run build
npm --prefix web run test:e2e
```

如果某项因环境限制不能执行：

* 记录具体命令；
* 记录完整限制；
* 不得声称通过；
* 继续完成不依赖该环境的工作。

必须阅读：

```text
PRO_REVIEW_BRIEF.md
README.md

docs/PRODUCT_HIERARCHY.md
docs/GUIDED_EXPERIENCE.md
docs/RUN_AND_REVIEW_UX.md
docs/WORKFLOW_MODEL.md
docs/WORKFLOW_RUNTIME.md
docs/API.md
docs/KNOWN_LIMITATIONS.md
docs/execution/

web/src/App.tsx
web/src/navigation.ts
web/src/navigation.test.ts
web/src/workspaceContext.ts
web/src/api.ts
web/src/types.ts
web/src/styles.css
web/e2e/guided-workspace.spec.ts

crates/annotagent-server/src/lib.rs
crates/annotagent-application/src/lib.rs
crates/annotagent-storage/src/lib.rs
crates/annotagent-plugin-host/src/package.rs
crates/annotagent-plugin-registry/src/lib.rs
migrations/
```

不要盲信文档中的完成声明。必须以源码、测试和浏览器行为为准。

禁止：

```text
git reset
git rebase
git commit --amend
破坏性 checkout
修改 Git remote
push
删除历史 Run
删除用户数据
使用或恢复任何对话中出现过的 API Key
提交模型权重
用 Mock 冒充真实模型
```

---

# 三、建立执行账本

创建并持续维护：

```text
docs/execution/WORKSPACE_INTEGRITY_MASTER_PLAN.md
docs/execution/WORKSPACE_INTEGRITY_STATUS.md
docs/execution/WORKSPACE_INTEGRITY_DECISIONS.md
docs/execution/WORKSPACE_INTEGRITY_ACCEPTANCE.md
docs/execution/WORKSPACE_INTEGRITY_DEFECTS.md
docs/execution/WORKSPACE_INTEGRITY_BLOCKERS.md
docs/execution/WORKSPACE_INTEGRITY_KNOWN_LIMITATIONS.md
```

`WORKSPACE_INTEGRITY_DEFECTS.md` 对每个问题记录：

```text
Defect ID
Severity
Verified / runtime-risk / UX
File and line
Reproduction
Impact
Root cause
Fix
Regression test
Commit
Status
```

`WORKSPACE_INTEGRITY_STATUS.md` 记录：

```text
当前 Milestone
已修复缺陷
正在处理
下一步
最近 Rust 测试
最近 Web 测试
最近 E2E
最近浏览器验证
最近提交
Release Blocking 剩余项
真实 Blocker
```

每完成一个 Milestone：

1. 更新状态；
2. 更新缺陷表；
3. 更新验收证据；
4. 执行对应测试；
5. 修复回归；
6. 创建独立本地提交；
7. 继续下一 Milestone；
8. 不等待用户确认。

---

# 四、核心产品不变量

所有修复必须满足以下不变量。

## 4.1 所有权不变量

每个持久实体必须有稳定的所有权 ID：

```text
Run.project_id
Batch.project_id
ReviewItem.project_id
ReviewItem.run_id
Image.project_id
Artifact.run_id
Artifact.image_id
WorkflowDraft.project_id
WorkflowVersion.project_id
SampleTest.project_id
```

任何关联都不得依赖：

```text
project_name
数组下标
显示名称
当前筛选器
localStorage
组件挂载顺序
```

名称只用于展示。

## 4.2 路由不变量

属于 Project 的对象，其 canonical URL 必须包含 Project ID：

```text
/projects/:projectId/runs/:runId
/projects/:projectId/batches/:batchId
/projects/:projectId/review/:reviewId
```

全局路由只作为跨项目索引：

```text
/runs
/review
```

对象详情不能通过查询参数伪装所有权。

## 4.3 状态不变量

```text
服务器持久化状态 = 业务真值
URL = 用户当前查看上下文
查询缓存 = 服务器状态的客户端镜像
localStorage = 非关键偏好
组件局部 state = 未提交编辑草稿
```

禁止用 localStorage 或局部 state 表达：

```text
当前 Run
当前 Project 所有权
当前 Published Workflow
当前 Review 决策
当前 Sample Test
```

## 4.4 结果不变量

Results 只显示：

```text
正式 committed annotations
当前 review-final candidates
合法 no-target 结果
```

Debug 才显示：

```text
所有中间 Artifacts
粗框
精修框
fallback
mask
evidence
validator output
```

不能把所有 Node 输出展平后称为最终结果。

## 4.5 功能真实性不变量

一个控件只有在以下条件都成立时才可以显示为可用：

```text
后端端点存在
前端调用存在
先决条件满足
操作结果可验证
错误可恢复
刷新后状态可恢复
```

否则：

* 隐藏；
* 标记 Labs；
* 或 disabled 并给出明确原因。

不能用按钮存在代替功能完成。

---

# 五、已确认缺陷清单

以下缺陷来自当前源码快照。修复前先为每项建立失败测试或可重复的复现记录。

---

# 六、P0：本地服务安全边界

## P0-01：跨源访问可触发高权限本地操作

位置：

```text
crates/annotagent-server/src/lib.rs:858
```

当前：

```rust
.layer(CorsLayer::permissive())
```

同时存在浏览器可调用的高权限接口：

```text
/api/plugins/packages/install
/api/plugins/:plugin_id/:version/test
/api/providers/:provider_id/credential
/api/providers/:provider_id/active-probe
/api/model-bundles/install
/api/projects/:project_id/runs
/api/projects/:project_id/batches
```

插件安装的人工确认由查询参数布尔值表达：

```text
crates/annotagent-server/src/lib.rs:8356-8400
```

插件签名只被标记为：

```text
PresentUnverified
Unsigned
```

位置：

```text
crates/annotagent-plugin-host/src/package.rs:254-258
crates/annotagent-plugin-registry/src/lib.rs:384-439
```

风险：

```text
恶意网页
→ 跨源访问 127.0.0.1
→ 上传任意 .annotplugin
→ 声明已审核权限和许可证
→ 安装并测试原生二进制
```

同一边界也暴露：

* Project 数据；
  -历史记录；
  -凭证覆盖；
  -付费模型探测；
* Run 启动；
  -模型安装和删除。

修复要求：

1. 默认移除 permissive CORS；
2. 只允许当前 GUI Origin；
3. 校验 `Origin` 和 `Host`；
4. 增加本地 Server Session；
5. 状态修改请求要求 CSRF Token 或等价同源证明；
6. Cookie 使用 `SameSite=Strict`；
7. Plugin、Model Bundle、Credential、Billable Probe 和删除操作要求一次性 privileged confirmation token；
8. unsigned 或 unverified Plugin 只能在显式 CLI Developer Mode 安装；
9. 浏览器 API 默认不得安装未验证签名的原生插件；
   10.远程模式必须使用独立认证；
10. 不在 `/api/health` 返回绝对 workspace 和 database 路径。

必须测试：

```text
恶意 Origin preflight
恶意 Origin simple request
同源请求
CSRF token 缺失
CSRF token 错误
Plugin install
Plugin test
Credential update
Billable probe
Run start
SSE
```

## P0-02：缺少统一请求、连接和事件流资源限制

当前没有看到全局：

```text
DefaultBodyLimit
ConcurrencyLimit
RateLimit
SSE client limit
```

Plugin 上传有局部限制，但普通 JSON、昂贵 Summary、历史查询和 SSE 连接没有统一边界。

修复要求：

* 全局 JSON body limit；
* 文件上传端点单独 override；
* 每 IP / Session 并发限制；
* 状态修改请求速率限制；
* Agent 和模型调用并发限制；
* SSE 客户端数量限制；
* 慢客户端断开；
* 所有列表分页；
* Request ID；
  -结构化 413、429 和超时错误。

---

# 七、P1：身份、数据完整性和后端真值

## P1-01：Run 的稳定 Project ID 在 API 层丢失

Storage 已有：

```text
crates/annotagent-storage/src/lib.rs:92-107
HistoryRun.project_id
```

但 Server DTO 丢失：

```text
crates/annotagent-server/src/lib.rs:2802-2830
```

Web DTO 也丢失：

```text
web/src/types.ts:84-111
```

前端改用名称关联：

```text
web/src/workspaceContext.ts:3-9
web/src/workspaceContext.ts:24-33
web/src/App.tsx:1595
web/src/App.tsx:6764-6770
```

复现：

1. 创建两个同名 Project；
2. 或创建 Run 后重命名 Project；
3. 打开 Run、Review 和 Project Usage；
4. 观察归属错误或对象无法找到。

修复：

* `RunSummary.project_id` 必填；
* Web `HistoryRun.project_id` 必填；
* 所有查询、筛选、跳转按 ID；
* legacy `project_id = null` 使用一次性迁移或显式 legacy resolver；
* 不允许顶层 UI 继续按名称 fallback。

## P1-02：所有 Project 被覆盖成同一组全局模型绑定

位置：

```text
crates/annotagent-server/src/lib.rs:2790-2799
```

当前 `product_projects` 将相同 Registry bindings 写入每个 Project。

位置：

```text
crates/annotagent-server/src/lib.rs:4418-4442
```

`get_project_summary` 又用一个 `workspace_model_binding` 覆盖 Project binding，并重写 Workflow 中所有有模型的节点。

Run Summary 还虚构单一：

```text
default-vision
```

位置：

```text
crates/annotagent-server/src/lib.rs:3001-3031
```

影响：

* UI 显示的模型未必是 Project 真实绑定；
* 多模型 Workflow 被压成一个模型；
* Plugin、Model revision 和 frozen binding 丢失；
* Run 可复现性展示失真。

修复：

分离：

```text
available_model_profiles
project_model_bindings
workflow_node_bindings
frozen_run_model_bindings
```

禁止在 Response 生成时修改 Workflow DTO。

## P1-03：Run Detail 可以将一个 Run 的 Artifact 叠加到另一个 Project Image

位置：

```text
web/src/App.tsx:6959-6971
web/src/App.tsx:7554-7556
```

当前 URL 使用数字 image index，预览判断没有确认所选 Image 与 Run input 一致。

修复：

* 使用稳定 `run_image_id` 或 `image_id`；
* 后端返回 Run 的合法 Image 集；
* URL query 解码后校验所有权；
* 不匹配时 404、canonical redirect 或清除选择；
* Canvas 只能加载 Run 允许的图片。

## P1-04：Results 将所有中间 Artifact 当成最终结果

位置：

```text
web/src/App.tsx:6966-6967
web/src/App.tsx:7531-7551
```

当前展平所有 Node outputs，再按几何近似去重。

影响：

```text
VLM 粗框
SAM 精修框
fallback 框
最终 Commit 框
```

可能同时进入 Results，或被不透明地合并。

修复：

后端提供显式：

```rust
RunResultProjection {
    committed_annotation_ids,
    review_candidate_ids,
    no_target_images,
    failed_images,
}
```

Results 只消费投影。

Debug 消费完整 lineage。

不得靠几何相似度猜哪个是最终结果。

## P1-05：Project-scoped Review 可显示其他 Project 的 Review Item

位置：

```text
web/src/App.tsx:7684-7701
```

直接 `reviewItemId` 的结果优先于 Project scope，URL 可以保留错误 `project_id`。

修复：

* canonical Review Detail 必须带真实 owner Project；
* `/projects/A/review/itemB` 若 itemB 属于 B，返回 404 或重定向到 B；
* Server 路由执行 ownership check；
* 前端不自行拼接虚假 scope。

## P1-06：空 Run 上创建 Annotation 时没有始终验证 Image 所有权

位置：

```text
crates/annotagent-application/src/lib.rs:6080-6100
```

修复：

无论 Run 是否已有 Annotation，都验证：

```text
annotation.image_id
∈ run input images / batch child image
```

拒绝其他 Project 或其他 Run 的 Image。

## P1-07：Annotation Import 按 project_name 选择 Run

位置：

```text
crates/annotagent-application/src/lib.rs:6117-6122
```

修复：

* 只使用 Project ID；
* legacy import 需要用户明确选择目标 Project；
* 不通过名称猜测。

## P1-08：Publish 没有要求当前 Draft 对应的持久化 Sample Test

位置：

```text
crates/annotagent-application/src/lib.rs:14765-14806
crates/annotagent-server/src/lib.rs:3505-3525
```

当前可以直接调用 API 发布一个静态合法但未完成真实 Sample Test 的 Draft。

修复：

Publish 必须要求：

```text
sample_test.draft_id
sample_test.draft_revision
sample_test.workflow_hash
sample_test.model_snapshot_hash
sample_test.image_set_hash
status = passed / human-approved
```

提供显式、审计化 override，但不能静默绕过。

## P1-09：Sample Test 使用时间戳判断新鲜度，并覆盖历史

位置：

```text
migrations/0005_workflow_sample_tests.sql
crates/annotagent-storage/src/lib.rs:1382-1404
crates/annotagent-server/src/lib.rs:3488-3492
```

当前：

* `draft_id` 是单行主键；
* 新测试覆盖旧测试；
* 使用 `completed_at >= draft.updated_at` 判断新鲜度。

问题：

* 同毫秒更新；
  -时钟精度；
  -异步乱序完成；
  -旧测试晚返回覆盖新测试；
  -无法比较历史测试。

修复：

```text
immutable sample_test_id
draft_revision
workflow_content_hash
request_revision
image_set_hash
model_snapshot_hash
started_at
completed_at
```

历史测试不可覆盖。

## P1-10：Workflow Autosave 没有乐观并发控制

位置：

```text
crates/annotagent-server/src/lib.rs:3383-3403
crates/annotagent-application/src/lib.rs:13350-13368
crates/annotagent-storage/src/lib.rs:1904-1926
```

当前是 last-write-wins。

复现：

1. 两个标签页打开同一 Draft；
2. A 修改并保存；
3. B 基于旧状态保存；
4. A 的改动被静默覆盖。

修复：

* Draft revision；
* ETag / `If-Match` 或 body `expected_revision`；
* 服务器返回 409；
* UI 显示 reload、compare、merge；
* autosave 请求有顺序号和取消；
* 旧请求返回不得覆盖新状态。

## P1-11：选择 Source Box 时覆盖 Annotation 的通用 confidence

位置：

```text
web/src/App.tsx:7790-7800
```

不同来源可能使用：

```text
semantic confidence
relative detection score
ranking score
not provided
```

不能写进同一个未标注语义的 `confidence`。

修复：

* 保留 source evidence；
* geometry 选择只修改 geometry；
* score + semantics 保留在 provenance；
* 只有明确策略允许时才更新 Annotation quality summary。

## P1-12：Review Item 之间的本地编辑状态泄漏

位置：

```text
web/src/App.tsx:7772-7780
web/src/App.tsx:7807
```

切换 Item 时没有完整重置：

* reviewer note；
* rejection reason；
* skill reason；
* unsaved state。

修复：

```text
ReviewDraft keyed by review_item_id
```

或在 Item identity 变化时原子初始化全部状态。

增加离开前 unsaved guard。

## P1-13：Run Detail 为找 Review Item 下载整个全局 Review Queue

位置：

```text
web/src/App.tsx:6935-6939
```

影响：

* N+1；
  -跨项目数据加载；
  -只取到一个 Review；
  -大量数据时性能恶化。

修复：

```text
GET /api/runs/:runId/reviews
```

或在 RunResultSummary 返回 review IDs。

## P1-14：每张图片都显示 Run 聚合状态

位置：

```text
web/src/App.tsx:2097-2108
web/src/App.tsx:7050-7059
```

Project 图片网格和 Run 图片列表使用同一个 Run-level `visibleStatus`。

一个 Batch 中：

```text
一张失败
一张 no target
一张 accepted
一张 review
```

现在可能全部显示相同状态。

修复：

后端提供：

```rust
RunImageSummary {
    image_id,
    status,
    annotation_count,
    review_count,
    failure,
}
```

## P1-15：Image 使用可变排序下标作为 ID

位置：

```text
crates/annotagent-server/src/lib.rs:762-787
crates/annotagent-application/src/lib.rs:7732-7742
crates/annotagent-application/src/lib.rs:14960-14974
web/src/App.tsx:854-858
```

当前删除、内容读取和 deep link 使用 `{index}`。

风险：

* 添加图片后 index 改变；
  -删除图片后后续 index 移动；
  -旧 URL 指向另一张图片；
  -过期 UI 可能删错文件。

修复：

* 持久化稳定 `ImageId`；
* index 只用于展示排序；
* API 使用 `/images/:imageId`；
* 删除要求 expected revision/hash；
  -历史 Run 保存 image content hash 和 path snapshot。

## P1-16：顶层代码仍存在 project_name legacy fallback

搜索并清理：

```text
web/src/App.tsx
web/src/workspaceContext.ts
crates/annotagent-application/src/lib.rs
```

legacy fallback 只能存在于显式 migration/reconciliation 层，不能继续驱动正常产品行为。

---

# 八、P2：路由、状态恢复和功能真实性

## P2-01：Project 内的 Runs 和 Review 不是真正的子路由

当前路由定义：

```text
web/src/navigation.ts:10-43
web/src/navigation.ts:130-203
```

Project 页面跳转：

```text
web/src/App.tsx:517-520
web/src/App.tsx:553-555
web/src/App.tsx:1740-1745
web/src/App.tsx:2187-2193
```

当前行为：

```text
Project
→ /runs?project_id=...
→ 全局 Runs Shell

Project
→ /review?project_id=...
→ 全局 Review Shell
```

顶部 Project Context 仅在：

```text
project
build
export
```

显示：

```text
web/src/App.tsx:432-460
```

修复目标见第九节的 canonical route model。

## P2-02：Start Dataset Run 后跳到筛选列表，不是刚启动的执行详情

位置：

```text
web/src/App.tsx:1624-1644
web/src/App.tsx:1117-1125
```

修复：

`startBatch` 返回稳定 Batch ID。

直接进入：

```text
/projects/:projectId/batches/:batchId
```

不要让用户在历史列表里寻找刚启动的任务。

## P2-03：没有可深链接的 Batch Detail

位置：

```text
web/src/App.tsx:6843-6877
```

Batch 只是列表里的 `<details>`。

问题：

* 刷新后折叠；
  -不可分享；
  -没有 aggregate progress 页面；
  -控制动作和子 Run 分散；
  -无法在 Project 内保持上下文。

修复：

实现 Project-scoped Batch Detail。

## P2-04：Run、Review 和 Back 链接丢失 Project scope

位置：

```text
web/src/App.tsx:6889
web/src/App.tsx:7024
web/src/App.tsx:7033-7054
web/src/App.tsx:8266
```

修复：

* canonical detail URL 带 Project；
* Back 使用真实父级；
* Review ↔ Run 保留 origin、image、node、artifact；
* 浏览器 Back/Forward 自然恢复，不依赖自定义猜测。

## P2-05：Run Detail canonicalization 丢弃 project_id

位置：

```text
web/src/navigation.ts:163-188
```

当前只有 Run list 才保留 `project_id`，Run Detail 不保留。

修复为真实 nested path，不继续堆 query。

## P2-06：Legacy `/models` 和 `/skills` 都错误跳向 Vision Workers

位置：

```text
web/src/navigation.ts:88-105
web/src/navigation.test.ts:13-22
```

正确目标：

```text
/models → /settings/models
/skills → /settings/capabilities 或 /settings/plugins
```

根据当前真实 IA 决定，不得都指向 `vision-workers`。

## P2-07：未知 URL 被静默重写到 Home

位置：

```text
web/src/navigation.ts:216
```

影响：

* 坏 deep link 被隐藏；
  -用户以为数据消失；
  -测试无法发现路由错误。

修复：

实现 Not Found 页面，提供：

```text
Go Home
Open Projects
Copy invalid URL
```

## P2-08：Canonicalizer 丢弃未纳入 Route 类型的 Query 状态

位置：

```text
web/src/App.tsx:264-268
```

当前 parser 重建窄化 URL，未知但合法的持久状态会被删除。

需要纳入 typed route：

```text
draft
workflow version
agent session
improvement session
return_to
filter
sort
image
node
artifact
view
```

禁止各组件手写 URL 字符串。

## P2-09：选择 Image、Node 或 Artifact 时 H1 反复抢焦点

位置：

```text
web/src/App.tsx:322-324
```

因为 `canonicalPath` 包含 query，切节点也触发页面标题 focus。

修复：

只在 route page identity 改变时 focus heading。

页内选择保持用户焦点。

## P2-10：URL 和 localStorage 同时作为 Active Project 真值

位置：

```text
web/src/App.tsx:235-237
web/src/App.tsx:326-358
```

修复：

* Project-scoped route 的 Project ID 来自 URL；
* global pages 默认 All Projects；
* localStorage 只记录 `lastOpenedProjectId`；
* 不用它决定当前对象所有权。

## P2-11：Run 图片状态筛选器是死控件

位置：

```text
web/src/App.tsx:7050
web/src/App.tsx:7059
```

使用 `defaultValue`，没有 state、事件或过滤逻辑。

修复：

* 有真实 per-image status 后实现；
* 若当前 Run 只有一张图片，删除该控件。

## P2-12：选择 Image、Node 或 Artifact 会强制切换 Debug

位置：

```text
web/src/App.tsx:6972-6978
```

修复：

* Results 中选择图片仍留在 Results；
* 只有点击 Node/Artifact Inspector 才切 Debug；
* `view` 是显式用户选择。

## P2-13：Image Query 没有完整类型校验

位置：

```text
web/src/App.tsx:6959-6960
```

需要处理：

```text
NaN
负数
越界
已删除 image
不属于 Run
```

迁移稳定 ImageId 后解决。

## P2-14：只有 checkpoint_present 才加载 Debug Artifacts

位置：

```text
web/src/App.tsx:6929-6930
```

历史事件、Artifacts 或错误可能存在但没有 checkpoint。

修复：

始终请求 authoritative debug summary。

后端明确返回：

```text
available
not recorded
legacy history
```

## P2-15：Pipeline 页 URL 不保存选中的 Draft 或 Published Version

位置：

```text
web/src/App.tsx:2594-2605
web/src/navigation.ts:130-142
```

当前只有 Test 路由保存 `draft`。

刷新 Pipeline 时可能选到：

```text
当前内存 Draft
否则第一个 Draft
```

修复：

```text
/projects/:pid/build/pipeline?draft=:draftId
/projects/:pid/build/pipeline?version=:workflowId@:version
```

Advisor Session 和 Improvement Session 也应持久化到 URL。

## P2-16：多个异步加载存在过期响应覆盖新页面

典型位置：

```text
web/src/App.tsx:664-679
web/src/App.tsx:1531-1579
web/src/App.tsx:2594-2797
web/src/App.tsx:6923-6940
web/src/App.tsx:7702-7771
```

没有统一 AbortController 或 request generation。

修复：

采用单一 route-aware query/cache 层。

可以使用成熟查询库，或实现明确的：

```text
resource key
AbortController
request generation
staleTime
invalidation
error scope
```

要求旧请求不得写入新 route。

## P2-17：SSE 全局刷新和高频 Polling 重复同步

位置：

```text
web/src/App.tsx:285-320
web/src/App.tsx:2767-2797
```

当前：

* SSE 事件可能触发整个 Dashboard refresh；
* Advisor 每 750ms poll；
  -页面自己的 effects 又重新请求。

修复：

* SSE 事件驱动精确 cache invalidation；
* polling 只作为断线 fallback；
* 不因一个 Run Event 重载所有 Projects、Runs 和 Models；
  -对 Agent Session 使用专属事件或指数退避。

## P2-18：Advanced Graph Editor 当前会生成不安全图

位置：

```text
web/src/App.tsx:3071-3112
```

问题：

* Node ID 使用数组长度生成，删除后可能碰撞；
  -删除 Node 不级联或阻止相关 Edge；
  -添加 Edge 默认连接前两个节点；
  -不选择端口；
  -不检查类型；
  -缺少 undo。

修复选项：

A. 在完整修复前将编辑器设为 read-only / Labs；

或 B. 实现：

* UUID；
  -端口选择；
  -类型兼容；
  -循环检查；
  -级联边处理；
  -Undo；
  -Graph validator。

不得继续把它作为正常可用功能展示。

## P2-19：Dry Run 的 “Review uncertain result” 只是滚动页面

位置：

```text
web/src/App.tsx:1178
```

这不是 Review。

修复：

* 改名为 `Inspect uncertain samples`；
* 或实现 sandbox review detail；
* 不将滚动行为描述为人工复核。

## P2-20：Sample 数量没有按数据集大小约束

UI 固定允许到 10，但 Project 可能少于 10 张或为 0。

修复：

```text
min = 1
max = available image count
0 images → disabled + Add images
```

## P2-21：Project Overview 和 Build 重复提供修改入口

位置：

```text
web/src/App.tsx:1972-2093
```

Overview 仍能：

* 改 Skill；
  -新增 Label；
  -导入 Annotation；
  -处理模型和数据。

这与 Guided Build 重复。

修复：

* Overview 只读、引导、状态和下一步；
  -所有配置修改进入 Build；
  -允许少量快捷 CTA，但跳往唯一编辑页面。

## P2-22：Run Progress 是按事件数量伪造

位置：

```text
web/src/App.tsx:1881-1885
```

当前：

```text
progress = runEvents.length * 3
```

修复：

后端提供：

```text
completed images / total images
completed tasks / total tasks
current stage
indeterminate flag
```

无法计算时使用 indeterminate，不伪造百分比。

## P2-23：Blocked Build Step 使用原生 disabled，原因不可聚焦

位置：

```text
web/src/App.tsx:782
web/src/App.tsx:803-809
```

修复：

* 使用可聚焦 `aria-disabled` 控件并阻止 activation；
  -或在旁边显示可访问 Blocker；
  -键盘用户必须能知道为何不可进入。

## P2-24：错误是全局状态，并用整页 reload 恢复

位置：

```text
web/src/App.tsx:462-472
```

影响：

* 旧页面请求错误可能出现在新页面；
* Reload 可能丢失未提交编辑；
  -无法精确重试失败操作。

修复：

* 错误按 Route / Resource / Mutation scope；
  -提供 Retry exact action；
  -全局仅保留真正的 Server connectivity error；
* Reload 前检查 unsaved state。

## P2-25：Review 未保存编辑没有导航和关闭保护

修复：

* Review Draft 显式 dirty；
  -切 Item、导航、刷新、关闭页面时提示；
  -或自动持久化 sandbox Review Draft；
  -不能静默丢失 bbox 修改。

## P2-26：Revision History 使用 `alert(JSON)`

位置：

```text
web/src/App.tsx:8303
```

修复：

* Drawer 或子路由；
* Loading；
  -Error；
  -版本对比；
  -Deep link；
  -不阻塞浏览器主线程。

## P2-27：通用 UI 硬编码具体模型和 Refiner 品牌

位置：

```text
web/src/App.tsx:7369-7373
web/src/App.tsx:8294
```

修复：

从 Model Profile、Plugin Registry 和 Refiner descriptor 读取：

```text
display_name
capability
version
status
```

通用 Canvas 和 Review 不认识 RF-DETR、LocateAnything、SAM 名称。

## P2-28：模型选择以泛化 Role 判断，可能拒绝合法 VLM Detection

位置：

```text
web/src/App.tsx:129-159
web/src/App.tsx:2624-2634
```

必须区分：

```text
native object detector:
ObjectDetection

VLM structured detector:
VisionLanguage
+ image input
+ structured output/tool calls
```

Qwen 可以合法绑定 `vlm_detection.detect`，但不能冒充 specialist detector。

模型兼容由 Node Contract 决定，而不是 UI 的泛化 role。

## P2-29：New Project 推荐与 Geometry Safety 冲突

位置：

```text
web/src/App.tsx:9352-9368
```

当前无可用模型时仍建议：

```text
Use open-vocabulary detector
Automatically accept high-confidence results
```

对 VLM bbox 还可能暗示语义高分可自动接受。

修复：

推荐必须基于：

```text
真实 Ready Model
Node quality contract
Geometry semantics
Calibration
Project geometry policy
```

只有 VLM 时默认：

```text
coarse detection → mandatory review
```

## P2-30：浏览器 UI 要求用户输入服务端本地文件路径

位置：

```text
web/src/App.tsx:870-879
web/src/App.tsx:2084-2092
```

在桌面本地模式可以作为高级功能，但不能冒充普通文件选择器。

修复：

* 浏览器 File Picker / 上传；
  -桌面受控目录选择；
  -或明确标记 `Advanced server-local path`；
  -部署模式不能暴露服务器文件系统输入。

## P2-31：现有测试固化错误 IA

位置：

```text
web/src/navigation.test.ts:96-113
web/e2e/guided-workspace.spec.ts:675-688
web/e2e/guided-workspace.spec.ts:1230-1255
web/e2e/guided-workspace.spec.ts:1307-1370
```

这些测试将：

```text
/runs?project_id=...
/review?project_id=...
```

作为 Project-scoped 正确行为。

修复测试时不能只调整字符串，需要重新验证完整 Project Shell、Breadcrumb、Sidebar 和 Back 行为。

## P2-32：列表接口存在 N+1 和无界加载

位置：

```text
crates/annotagent-server/src/lib.rs:2904-2919
crates/annotagent-server/src/lib.rs:3055-3062
crates/annotagent-server/src/lib.rs:3072-3078
crates/annotagent-server/src/lib.rs:5607-5654
```

`product_runs` 对每条 Run 调用完整 `history(run.id)`。

Project list 又加载所有 Runs。

Review progress 又加载多个 Run 的 annotations。

修复：

* Purpose-built SQL summary；
  -分页；
  -Project scope；
  -必要索引；
  -避免在列表 API 加载完整 Artifact/History；
  -性能测试记录 query count。

## P2-33：前后端超大单文件形成高回归风险

当前：

```text
web/src/App.tsx                         9,559 lines
crates/annotagent-server/src/lib.rs    12,222 lines
crates/annotagent-application/src/lib.rs 24,297 lines
```

不要先做大爆炸重写。

在 P0、P1 和路由正确性稳定后，按功能边界渐进拆分：

```text
web/src/routes/
web/src/features/projects/
web/src/features/runs/
web/src/features/review/
web/src/features/workflows/
web/src/features/settings/

server/routes/
application/projects/
application/runs/
application/review/
application/workflows/
```

每次拆分保持测试和行为不变。

---

# 九、目标路由和包含关系

建立两个明确 Shell。

## 9.1 Global Shell

```text
/
├── /projects
├── /runs
├── /review
└── /settings
```

职责：

```text
Home       全局待办和最近活动
Projects   项目库存
Runs       跨项目执行索引
Review     跨项目审核收件箱
Settings   全局 Provider、Model、Plugin 和 Storage
```

## 9.2 Project Shell

```text
/projects/:projectId
├── /overview
├── /build/data
├── /build/labels
├── /build/pipeline
├── /build/test
├── /runs
├── /runs/:runId
├── /batches/:batchId
├── /review
├── /review/:reviewId
└── /export
```

Project Shell 必须始终显示：

```text
Project name
Project breadcrumb
Project tabs
Current workflow
Active execution
Review count
```

Project 内点击 Runs、Review 或某个 Run，不能切换到全局 Sidebar 语义。

## 9.3 Canonical Object Route

Run 的 canonical route：

```text
/projects/:projectId/runs/:runId
```

Batch：

```text
/projects/:projectId/batches/:batchId
```

Review：

```text
/projects/:projectId/review/:reviewId
```

Global list 点击对象后，也进入其 Project canonical route。

## 9.4 Legacy Redirect

支持：

```text
/runs/:runId
/review/:reviewId
/runs?project_id=...
/review?project_id=...
```

服务器或前端先解析对象的真实 owner，再使用 `replaceState` 重定向到 canonical route。

不得信任 query 中声称的 Project ID。

## 9.5 URL 持久上下文

Run Detail：

```text
/projects/:pid/runs/:rid
  ?view=results
  &image=:imageId
```

Debug：

```text
/projects/:pid/runs/:rid
  ?view=debug
  &image=:imageId
  &node=:nodeId
  &artifact=:artifactId
```

Pipeline：

```text
/projects/:pid/build/pipeline?draft=:draftId
```

或：

```text
/projects/:pid/build/pipeline
  ?workflow=:workflowId
  &version=:version
```

Review origin 可以使用受控 `return_to`，但 canonical owner 仍来自对象。

所有 URL 构造必须经过统一 typed route builder。

禁止各组件直接拼接字符串。

---

# 十、Batch、Run 和 Image Run 的产品模型

明确区分：

```text
Dataset Execution / Batch
Image Run
Task Run
Node Execution
```

建议产品语言：

```text
Dataset Run
Image Result
Pipeline Step
```

## 10.1 Dataset Run Detail

显示：

* aggregate status；
* progress；
* image summaries；
  -总 usage；
  -总 cost；
  -Pause、Resume、Cancel；
  -failed/review/no-target filters；
  -child image links。

## 10.2 Image Run Detail

只显示该 Image：

* final result；
  -debug artifacts；
  -review；
  -replay；
  -error。

不要在单图 Run Detail 中提供一个假多图浏览器。

从 Dataset Run 切换 Image 时，进入相应 child Run canonical URL，或以 Batch-owned Image Result 路由表达。

## 10.3 Start 行为

```text
Start dataset run
→ 返回 batch_id
→ 进入 Batch Detail
```

```text
Start image run
→ 返回 run_id
→ 进入 Image Run Detail
```

不能进入历史列表再让用户找。

---

# 十一、API 数据契约重构

所有 Summary DTO 必须包含稳定所有权。

## ProjectSummary

```rust
project_id
readiness
active_batch_id
active_run_id
last_execution_id
default_workflow_version_id
review_count
```

## RunSummary

```rust
run_id
project_id
batch_id
image_id
workflow_version_id
frozen_model_bindings
status
progress
result_summary
```

## ReviewSummary

```rust
review_id
project_id
run_id
image_id
annotation_id
source_artifact_id
```

## ImageSummary

```rust
image_id
project_id
display_index
content_hash
path_snapshot
status
```

## WorkflowDraftSummary

```rust
draft_id
project_id
revision
content_hash
sample_test_status
```

禁止 Response-time 覆盖真实 persisted binding。

---

# 十二、前端状态和请求架构

## 12.1 Route 组件化

将 `App.tsx` 渐进拆分为：

```text
AppShell
GlobalRoutes
ProjectShell
ProjectOverviewRoute
ProjectBuildRoute
ProjectRunsRoute
ProjectRunDetailRoute
ProjectBatchDetailRoute
ProjectReviewRoute
ProjectReviewDetailRoute
ProjectExportRoute
GlobalRunsRoute
GlobalReviewRoute
SettingsRoute
NotFoundRoute
```

不要一次重写全部 UI。

## 12.2 Query Cache

建立统一资源缓存。

每个 Query 使用稳定 Key：

```text
project/:id
project/:id/summary
project/:id/runs
run/:id
run/:id/results
run/:id/debug
review/:id
workflow-draft/:id
sample-test/:id
```

支持：

* AbortController；
  -request generation；
  -deduplication；
  -SSE invalidation；
  -loading；
  -error；
  -stale state；
  -retry。

## 12.3 Mutation

每个 Mutation：

* 使用 expected revision；
  -返回 updated entity；
  -精确 invalidation；
  -不调用全局 `refresh()`；
  -失败不清空本地数据；
  -错误属于具体操作。

## 12.4 Unsaved State

对以下状态增加 dirty guard：

```text
Workflow Draft
Review geometry edit
Provider edit
Plugin install approval
Project Schema edit
```

---

# 十三、Workflow 生命周期修复

完整生命周期：

```text
Editing Draft
→ Static Validation
→ Persisted Sample Test
→ Human Review of test
→ Published Immutable Version
```

要求：

1. Pipeline URL 固定 Draft；
2. autosave 使用 revision；
3. Sample Test 是不可变记录；
4. Sample Test 固定 exact Draft hash；
5. Publish 检查 exact Sample Test；
   6.旧测试不得覆盖新测试；
6. Published Version 不可编辑；
7. Clone 创建新 Draft；
8. Test 页面刷新恢复 exact Draft 和 exact Test；
9. 同一 Draft 两标签页冲突有明确 UX。

---

# 十四、Review 完整性

Review 必须：

* canonical owner；
* run-scoped来源；
  -稳定 item ID；
* item-keyed local draft；
* unsaved guard；
  -正确 score semantics；
* source box 只修改 geometry；
* Revision drawer；
* Accept & Next；
* Reject & Next；
  -Project scope；
  -Run 双向跳转；
  -刷新恢复。

决策 API 不应依赖前端传入的 `project_id` 证明所有权。

Server 应从 Review ID 得到真实 Project，再校验可选 route Project。

---

# 十五、Feature Truth 审计

为所有可见操作建立：

```text
docs/execution/FEATURE_TRUTH_MATRIX.md
```

字段：

```text
Page
Control
Backend endpoint
Prerequisites
Persistence
Refresh behavior
Error behavior
E2E coverage
Status: ready / labs / hidden / remove
```

至少审计：

* Start image run；
  -Start dataset run；
  -Pause；
  -Resume；
  -Cancel；
  -Replay；
  -Image status filter；
  -Advanced graph；
  -Review uncertain sample；
  -Revision history；
  -Improve Automation；
  -Provider active probe；
  -Plugin install；
  -Model Bundle install；
  -Annotation import；
  -Export；
  -Open folder；
  -legacy worker setup。

任何无法完成真实闭环的控件：

* 修复；
  -改名；
  -标 Labs；
  -或删除。

不得继续保留“看起来已经能用”的空操作。

---

# 十六、性能和分页

建立 purpose-built summary query：

```text
list_projects_summary
list_executions_summary
list_project_runs_summary
list_batch_images_summary
list_review_summary
```

要求：

* 不在列表接口读取完整 History；
* 不加载完整 Artifact payload；
  -分页；
  -排序稳定；
  -Project ID 索引；
  -Run ID 索引；
  -Review project/run 索引；
  -Sample Test draft/hash 索引。

增加 query-count 或性能测试：

```text
100 Projects
1000 Runs
1000 Review Items
```

列表请求不能线性触发完整 History 查询。

---

# 十七、P3 产品简化建议

这些属于产品改进，不能代替 P0–P2 修复。

## 17.1 Project Overview 只承担“现在该做什么”

保留：

* readiness；
  -primary action；
  -active execution；
  -review count；
  -recent activity。

移除重复编辑器。

## 17.2 每页一个主要操作

Project、Run、Review、Export 各自只有一个主要 CTA。

## 17.3 Results 和 Debug 明确分离

Results 不出现内部 Artifact 名称。

Debug 保留全部技术信息。

## 17.4 Settings 收敛

区分：

```text
Providers
Models
Expert Model Plugins
Storage
Usage
```

旧 Vision Worker 作为 migration / legacy，不与 Plugin 和 Model 平行混杂。

## 17.5 Improve Automation 自动带入上下文

从 Run 或 Review 打开时，预选：

* current workflow version；
  -evidence run；
  -image；
  -source node；
  -correction reason。

不能只跳到 Pipeline 顶部。

## 17.6 状态词统一

统一：

```text
Dataset Run
Image Run
Review
No Target
Completed with Review
Interrupted
```

前后端共用枚举和展示映射。

---

# 十八、Milestone 计划

## Milestone 0：基线和失败测试

完成：

* 建立缺陷账本；
  -复现 Project → global Runs/Review；
  -复现名称关联；
  -复现 Draft 刷新；
  -复现 dead filter；
  -复现中间 Artifact 混入 Results；
  -记录 P0 安全行为；
  -记录性能基线。

提交：

```text
test(integrity): reproduce workspace ownership and navigation failures
```

## Milestone 1：本地服务安全边界

完成：

-严格 CORS；
-Origin/Host；
-local session；
-CSRF；
-privileged confirmation；
-body/concurrency/SSE limit；
-插件签名策略；
-security tests。

提交：

```text
fix(security): protect privileged localhost APIs from cross-origin access
```

## Milestone 2：稳定 ID 与 API 真值

完成：

* Run.project_id；
  -Review ownership；
  -ImageId；
  -移除名称关联；
  -真实 Project model bindings；
  -frozen Run bindings；
  -migrations；
  -API tests。

提交：

```text
fix(core): preserve stable ownership across projects runs and images
```

## Milestone 3：Project-scoped Route Model

完成：

* GlobalShell；
  -ProjectShell；
  -nested Runs；
  -nested Batch；
  -nested Review；
  -canonical detail routes；
  -legacy redirects；
  -NotFound；
  -typed route builders；
  -navigation tests。

提交：

```text
refactor(web): keep project-owned work inside the project workspace
```

## Milestone 4：Execution 和 Results 正确性

完成：

* Dataset Run Detail；
  -Image Run Detail；
  -start direct navigation；
  -per-image status；
  -result projection；
  -final vs intermediate artifacts；
  -review IDs；
  -image ownership；
  -E2E。

提交：

```text
fix(run): separate dataset execution final results and debug artifacts
```

## Milestone 5：可恢复前端状态

完成：

* route-aware query layer；
  -AbortController；
  -SSE invalidation；
  -remove duplicate polling；
  -URL draft/version/session；
  -focus behavior；
  -localStorage 降级为偏好；
  -refresh/back/forward tests。

提交：

```text
fix(web): restore project run workflow and review context from URLs
```

## Milestone 6：Workflow 生命周期和并发

完成：

* Draft revision；
  -optimistic concurrency；
  -immutable Sample Tests；
  -content hash；
  -publish requirement；
  -legacy migration；
  -two-tab tests；
  -out-of-order tests。

提交：

```text
fix(workflow): bind publication to the exact tested draft revision
```

## Milestone 7：Review 完整性

完成：

* scoped Review endpoints；
  -item-keyed drafts；
  -unsaved guard；
  -score semantics；
  -revision drawer；
  -bidirectional route；
  -foreign image/review rejection；
  -tests。

提交：

```text
fix(review): preserve ownership edits and provenance across review navigation
```

## Milestone 8：Feature Truth 和 Guided UX

完成：

* Feature Truth Matrix；
  -dead control removal；
  -status filter；
  -graph editor Labs/read-only 或完整修复；
  -Overview read-only；
  -real progress；
  -sample count；
  -file import UX；
  -model metadata；
  -recommendation safety；
  -accessibility。

提交：

```text
refactor(product): expose only complete and recoverable workspace actions
```

## Milestone 9：服务查询与渐进拆分

完成：

* summary SQL；
  -pagination；
  -indexes；
  -N+1 removal；
  -App route extraction；
  -Server route modules；
  -Application service modules；
  -no behavior regression。

提交：

```text
refactor(architecture): isolate workspace features and bounded summary queries
```

## Milestone 10：Release 验收

完成：

-全部 Rust 测试；
-Web unit；
-Playwright；
-security E2E；
-performance fixture；
-responsive；
-200% zoom；
-keyboard；
-docs；
-acceptance matrix。

提交：

```text
test(release): validate project-scoped workspace integrity alpha
```

---

# 十九、Release Blocking Acceptance Matrix

以下全部满足后才能声称完成。

## A. 安全

* [ ] permissive CORS 已移除。
* [ ] 跨源网页不能调用状态修改 API。
* [ ] Plugin 安装需要可信同源会话和 privileged confirmation。
* [ ] 未验证签名原生插件不能通过普通 Web UI 安装。
* [ ] Credential API 受 CSRF 防护。
* [ ] Billable Probe 受显式确认。
* [ ] 全局 body、并发和 SSE 限制存在。
* [ ] `/api/health` 不泄露绝对路径。

## B. 所有权

* [ ] Run API 包含 project_id。
* [ ] 不通过 project_name 关联正常对象。
* [ ] 重名 Project 不混淆。
* [ ] Project 改名后 Run 仍可打开。
* [ ] Review 固定 Project 和 Run。
* [ ] Image 使用稳定 ID。
* [ ] 外部 ImageId 不能写入 Run。
* [ ] 导入不按名称猜测 Run。

## C. 路由

* [ ] Project Runs 是真实 Project 子路由。
* [ ] Project Review 是真实 Project 子路由。
* [ ] Project Run Detail 保留 Project Shell。
* [ ] Project Batch Detail 可深链接。
* [ ] Global Runs 仍是跨项目索引。
* [ ] Global Review 仍是跨项目索引。
* [ ] Legacy URL 重定向到真实 owner。
* [ ] 未知 URL 显示 Not Found。
* [ ] Back/Forward 保持正确层级。

## D. 状态恢复

* [ ] Pipeline 刷新保持 exact Draft。
* [ ] Test 刷新保持 exact Draft 和 Sample Test。
* [ ] Run 刷新保持 Image、View、Node、Artifact。
* [ ] Review 刷新保持 Item。
* [ ] 切 Project 不被旧请求覆盖。
* [ ] SSE 重连重新同步服务器真值。
* [ ] localStorage 不决定对象所有权。
* [ ] H1 不在页内选择时抢焦点。

## E. Run 和结果

* [ ] Start Dataset Run 直接进入 Batch Detail。
* [ ] Start Image Run 直接进入 Run Detail。
* [ ] Batch 显示真实 aggregate progress。
* [ ] 每张图有独立状态。
* [ ] Results 只显示 final projection。
* [ ] Debug 显示全部 intermediate artifacts。
* [ ] 一张 Run 的 Artifact 不能叠加到另一张图。
* [ ] No Target 是合法结果。
* [ ] Review links 是 run-scoped。
* [ ] Model bindings 显示真实 frozen snapshot。

## F. Workflow

* [ ] Draft 有 revision。
* [ ] Autosave 使用 optimistic concurrency。
* [ ] 两标签页不会静默覆盖。
* [ ] Sample Test 不覆盖历史。
* [ ] Sample Test 绑定 Draft content hash。
* [ ] Publish 必须有 exact current Sample Test。
* [ ] Published Version 不可修改。
* [ ] Pipeline URL 保存 Draft/Version。
* [ ] 旧异步结果不能覆盖新 Draft。

## G. Review

* [ ] Project A 路由不能显示 Project B Review。
* [ ] Item 切换不泄漏 note 或 reason。
* [ ] 未保存 bbox 修改有保护。
* [ ] Source Box 选择不破坏 score semantics。
* [ ] Revision History 使用正式 UI。
* [ ] Review ↔ Run 双向导航保持上下文。
* [ ] Run 不下载整个全局 Review Queue。
* [ ] Accept & Next 使用正确 Project queue。

## H. Feature Truth

* [ ] Image status filter 真实工作或已删除。
* [ ] “Review uncertain result” 不再只是滚动。
* [ ] Advanced Graph 不再以损坏状态开放。
* [ ] Fake event-count progress 已移除。
* [ ] Sample count 与数据集大小一致。
* [ ] Overview 不重复提供全部 Build 编辑功能。
* [ ] 所有 disabled 操作有可访问原因。
* [ ] 所有 Labs 功能明确标识。
* [ ] UI 不硬编码专家模型品牌。
* [ ] 模型选择使用 Node capability contract。
* [ ] 推荐方案遵守 Geometry Safety。
* [ ] 浏览器文件导入不伪装成普通服务端路径输入。

## I. 性能与架构

* [ ] Run list 不为每条 Run 加载完整 History。
* [ ] Project list 不加载所有完整 Run history。
* [ ] Review progress 使用 summary query。
* [ ] 列表分页。
* [ ] 1000 Run fixture 查询数量受控。
* [ ] App.tsx 已按 route feature 渐进拆分。
* [ ] Server 和 Application 至少完成关键 route/service 拆分。
* [ ] 没有大爆炸重写导致功能回归。

---

# 二十、必须完成的回归场景

## Case 1：Project 内完整旅程

```text
Project A
→ Runs
→ Batch Detail
→ Image Run
→ Review
→ Return to Run
→ Export
```

整个过程始终保持 Project A Shell。

## Case 2：全局发现

```text
Global Runs
→ 打开 Project B Run
→ 自动进入 /projects/B/runs/:runId
```

## Case 3：Project 重名

```text
Project A name = Demo
Project B name = Demo
→ Run 和 Review 仍正确归属
```

## Case 4：Project 改名

```text
创建 Run
→ 改 Project 名称
→ Run、Review、Usage 和 Artifact 仍可找到
```

## Case 5：刷新恢复

分别刷新：

```text
Pipeline Draft
Test Result
Batch Detail
Run Results
Run Debug + Node + Artifact
Review Detail
Export
```

全部恢复相同对象。

## Case 6：慢请求竞态

```text
打开 Project A
→ 请求延迟
→ 快速切 Project B
→ A 响应晚到
```

页面必须仍显示 B。

## Case 7：两标签页编辑 Draft

第二个旧 revision 保存返回 409，不覆盖第一个。

## Case 8：中间 Artifact 与最终结果

```text
VLM coarse bbox
→ SAM refined bbox
→ final committed bbox
```

Results 只显示 final。

Debug 显示三者和 lineage。

## Case 9：混合 Batch

```text
Image 1 accepted
Image 2 no target
Image 3 review
Image 4 failed
```

每张图显示正确状态。

## Case 10：跨项目攻击

尝试：

```text
Project A route + Project B Run
Project A route + Project B Review
Run A + Image B
Run A + Artifact B
```

全部拒绝或 canonical redirect。

## Case 11：恶意网页 Origin

尝试跨源：

```text
install plugin
modify credential
run billable probe
start batch
delete model
```

全部失败。

## Case 12：Feature Truth

自动遍历主要按钮，确认：

* 操作可完成；
  -或明确 disabled；
  -没有 click 后无行为；
  -没有只滚动却声称 Review；
  -没有 alert JSON。

---

# 二十一、最终测试命令

Rust：

```bash
cargo fmt --all --check

cargo clippy \
  --workspace \
  --all-targets \
  --all-features \
  -- \
  -D warnings

cargo test --workspace --all-features
cargo build --workspace --all-features
```

Web：

```bash
npm --prefix web ci
npm --prefix web run typecheck
npm --prefix web test
npm --prefix web run build
npm --prefix web run test:e2e
```

重点新增测试：

```text
navigation ownership
legacy redirect
duplicate project names
project rename
foreign object access
stable image IDs
final result projection
sample-test content hash
draft optimistic concurrency
review state isolation
same-origin security
plugin install CSRF
N+1 query count
slow request race
refresh and Back/Forward
feature truth
```

浏览器尺寸：

```text
1920×1080
1440×900
1280×720
1024×768
720×450
390×844
```

验证：

* 无横向溢出；
  -200% zoom；
  -键盘；
  -focus；
  -Back/Forward；
  -refresh；
  -deep link。

---

# 二十二、不得采用的假修复

禁止：

* 只给 `/runs?project_id=` 加更醒目的 Breadcrumb；
* 继续使用 query 参数假装 Project 包含关系；
* 用 localStorage 恢复 Run 所有权；
* 用 Project 名称关联；
* 给 Run DTO 加 project_id，但前端仍按名称筛选；
* 添加一个 `isProjectMode` 布尔值；
* 用 Geometry 去重猜最终结果；
* 用事件数伪造进度；
* 把所有错误改成 `window.location.reload()`；
* 增加更多 polling；
* 用前端假状态修复后端 DTO；
* 把不可用功能藏在一个仍可点击的菜单；
* 修改测试字符串但不改页面行为；
* 大爆炸重写整个 App；
* 顺便增加新模型或插件；
* 删除历史 Published Workflow；
* 修改 Git remote；
* push；
  -提交 API Key；
  -用 Mock 冒充 live 验证。

---

# 二十三、最终报告格式

最终报告必须包含：

## 1. 审计复现

逐项列出修复前实际复现的 P0、P1 和 P2 缺陷。

## 2. 新路由模型

说明：

* GlobalShell；
  -ProjectShell；
  -canonical object routes；
  -legacy redirects；
  -Not Found。

## 3. 稳定身份

说明：

* Project ID；
  -Run ID；
  -Batch ID；
  -Image ID；
  -Review ID；
  -Artifact ownership；
  -名称 fallback 如何移除。

## 4. 数据真值

说明：

* Project model bindings；
  -frozen Run bindings；
  -final result projection；
  -per-image status；
  -sample-test binding。

## 5. 状态恢复

说明：

* URL；
  -query cache；
  -AbortController；
  -SSE；
  -refresh；
  -Back/Forward；
  -unsaved guards。

## 6. 安全

说明：

* CORS；
  -CSRF；
  -local session；
  -plugin install；
  -credential；
  -billable probe；
  -resource limits。

## 7. Feature Truth

列出：

```text
修复的控件
删除的控件
降级为 Labs 的控件
仍未完成的功能
```

## 8. 性能和模块化

说明：

* N+1；
  -pagination；
  -query count；
  -App/Server/Application 拆分。

## 9. 自动测试

列出实际执行命令和真实结果。

不得将未执行测试写成通过。

## 10. 手工浏览器验收

列出：

* Project journey；
  -global discovery；
  -refresh；
  -Back/Forward；
  -cross-project；
  -security；
  -responsive；
  -keyboard。

## 11. Milestone 提交

按顺序列出：

```text
commit hash
commit message
milestone
```

## 12. 未完成内容

明确区分：

```text
未实现
已实现但未人工验证
外部环境限制
不属于本轮
```

禁止使用：

```text
基本完成
理论上可用
应该没问题
大概率恢复
```

## 13. Git 状态

说明：

* 当前分支；
  -工作区是否干净；
  -领先远程提交数；
  -未 push；
  -remote 未修改。

---

# 二十四、启动指令

将本文保存为：

```text
docs/execution/WORKSPACE_INTEGRITY_MASTER_PROMPT.md
```

然后从 AnnotAgent 仓库根目录启动 Codex，输入：

```text
阅读 docs/execution/WORKSPACE_INTEGRITY_MASTER_PROMPT.md，并将其作为本次长期任务的最高目标。

先核验 Git、当前路由、API DTO、数据库实体、Project/Run/Review 所有权、Workflow Draft、Sample Test、插件安装接口、Web 测试和浏览器行为，不要盲信已有完成说明。

本次任务不是视觉优化，也不是增加模型。优先级必须是：

1. 修复本地服务跨源高权限调用风险；
2. 恢复 Project、Run、Batch、Image、Review 和 Artifact 的稳定 ID 所有权；
3. 建立真实 ProjectShell 和 canonical nested routes；
4. 将 Batch、Image Run、Results、Debug 和 Review 的职责拆清；
5. 让 URL 和服务器状态支持刷新、Back/Forward 和深链接；
6. 修复 Draft 并发、Sample Test 新鲜度和 Publish 边界；
7. 清理死控件、假进度、重复入口和不可达功能；
8. 删除列表接口的 N+1；
9. 最后再渐进拆分 App、Server 和 Application 单文件。

必须先为每个严重缺陷建立失败测试或可重复复现，再修复。

每完成一个 Milestone：
1. 更新缺陷账本、状态和验收证据；
2. 执行对应 Rust、Web、安全和浏览器测试；
3. 修复回归；
4. 创建独立本地提交；
5. 继续下一 Milestone。

不要通过 query 参数继续伪装 Project scope。
不要通过 project_name 关联实体。
不要通过 localStorage 恢复业务所有权。
不要让 Results 展示所有中间 Artifact。
不要保留工作但无真实行为的控件。
不要用前端假状态覆盖后端数据问题。
不要新增模型、Skill 或 Plugin。
不要修改历史 Published Workflow。
不要 push。
不要修改 Git remote。
不要使用或恢复任何对话中出现过的 API Key。
不要执行 reset、rebase、amend 或破坏性 checkout。

只有 Release Blocking Acceptance Matrix 全部满足，或只剩明确记录的外部环境项时，才输出最终报告。
```
