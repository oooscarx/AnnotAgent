# AnnotAgent Guided Experience Alpha

你现在是 AnnotAgent 仓库的长期主实现工程师。

本次任务不是视觉微调，也不是简单重命名页面，而是一次完整的产品体验重构：

> 将 AnnotAgent 从“功能齐全但需要用户自行理解内部架构的标注工具台”，改造成“能够主动引导用户完成数据导入、Label 定义、自动化配置、样本测试、正式运行、人工复核和导出的视觉标注产品”。

你必须直接检查当前仓库、修改真实代码、执行测试、修复缺陷、维护阶段状态文档，并持续推进到 Release Blocking 验收完成，或只剩明确记录的外部条件项。

不要只输出设计方案。

不要只修改 CSS。

不要只创建新的页面而保留旧的重复流程。

不要因为普通产品或技术细节停下来询问我。根据当前代码做合理决策，并记录决策。

---

# 一、任务名称和最终目标

本次长期任务名称：

```text
AnnotAgent Guided Experience Alpha
```

产品名称始终是：

```text
AnnotAgent
```

默认产品副标题：

```text
Composable annotation workflows for vision data.
```

本次目标用户是：

* 计算机视觉开发者；
* 机器人开发者；
* 小型算法团队；
* 希望结合 VLM、检测模型、分类器、确定性视觉工具和人工复核的数据开发者。

最终用户流程必须收敛为：

```text
创建 Project
→ 导入图片
→ 定义要标注什么
→ 接受或调整 AnnotAgent 推荐的自动化方案
→ 在少量样本上测试
→ 查看实际标注结果
→ 激活不可变 Pipeline Version
→ 正式运行全部数据
→ 处理不确定结果
→ 导出训练数据
```

用户默认不需要理解：

```text
ArtifactId
NodeId
Skill Registry
Model Registry
DAG Runtime
Artifact Payload
Workflow Hash
Provider Protocol
Internal Task ID
```

这些技术信息保留在 Expert Mode、Debug View、Inspector 和 Advanced 设置中。

---

# 二、产品北极星

第一次使用 AnnotAgent 的技术用户，应能在 10 分钟内完成：

1. 创建一个 Project；
2. 导入 5 张图片；
3. 创建一个 bounding-box Label；
4. 接受推荐的 Detection + Crop 自动化；
5. Dry Run 3 张图片；
6. 查看检测框和 Crop；
7. 激活自动化版本；
8. 正式运行 5 张图片；
9. 修改并接受一个待审核结果；
10. 导出 YOLO 或 COCO 数据。

整个默认流程中：

* 用户不需要手工输入内部 ID；
* 用户不需要复制 Run ID；
* 用户不需要从 Runs 页面跳到 Workflows 页面寻找 Artifact；
* 用户不需要先打开 Settings 才能完成模型绑定；
* 用户不需要查看完整 DAG；
* 用户始终知道当前阶段；
* 用户始终知道为什么不能继续；
* 用户始终有一个明确的主要操作。

核心体验必须从：

```text
这里有很多功能，请自行决定怎么使用。
```

变成：

```text
你现在处于这个阶段。
还有这些问题需要解决。
最合理的下一步是这个操作。
```

---

# 三、现有架构必须保留

AnnotAgent 当前已经采用以下正确架构，不能在本轮重构中破坏：

```text
AnnotAgent
├── Project
├── Label Schema
├── Workflow / Pipeline
├── Models
├── Capability Skills
├── Domain Skills
├── Runs
├── Artifacts
├── Review
└── Export
```

必须继续保持：

* AnnotAgent 是全局产品身份；
* RoboCup 只是可选 Domain Skill 和示例 Project；
* Classification、VLM Detection、YOLO Detection 是 Capability Skills；
* Crop、Filter、Gate、Review、Commit 是 Core Nodes；
* Label Schema 只定义“标什么”；
* Workflow 定义“怎么标”；
* Published Workflow Version 不可变；
* Run 固定具体 Workflow Version；
* Artifact 保存节点中间结果和 lineage；
* Advisor 只能生成 Draft，不能自动 Publish；
* Review 修改产生 revision；
* API Key 不写入数据库或日志；
* Core 不硬编码 RoboCup Label。

本轮主要改变的是：

* 信息架构；
* 引导逻辑；
* 默认交互顺序；
* 页面职责；
* URL 状态；
* 结果呈现；
* 技术信息的渐进展开。

本轮不是重新实现整个 Runtime。

---

# 四、开始前必须核验仓库

首先执行：

```bash
git status --short --branch
git log --oneline -15
```

然后检查：

* 当前分支；
* 工作区是否干净；
* 当前领先远程多少提交；
* 最近视觉重构和产品层级重构；
* Web 路由；
* Project DTO；
* Run DTO；
* Review DTO；
* Workflow DTO；
* Application Service；
* Axum API；
* TUI；
  -当前测试数量；
* Known Limitations；
* 当前执行状态文档。

必须阅读：

```text
README.md
docs/DESIGN.md
docs/PRODUCT_HIERARCHY.md
docs/VISUAL_SYSTEM_INTEGRATION.md
docs/KNOWN_LIMITATIONS.md
docs/execution/
web/src/App.tsx
web/src/styles.css
web/src/components/
crates/annotagent-application/
crates/annotagent-server/
crates/annotagent-runtime/
apps/annotagent/src/tui.rs
```

不要盲信已有文档中的“已完成”声明，必须通过代码和测试确认。

禁止：

```text
git reset
git rebase
git commit --amend
破坏性 git checkout
修改 Git remote
push
使用或恢复对话中出现过的 API Key
```

---

# 五、长期执行状态文件

创建并持续维护：

```text
docs/execution/GUIDED_EXPERIENCE_MASTER_PLAN.md
docs/execution/GUIDED_EXPERIENCE_STATUS.md
docs/execution/GUIDED_EXPERIENCE_DECISIONS.md
docs/execution/GUIDED_EXPERIENCE_ACCEPTANCE.md
docs/execution/GUIDED_EXPERIENCE_BLOCKERS.md
docs/execution/GUIDED_EXPERIENCE_KNOWN_LIMITATIONS.md
```

`GUIDED_EXPERIENCE_STATUS.md` 必须持续记录：

```text
当前 Milestone
已完成内容
正在处理内容
下一步
最近自动测试
最近浏览器测试
最近提交
Release Blocking 剩余项
真实 Blocker
```

每完成一个 Milestone：

1. 更新状态；
2. 更新验收证据；
3. 执行对应测试；
4. 修复回归；
5. 创建独立本地提交；
6. 继续下一个 Milestone；
7. 不等待我确认。

---

# 六、Guided Mode 与 Expert Mode

必须实现两层体验。

## 6.1 Guided Mode

Guided Mode 是默认模式。

它使用面向任务的语言：

```text
Data
Labels
Automation
Test
Run
Review
Export
```

默认隐藏：

```text
Artifact ID
Node ID
Workflow Hash
Model Capability Schema
Port Type
Provider Raw Response
Internal JSON
Raw Tool Calls
```

Guided Mode 重点回答：

```text
当前阶段是什么？
下一步是什么？
为什么？
执行后会得到什么？
```

## 6.2 Expert Mode

Expert Mode 按需打开。

它显示：

```text
Shared Stages
Label Pipelines
Advanced Graph
Node IDs
Artifact Inspector
Replay
Model Bindings
Validator
Fallback
Retry
Prompt Version
Workflow Version
Token Details
Provider Details
```

Guided Mode 和 Expert Mode 必须使用同一份后端数据和 Workflow Definition。

禁止：

* 复制两套 Workflow 配置；
* Guided Mode 修改一套配置；
* Expert Mode 修改另一套配置；
* 使用前端映射出一份无法被 Runtime 执行的假流程。

---

# 七、全局导航

全局主导航最多保留五个入口：

```text
Home
Projects
Runs
Review
Settings
```

迁移规则：

```text
Dashboard     → Home
Workflows     → Project Workspace / Build / Automation
Models        → Settings / Models
Skills        → Settings / Capabilities
Runs          → 保留全局入口
Review        → 保留全局入口
Settings      → 保留
```

禁止继续保留独立的全局 Workflows、Models、Skills 主导航入口。

## 7.1 Home

Home 只显示：

* New Project；
* Recent Projects；
* Active Runs；
* Needs Review；
* Recent Failures；
* Usage and Cost。

Home 不显示：

* 完整 Workflow Graph；
* 所有 Node；
* 所有 Skills；
* Provider 高级参数；
* Artifact Inspector；
* RoboCup 专用内容。

## 7.2 全局作用域

全局 `/runs` 和 `/review` 默认显示所有 Projects。

可以有明确的 Project Filter：

```text
All projects
```

顶部 Active Project 可以作为快捷入口，但不得暗中改变全局 Runs 和 Review 的查询范围。

Project 作用域必须由 URL 明确表达：

```text
/projects/:projectId/...
```

禁止依靠隐藏的全局 Active Project 状态决定用户正在修改哪个 Project。

---

# 八、Project Journey 状态机

新增后端确定性 Project Journey。

```rust
pub enum ProjectStage {
    NeedsData,
    NeedsLabels,
    NeedsAutomation,
    NeedsModelBinding,
    ReadyForSampleTest,
    SampleTestNeedsAttention,
    ReadyToActivate,
    ReadyToRun,
    Running,
    NeedsReview,
    ReadyToExport,
    ConfigurationIssue,
}
```

阶段必须由真实持久化状态计算，而不是前端自行猜测。

实现：

```rust
pub struct ProjectGuidance {
    pub project_id: ProjectId,
    pub stage: ProjectStage,
    pub completed_steps: u32,
    pub total_steps: u32,
    pub headline: String,
    pub explanation: String,
    pub primary_action: GuidedAction,
    pub secondary_actions: Vec<GuidedAction>,
    pub blockers: Vec<GuidanceBlocker>,
    pub updated_at: DateTime<Utc>,
}
```

```rust
pub struct GuidedAction {
    pub kind: GuidedActionKind,
    pub label: String,
    pub destination: Option<String>,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
}
```

```rust
pub struct GuidanceBlocker {
    pub code: String,
    pub title: String,
    pub explanation: String,
    pub repair_action: Option<GuidedAction>,
}
```

每个 Project 在任意时刻只能有一个主要操作。

规则至少包括：

| 真实状态         | Primary Action      |
| ------------ | ------------------- |
| 无图片          | Add images          |
| 无 Label      | Define labels       |
| 无 Automation | Choose automation   |
| 缺少模型绑定       | Connect model       |
| Workflow 非法  | Fix automation      |
| 未 Dry Run    | Test on samples     |
| Dry Run 有问题  | Review test results |
| 已测试未发布       | Activate automation |
| 已准备好         | Run dataset         |
| 有 Active Run | Open active run     |
| 有 Review     | Review results      |
| 全部完成         | Export dataset      |

Guidance Engine 必须位于 Rust Application 层。

前端不得通过多个接口自行推导 Primary Action。

---

# 九、推荐新增 API

实现或调整：

```text
GET  /api/projects/:projectId/guidance
GET  /api/projects/:projectId/readiness
GET  /api/projects/:projectId/summary
POST /api/projects/:projectId/guided-setup
POST /api/projects/:projectId/sample-tests
GET  /api/runs/:runId/result-summary
GET  /api/runs/:runId/debug-summary
GET  /api/reviews/:reviewId/next
POST /api/reviews/:reviewId/accept-and-next
POST /api/reviews/:reviewId/reject-and-next
GET  /api/projects/:projectId/export-readiness
```

如果已有等价 API，应扩展现有 API，而不是制造重复接口。

新增核心 DTO：

```text
ProjectGuidance
ProjectReadiness
ProjectWorkspaceSummary
RunResultSummary
RunDebugSummary
ReviewProgress
ReviewNextAction
ExportReadiness
AutomationSummary
SampleTestSummary
```

所有 DTO 必须来自真实 Storage、Runtime 和 Workflow 数据。

禁止前端硬编码 B-Human 或 RoboCup 示例数据。

---

# 十、Project Workspace

进入 Project 后，页面保持持续 Project Context。

Project Header 始终显示：

```text
Project name
Image count
Label count
Current automation
Active run
Needs review
```

Project 内只保留：

```text
Overview
Build
Runs
Review
Export
```

## 10.1 Overview

Overview 第一屏必须展示 Guidance Hero，而不是等权 Card 网格。

示例：

```text
B-Human Football Dataset

Ready to test

Your images and football label are configured.
Test the current automation on 3 images before running all 5 images.

[Test 3 images]
View automation
```

下面显示 Journey：

```text
Data          Complete · 5 images
Labels        Complete · 1 label
Automation    Draft ready
Sample test   Not run
Full run      Not started
Review        No items
Export        Not ready
```

Overview 下方才允许显示：

* Recent Activity；
* Usage；
* Advanced Project Details。

页面规则：

* 一个实心 Primary Button；
* 最多两个 Secondary Actions；
* 主要状态必须是 Journey Stage；
* Project 本身不使用 Running 或 Failed 作为主状态；
* Running 和 Failed 属于 Run。

---

# 十一、新建 Project 向导

`Projects → New Project` 必须改为短向导。

## Step 1：What do you want to annotate?

选择：

```text
Classify images
Find objects
Segment regions
Custom
```

选择 `Find objects` 后输入：

```text
Object name: Football
Output: Bounding boxes
```

内部自动生成稳定 ID。

用户默认不填写：

```text
task_id
label_id
schema field name
```

Advanced 中可以查看或修改。

## Step 2：Add data

支持：

* 选择图片目录；
* 显示发现的图片数；
* 显示损坏图片；
* 显示重复图片；
* 显示支持格式；
* 导入后自动进入下一步。

## Step 3：Choose a priority

选择：

```text
Faster
Balanced
Higher accuracy
```

可选高级约束：

```text
Maximum expected cost
Target human review rate
Offline only
Available local models
```

## Step 4：Recommended automation

AnnotAgent 根据：

* Annotation Kind；
* Labels；
* 已配置 Models；
* 已安装 Skills；
* 用户偏好；

给出推荐方案。

示例：

```text
Recommended

Detect football candidates with qwen3.7-flash
Crop each candidate
Automatically accept high-confidence results
Send uncertain results to Review

Estimated:
Medium latency
Low setup effort
Human review only for uncertain results
```

操作：

```text
Use recommendation
Customize
```

向导完成后直接进入 Project Overview。

---

# 十二、Build 流程

Build 是一个连续步骤：

```text
Data → Labels → Automation → Test & Activate
```

页面必须：

* 显示当前步骤；
* 显示已完成步骤；
* 支持前后导航；
* 自动保存；
* 刷新恢复；
* 阻止越过真实 Blocker；
* 不把四个步骤当成互不关联的管理页面。

## 12.1 Data

展示：

* 图片列表；
* 图片数量；
* 重复图片；
* 损坏图片；
* 路径；
* 导入状态；
* 添加更多图片；
* 删除导入引用。

## 12.2 Labels

默认语言：

```text
What do you want to annotate?
```

而不是：

```text
Label Schema Editor
```

任务卡展示：

```text
Object detection
Labels: football
Output: Bounding boxes
```

内部 ID、属性类型、原始 Schema 放在 Advanced。

## 12.3 Automation

Guided Mode 使用名称：

```text
How AnnotAgent will label your data
```

或者：

```text
Automation
```

默认展示自然语言步骤：

```text
1. Find football candidates in each image
2. Keep detections labeled as football
3. Crop each candidate with 25% padding
4. Automatically accept confidence ≥ 0.90
5. Send uncertain results to Review
```

操作：

```text
Edit automation
View technical graph
Ask AnnotAgent for a suggestion
```

## 12.4 Test & Activate

生命周期在内部仍然是：

```text
Draft
→ Static Validation
→ Dry Run
→ Publish
```

Guided Mode 使用：

```text
Unpublished changes
→ Check setup
→ Test samples
→ Activate automation
```

Published Workflow Version 保持不可变。

---

# 十三、Automation 编辑器

第一版默认采用：

```text
Shared Stages + Label Pipelines
```

不以自由 DAG 作为默认视图。

示例：

```text
Shared stages

Image
→ VLM Detection
Used by 2 labels · Runs once per image

Football pipeline

Football detections
→ Crop
→ Verify
→ Auto-accept rule
→ Save annotation
```

Shared Stage 必须明显展示：

* 每张图只执行一次；
* 被几个 Label Pipeline 使用；
* 输出 Artifact 类型；
* 绑定 Model。

节点卡默认只显示：

```text
Node name
Model
Input
Output
Core threshold
Status
```

高级配置进入 Drawer：

```text
Prompt
Timeout
Retry
Fallback
Class mapping
Padding
Raw node config
```

提供 Expert Mode：

```text
Advanced graph
```

Advanced graph 和 Guided Recipe 必须编辑同一份 Workflow Definition。

---

# 十四、Advisor 体验

Advisor 不以空白聊天框作为主要入口。

应显示上下文驱动建议卡。

示例：

```text
AnnotAgent recommendation

Football appears small in the selected samples.
Add crop verification after detection to reduce false positives.

[Preview change]
```

点击后显示 Proposed Changes：

```diff
 VLM Detection
 Filter football
+Crop candidates
+Verify each crop
 Auto-accept rule
 Save annotation
```

同时显示：

```text
Why
Expected model calls
Estimated cost level
Estimated latency
Unresolved model bindings
Warnings
```

操作：

```text
Apply to draft
Compare with current
Dismiss
```

Advisor 只能：

* 创建或修改 Draft；
* 调用 Static Validation；
* 调用 Dry Run；
* 查看结果；
* 提交给人工审批。

Advisor 不得：

* 自动 Publish；
* 自动运行完整数据集；
* 自动覆盖 Published Version；
* 执行任意代码或 Shell；
* 引用不存在的 Node 或 Model。

---

# 十五、Dry Run 结果优先

Dry Run 完成后的第一屏必须显示业务结果：

```text
Sample test complete

3 images tested
4 detections found
3 ready to accept
1 needs review
0 failed

Estimated full-run cost: ¥0.03
Estimated review workload: 1–2 results
```

主要操作根据结果变化：

```text
Review uncertain result
Activate automation
Fix automation
```

下方按顺序显示：

1. Results Gallery；
2. Uncertain Results；
3. Pipeline Diagnostics；
4. Model Usage；
5. Node Timings；
6. Technical Artifacts。

禁止第一屏只显示：

```text
input succeeded
detector succeeded
filter succeeded
crop succeeded
commit succeeded
```

节点成功不等于标注有效。

新增或完善：

```rust
pub struct SampleTestSummary {
    pub image_count: u32,
    pub detection_count: u32,
    pub candidate_count: u32,
    pub auto_accepted_count: u32,
    pub review_count: u32,
    pub failed_count: u32,
    pub empty_count: u32,
    pub duration_ms: u64,
    pub usage: UsageSummary,
    pub estimated_full_run: Option<FullRunEstimate>,
}
```

---

# 十六、Run Detail 分为 Results 和 Debug

Run Detail 顶部提供：

```text
Results
Debug
```

默认进入 Results。

## 16.1 Results View

显示：

```text
Run completed

5 images
1 football found
1 needs review
4 no-target results
0 failed
¥0.028
```

主要区域：

```text
Images
Result Preview
Needs Attention
```

主要操作：

```text
Review 1 result
Export
Open active run
```

合法空结果显示：

```text
No target found
```

不得默认显示：

```text
SucceededEmpty
```

## 16.2 Debug View

Debug 才显示：

```text
Pipeline Steps
Node Inspector
Artifact
Replay
Input
Output
Configuration
Provider request
Token
Cost
Raw errors
```

Run Detail 布局可以是：

```text
Images | Result Preview | Pipeline Steps
```

但 Results 模式下，Pipeline Steps 必须弱化或折叠。

## 16.3 Node Inspector

点击节点后显示：

```text
Status
Duration
Model
Input
Output
Warnings
Artifacts
Replay
```

不要求用户重新选择 Run ID。

Artifact Inspector 不再作为 Workflows 页面底部的独立工具。

---

# 十七、bbox 与 Crop 联动

必须实现：

1. bbox 显示 Label；
2. bbox 显示 confidence；
3. 显示颜色图例；
4. 点击 bbox 选中对应 Crop；
5. 点击 Crop 高亮父 bbox；
6. lineage 使用稳定视觉标识；
7. Crop 支持放大；
8. 显示 parent Artifact；
9. 显示 source node；
10. 多框选择稳定；
11. 键盘可以切换 bbox 和 Crop；
12. 可切换 Original、Result、Compare、Crop。

颜色映射顺序保持：

```text
Project override
→ Skill visual profile
→ Schema mapping
→ stable hash
```

不能只靠颜色区分类别。

bbox 标签示例：

```text
football · 0.98
```

---

# 十八、Review 改造成 Inbox

Review 默认目标是快速做决定，不是展示所有技术能力。

布局继续使用：

```text
Review Queue | Annotation Canvas | Details
```

但默认操作必须是：

```text
Accept & next
```

快捷键至少支持：

```text
A       Accept and next
R       Reject and next
E       Edit
Space   Toggle original/result
← →     Previous/next
```

顶部显示：

```text
3 of 12 results reviewed
```

Details 默认只显示：

```text
Why this needs review
Confidence
Source Run
Automation Version
Source Step
```

执行细节折叠在：

```text
Execution details
```

Reject 后再要求选择原因：

通用原因：

```text
Not the target
Wrong box
Duplicate
Wrong label
Other
```

启用 Domain Skill 时动态增加对应原因。

例如启用 `robocup.ball`：

```text
White shoe
White sock
Penalty mark
Field-line intersection
```

人工修正后明确显示：

```text
This correction will make similar candidates more likely to be reviewed.
```

Review Detail 与 Run Detail 必须双向跳转：

```text
Review → Open run context
Run → Open review item
```

返回后保持原选择。

---

# 十九、Export 作为旅程终点

Export 页面先显示 readiness：

```text
Your dataset is ready

5 images
1 accepted football annotation
0 unresolved reviews
```

根据 Label Schema 推荐格式。

例如：

```text
Recommended for object detection training

YOLO Detection
All annotations supported
```

其他格式展示兼容性：

```text
COCO
All annotations supported

LabelMe
Attributes included as metadata

AnnotAgent Native
Full lineage and revisions preserved
```

主要操作：

```text
Export YOLO dataset
```

导出完成后：

```text
Dataset exported successfully

Open folder
View export report
```

实现或完善：

```rust
pub struct ExportReadiness {
    pub ready: bool,
    pub accepted_annotations: u64,
    pub unresolved_reviews: u64,
    pub blocking_issues: Vec<ExportBlocker>,
    pub recommended_format: Option<String>,
    pub formats: Vec<ExportFormatCompatibility>,
}
```

---

# 二十、术语分层

默认产品语言与内部术语的映射：

| 内部术语                    | Guided Mode           |
| ----------------------- | --------------------- |
| Label Schema            | What to annotate      |
| Workflow                | Automation            |
| Workflow Draft          | Unpublished changes   |
| Static Validation       | Check setup           |
| Dry Run                 | Test samples          |
| Publish Version         | Activate automation   |
| Artifact                | Intermediate result   |
| Node Artifact Inspector | Execution details     |
| Replay from node        | Re-run from this step |
| Skill                   | Capability            |
| Model Binding           | Model choice          |
| Confidence Gate         | Auto-accept rule      |
| Commit                  | Save annotation       |
| SucceededEmpty          | No target found       |
| Partial                 | Completed with issues |

Expert Mode 可以显示内部术语。

不要为了“友好”删除真实技术信息，只需把它们放到合适层级。

---

# 二十一、视觉密度和组件规则

现有“深色导航 + 浅色工作区”视觉系统保留。

本轮重点不是重做 Logo 或配色。

必须遵守：

* 每页最多一个实心 Primary Button；
* 最多两个直接可见 Secondary Actions；
* 危险操作进入 More Menu；
* 不允许 Card 套 Card；
* 第一屏最多三个同权重指标；
* 技术 metadata 默认折叠；
* 页面标题下必须说明用户当前任务；
* 空状态必须包含具体下一步；
* 不同时展示完整 Pipeline 和完整 Inspector；
* Status pill 只用于真实状态；
* 主操作只使用主蓝色；
* 紫色只用于 Skill 或领域上下文；
* 红色只用于失败和危险操作；
* 图标必须配文字或 accessible name；
* 不使用难读的技术字体；
* 代码和原始 JSON 才使用等宽字体。

---

# 二十二、URL 与状态恢复

所有重要上下文写入 URL。

推荐路由：

```text
/
/projects
/projects/new
/projects/:projectId
/projects/:projectId/build/data
/projects/:projectId/build/labels
/projects/:projectId/build/automation
/projects/:projectId/build/test
/projects/:projectId/runs
/projects/:projectId/review
/projects/:projectId/export

/runs
/runs/:runId?view=results&image=:imageId
/runs/:runId?view=debug&image=:imageId&node=:nodeId&artifact=:artifactId

/review
/review/:reviewItemId

/settings
/settings/models
/settings/capabilities
/settings/storage
```

验收：

* 刷新不丢状态；
* 浏览器前进后退不丢状态；
* 复制链接可以打开同一 Run、Image、Node 和 Artifact；
* Project 页面从服务端恢复 Active Run；
* SSE 重连后重新读取服务端真值；
* 离开 Project 再返回，Journey Stage 正确；
* Run 页面不依赖局部 `useState` 保存唯一 Run 真相；
* Review 返回后保持当前队列位置；
* Guided Build 返回后保持步骤。

---

# 二十三、加载、错误和恢复

所有主要页面必须有：

```text
Loading
Empty
Error
Recoverable error
Permission/configuration issue
```

错误信息必须说明：

```text
发生了什么
为什么
是否影响数据
下一步如何修复
```

例如：

```text
The selected automation cannot be tested.

The VLM Detection step has no model selected.

[Choose a model]
```

禁止只显示：

```text
Request failed
run reached a terminal condition
invalid configuration
```

所有 Blocker 必须尽量带 Repair Action。

---

# 二十四、响应式和无障碍

桌面端：

* 完整 Sidebar；
* Run 可三栏；
* Review 可三栏。

平板端：

* Sidebar 缩为图标导航；
* Inspector 下沉；
* Pipeline Steps 可折叠。

手机端：

* 顶部导航；
* Project Context 独占一行；
* 单栏；
* Queue 横向滑动；
* Canvas、Actions、Details 顺序排列；
* 不产生横向溢出。

必须验证：

* 1920×1080；
* 1440×900；
* 1280×720；
* 1024×768；
* 720×450，作为约 200% Zoom 等价视口；
* 实际浏览器 200% Zoom，若环境允许。

无障碍要求：

* 完整键盘焦点；
* `focus-visible`；
* 图标按钮 accessible name；
* 表单有 label；
* 状态不只依赖颜色；
* Canvas 有等价的可操作标注列表；
* reduced motion；
* disabled 按钮说明原因；
* 错误与提示可被辅助技术识别。

---

# 二十五、Milestone 计划

## Milestone 0：基线和验收账本

完成：

* 仓库核验；
* 路由清单；
* 当前用户路径；
* 当前 API 和 DTO；
* 当前浏览器行为；
* 当前测试基线；
* 执行状态文档；
* Release Matrix 初稿。

提交：

```text
docs: establish guided experience baseline
```

## Milestone 1：Guidance Domain Model

完成：

* ProjectStage；
* ProjectGuidance；
* GuidedAction；
* GuidanceBlocker；
* ProjectReadiness；
* Rust Guidance Engine；
* API；
* 单元测试。

测试所有主要状态：

```text
NeedsData
NeedsLabels
NeedsAutomation
NeedsModelBinding
ReadyForSampleTest
ReadyToActivate
ReadyToRun
Running
NeedsReview
ReadyToExport
```

提交：

```text
feat(application): add deterministic project guidance
```

## Milestone 2：全局信息架构

完成：

* 全局导航收缩；
* Models 和 Skills 移入 Settings；
* Workflow 移入 Project；
* 旧路由 redirect；
* 全局 Runs 和 Review 明确作用域；
* 移除重复入口。

提交：

```text
refactor(web): simplify global navigation around user tasks
```

## Milestone 3：Guided Project Creation

完成：

* 创建向导；
* 任务类型选择；
* 自动生成内部 ID；
* 数据导入；
* 优先级；
* 推荐 Automation；
* Inline Model Connection；
* 创建后直接进入 Project。

提交：

```text
feat(web): add guided project creation
```

## Milestone 4：Project Journey Workspace

完成：

* Guidance Hero；
* Journey Timeline；
* 动态 Primary Action；
* Project Header；
* Active Run；
* Review Count；
* Readiness；
* Blocker Repair Action。

提交：

```text
feat(web): guide project work with a single next action
```

## Milestone 5：Guided Build

完成：

* Data；
* Labels；
* Automation；
* Test & Activate；
* 自动保存；
* Step URL；
* 刷新恢复；
* Advanced ID 设置；
* 结果导向语言。

提交：

```text
feat(web): turn project build into a guided sequence
```

## Milestone 6：Automation Recipe 和 Advisor

完成：

* 自然语言 Recipe；
* Shared Stages；
* Label Pipelines；
* Runs once per image；
* Node Drawer；
* Expert Graph；
* Proposed Changes；
* Compare；
* Apply to Draft；
* Advisor 不自动 Publish。

提交：

```text
feat(web): add guided automation recipes and advisor proposals
```

## Milestone 7：Outcome-first Dry Run

完成：

* SampleTestSummary；
* 结果数量；
* Review Workload；
* Full Run Estimate；
* Gallery；
* Uncertain Results；
* Diagnostics 下沉；
* Activate Automation。

提交：

```text
feat(run): present sample tests around annotation outcomes
```

## Milestone 8：Results-first Run Workspace

完成：

* Results / Debug 双模式；
* RunResultSummary；
* Image Browser；
* bbox label/confidence；
* Crop 联动；
* Debug Inspector；
* Deep Link；
* Replay；
* 错误修复操作。

提交：

```text
feat(web): unify run results and execution debugging
```

## Milestone 9：Inbox Review

完成：

* Accept & Next；
* Reject & Next；
* 队列进度；
* 键盘操作；
* Review 原因；
* Skill-specific reasons；
* Run 双向跳转；
* Correction impact explanation。

提交：

```text
feat(review): streamline human verification as an inbox
```

## Milestone 10：Guided Export

完成：

* ExportReadiness；
* 推荐格式；
* 兼容性；
* Blocking Reviews；
* 成功状态；
* Open Folder 或结果路径；
* Export Report。

提交：

```text
feat(export): complete the guided dataset journey
```

## Milestone 11：状态恢复和可靠性

完成：

* URL 状态；
* refresh；
* back/forward；
* SSE reconnect；
* Active Run 恢复；
  -重复 Start 锁定；
* Review Queue 恢复；
* Build Step 恢复；
* 服务端真值优先。

提交：

```text
fix(web): make guided context durable across navigation
```

## Milestone 12：无障碍、响应式和 Release

完成：

* 键盘路径；
* 1024px；
* 200%；
* reduced motion；
  -空状态；
  -加载状态；
  -错误恢复；
  -浏览器 E2E；
* README；
* Demo；
* Known Limitations；
* Release Matrix。

提交：

```text
test(release): validate guided experience alpha
```

---

# 二十六、Release Blocking Acceptance Matrix

以下全部满足后才能声称 Alpha 完成。

## A. 信息架构

* [ ] 全局主导航不超过 5 个。
* [ ] Workflows 不再是独立主要入口。
* [ ] Models 位于 Settings。
* [ ] Skills 或 Capabilities 位于 Settings。
* [ ] 全局 Runs 默认不被隐藏 Active Project 静默过滤。
* [ ] Project 内有持续上下文。
* [ ] 无重复 Workflow 或 Inspector 入口。

## B. Guidance

* [ ] Project Guidance 由 Rust 后端计算。
* [ ] 每个 Project 状态有且只有一个 Primary Action。
* [ ] 无数据时引导 Add images。
* [ ] 无 Label 时引导 Define labels。
* [ ] 无 Automation 时引导 Choose automation。
* [ ] 缺少模型时提供修复动作。
* [ ] 未测试时引导 Test samples。
* [ ] 有 Active Run 时引导 Open active run。
* [ ] 有 Review 时引导 Review results。
* [ ] 完成后引导 Export。

## C. 创建和 Build

* [ ] 用户无需手工输入内部 ID。
* [ ] New Project 向导可创建真实 Project。
* [ ] Build 是连续的四步流程。
* [ ] 刷新后恢复 Build Step。
* [ ] Labels 默认使用用户语言。
* [ ] Automation 默认使用 Recipe。
* [ ] Expert Graph 使用同一份 Workflow。
* [ ] Draft 自动保存。
* [ ] Published Version 不可编辑。

## D. Advisor

* [ ] Advisor 显示上下文建议。
* [ ] Advisor 显示 Proposed Changes。
* [ ] 可以 Compare。
* [ ] 只能 Apply to Draft。
* [ ] 不自动 Publish。
* [ ] 不自动运行完整 Dataset。
* [ ] 不引用未知 Node 或 Model。
* [ ] 显示预计成本和延迟等级。

## E. Dry Run

* [ ] 第一屏显示图片和标注结果数量。
* [ ] 第一屏显示 review 数量。
* [ ] 第一屏显示失败数量。
* [ ] 第一屏显示 duration 和 cost。
* [ ] 节点状态不占据第一视觉层级。
* [ ] 可以直接打开不确定结果。
* [ ] 成功后可以 Activate Automation。
* [ ] Dry Run 不写正式 Annotation。

## F. Run

* [ ] 默认进入 Results。
* [ ] Debug 需要主动切换。
* [ ] Results 显示结果数量。
* [ ] 无目标显示 No target found。
* [ ] bbox 显示 Label 和 Confidence。
* [ ] bbox 与 Crop 双向联动。
* [ ] Artifact Inspector 位于 Run Detail。
* [ ] 用户无需手工选择 Run ID。
* [ ] Replay 可从当前节点执行。
* [ ] 节点错误包含修复信息。

## G. Review

* [ ] 支持 Accept & Next。
* [ ] 支持 Reject & Next。
* [ ] 显示审核进度。
* [ ] 显示为什么进入 Review。
* [ ] 支持键盘操作。
* [ ] Skill-specific reason 只在对应 Skill 启用时出现。
* [ ] Review 可跳转来源 Run 和 Node。
* [ ] Run 可跳转对应 Review。
* [ ] 返回后保持选择。
* [ ] 完成最后一项后引导 Export。

## H. Export

* [ ] 显示 Export Readiness。
* [ ] 未解决 Review 会阻止或警告导出。
* [ ] 推荐与 Schema 匹配的格式。
* [ ] 显示格式兼容性。
* [ ] 显示导出报告。
* [ ] 导出完成后有明确结束状态。

## I. 状态恢复

* [ ] 刷新 Project 不丢 Stage。
* [ ] 刷新 Run 不丢 Image。
* [ ] 刷新 Debug 不丢 Node。
* [ ] URL 可打开同一 Artifact。
* [ ] 浏览器前进后退正确。
* [ ] Active Run 从服务器恢复。
* [ ] SSE 重连后重新同步。
* [ ] 活动 Run 时 Start 禁用。
* [ ] 后端仍阻止重复启动。

## J. 产品与视觉

* [ ] 默认模式不显示 ArtifactId。
* [ ] 默认模式不显示完整 DAG。
* [ ] 页面只有一个 Primary Button。
* [ ] 无 Card 套 Card。
* [ ] 首屏同权指标不超过三个。
* [ ] 技术 metadata 默认折叠。
* [ ] 空工作区不出现 RoboCup。
* [ ] Generic Project 不出现 RoboCup。
* [ ] AnnotAgent 仍是全局品牌。

## K. 响应式和无障碍

* [ ] 1024px 无横向溢出。
* [ ] 720×450 等价视口可操作。
* [ ] 实际 200% Zoom 人工验证，若环境允许。
* [ ] 主要流程可以使用键盘。
* [ ] Review 可以纯键盘完成。
* [ ] 焦点可见。
* [ ] 状态不只依赖颜色。
* [ ] Canvas 有等价标注列表。
* [ ] reduced motion 有效。

---

# 二十七、必须完成的端到端任务

使用 Mock 或当前 B-Human Demo 完成以下真实操作。

## Task A：首次项目旅程

```text
空工作区
→ New Project
→ Find objects
→ Football
→ Bounding boxes
→ 导入 5 张图
→ Balanced
→ 使用推荐 Automation
→ Dry Run 3 张
→ 查看 bbox
→ 查看 Crop
→ 激活 Automation
→ 正式运行
→ Review
→ 修改 bbox
→ Accept & next
→ Export YOLO
```

记录：

* 总步骤数；
* 主要点击数；
* 完成时间；
* 中断点；
* 截图；
* API 请求；
* 最终导出。

## Task B：刷新恢复

```text
打开 Run Detail
→ 选择 image
→ 切换 Debug
→ 选择 node
→ 刷新
→ 保持同一 Run、image 和 node
```

## Task C：Review 双向跳转

```text
Review Detail
→ Open run context
→ 回到 Review
→ 保持同一 review item
```

## Task D：Active Run

```text
启动 Run
→ 离开 Project
→ 返回
→ 显示 Open active run
→ Start 不可用
```

## Task E：Generic Project

```text
创建 Generic Classification Project
→ 不启用 RoboCup
→ 页面和 Trace 无 RoboCup 文案
→ 完成 Dry Run 和正式 Run
```

---

# 二十八、自动测试

Rust 必须执行：

```bash
cargo fmt --all --check
```

```bash
cargo clippy \
  --workspace \
  --all-targets \
  --all-features \
  -- \
  -D warnings
```

```bash
cargo test --workspace --all-features
```

```bash
cargo build --workspace --all-features
```

Web 根据现有包管理器执行：

```bash
npm run typecheck
npm run test
npm run build
```

新增浏览器或 E2E 测试：

1. 空工作区 Guidance；
2. Guided Project Creation；
3. Project Journey；
4. Build Step URL；
5. 推荐 Automation；
6. Dry Run Result Summary；
7. Activate Automation；
8. Results / Debug 切换；
9. Run URL Refresh；
10. bbox / Crop 联动；
11. Accept & Next；
12. Review → Run → Review；
13. Active Run Start Lock；
14. Export Readiness；
15. Generic Project 无 RoboCup；
16. 1024px 无溢出。

---

# 二十九、不得采用的假修复

禁止：

* 只改导航名称；
* 只调整 Card、阴影和圆角；
* 给现有页面加一个步骤条但内容不变；
* 新增 Guided Workspace，同时保留全部旧操作路径；
* 用前端假数据生成 Guidance；
* 把 Advisor 变成空白聊天框；
* 用 Tooltip 代替减少决策；
* 用产品导览动画代替真正引导；
* 隐藏错误但不给 Repair Action；
* 同时维护两份 Workflow 配置；
* 默认展示完整 DAG；
* 把 Run Inspector 复制到多个页面；
* 把内部状态名直接显示给普通用户；
* 让 Active Project 隐式过滤全局页面；
* 为了产品感删除真实 Debug 能力；
* 顺便实现无关模型 Backend；
* 破坏 Agent + Skill 边界；
* 将 RoboCup 恢复为全局产品身份；
* push；
* 修改 remote；
* 提交 API Key。

---

# 三十、文档

更新：

```text
README.md
docs/DESIGN.md
docs/PRODUCT_HIERARCHY.md
docs/VISUAL_SYSTEM_INTEGRATION.md
docs/KNOWN_LIMITATIONS.md
```

新增：

```text
docs/GUIDED_EXPERIENCE.md
docs/PROJECT_GUIDANCE.md
docs/GUIDED_PROJECT_SETUP.md
docs/RUN_AND_REVIEW_UX.md
docs/DEMO_GUIDED_EXPERIENCE.md
```

README 首屏仍然保持：

```markdown
# AnnotAgent

Composable annotation workflows for vision data.
```

但介绍顺序改为：

1. 告诉 AnnotAgent 要标什么；
2. AnnotAgent 推荐自动化；
3. 在样本上测试；
4. 运行数据；
5. 只处理不确定结果；
6. 导出。

复杂架构放到后面。

---

# 三十一、最终报告格式

最终回复必须包含：

## 1. Guided Experience 总结

说明旧流程和新流程的具体区别。

## 2. Guidance Engine

说明：

* 后端数据模型；
* ProjectStage；
* Primary Action；
* Blocker；
* API。

## 3. 新信息架构

说明：

* 全局导航；
* Project Workspace；
* Settings；
* Runs；
* Review。

## 4. Guided Project Creation

说明真实实现的向导步骤。

## 5. Automation 和 Advisor

说明：

* Guided Recipe；
* Expert Graph；
* Proposed Changes；
* Draft 边界。

## 6. Dry Run

说明结果优先的摘要和操作。

## 7. Run Workspace

说明 Results、Debug、Artifact、Replay 和 bbox/Crop 联动。

## 8. Review

说明 Inbox、Accept & Next、原因和双向跳转。

## 9. Export

说明 readiness、推荐格式和完成状态。

## 10. 状态恢复

说明 URL、刷新、前进后退、SSE 和 Active Run。

## 11. 自动测试

列出实际执行命令和真实结果。

## 12. 手工任务验收

列出 Task A 至 Task E 的真实结果、步骤数和失败点。

## 13. Milestone 提交

按顺序列出：

```text
commit hash
commit message
milestone
```

## 14. 未完成内容

必须明确区分：

* 未实现；
* 已实现但未人工验证；
* 外部环境限制；
* 不属于本轮范围。

禁止使用：

```text
基本完成
理论上支持
应该可用
大概率正常
```

## 15. Git 状态

说明：

* 当前分支；
* 工作区是否干净；
* 领先远程提交数；
* 未 push；
* remote 未修改。

---

# 三十二、启动指令

将本文保存为：

```text
docs/execution/GUIDED_EXPERIENCE_MASTER_PROMPT.md
```

然后执行：

```text
阅读 docs/execution/GUIDED_EXPERIENCE_MASTER_PROMPT.md，并将其作为本次长期任务的最高产品目标。

先核验 Git、当前代码、路由、API、测试和浏览器行为，不要盲信已有完成说明。

从 Milestone 0 开始持续执行。普通技术和产品决策自行决定，并记录到 GUIDED_EXPERIENCE_DECISIONS.md。

每完成一个 Milestone：
1. 更新状态和验收证据；
2. 执行对应 Rust、Web 和浏览器测试；
3. 修复回归；
4. 创建独立本地提交；
5. 继续下一 Milestone。

本次任务重点不是增加更多模型，也不是重新设计视觉品牌，而是：

1. 后端提供确定性的 Project Guidance；
2. 默认使用 Guided Mode；
3. 创建 Project 时直接完成关键配置；
4. Automation 默认使用自然语言 Recipe；
5. Dry Run 结果优先；
6. Run 默认 Results，Debug 按需展开；
7. Review 使用 Accept & Next；
8. Export 成为明确的旅程终点；
9. URL 和服务器状态保证上下文恢复；
10. 保留 Expert Mode、Artifact、Replay 和 Agent + Skill 架构。

除真实外部阻塞外不要停下来询问。
某项人工浏览器操作无法自动完成时，完成可自动验证部分，记录精确限制，并继续其他工作。

不要 push。
不要修改 Git remote。
不要使用、恢复或提交任何对话中出现过的 API Key。
不要执行 reset、rebase、amend 或破坏性 checkout。

只有 Release Blocking Acceptance Matrix 全部满足，或只剩明确记录的人工验证项和外部条件项时，才输出最终报告。
```
