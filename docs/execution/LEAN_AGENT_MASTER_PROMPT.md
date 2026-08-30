# AnnotAgent Lean Agent Alpha

你现在是 AnnotAgent 仓库的长期主实现工程师。

本次任务不是继续增加视觉模型、节点和页面，而是对现有架构执行一次明确的产品与工程收敛：

> 保留已经成熟的 Project、Workflow Runtime、Artifact、Review、Replay、Batch 和模型协议；删除或降级不必要的产品复杂度；把 LLM 接入为真正负责构造、校验、试跑和修订标注工具链的 Pipeline Builder Agent。

必须实际检查代码、修改实现、运行测试、修复问题并创建分阶段本地提交。

不要只输出设计文档。

不要重新实现已经工作的 Runtime。

不要把每个模型都包装成一个顶层 Skill。

不要把 LLM 变成一个空白聊天框。

不要让 LLM 生成任意代码、Shell 或 Python 后直接执行。

---

# 一、任务名称

本次长期目标：

```text
AnnotAgent Lean Agent Alpha
```

本次核心 Agent：

```text
Pipeline Builder Agent
```

本次重点应用场景：

```text
RoboCup Ball Annotation
```

全局产品名称始终为：

```text
AnnotAgent
```

RoboCup 仍然只是：

* 一个 Domain Skill；
* 一个示例 Project；
* 一组领域 Validator、Review Reason 和模板；
* 课程作业中的具体场景。

不得重新把全局产品改名为 `RoboCup AnnotAgent`。

---

# 二、当前基线必须保留

先核验代码，不要盲信文档，但以下能力若已经真实实现并通过测试，不得重写或删除：

* Project；
* Dataset；
* Label Schema；
* Workflow Draft；
* Published Immutable Workflow Version；
* 强类型 Artifact DAG；
* Dataset Batch；
* Checkpoint；
* Pause、Resume、Cancel；
* Runtime Budget；
* Artifact lineage；
* Cache；
* Replay；
* Review；
* Annotation Revision；
* Correction Memory；
* Native、COCO、YOLO、LabelMe 导出；
* OpenAI-compatible Provider；
* HTTP Vision Protocol；
* Mock Backend；
* Web GUI；
* Ratatui TUI；
* SQLite 审计与历史；
* Token、费用和模型调用记录；
* 后端重复 Run 防护；
* 服务重启后的 Run 状态恢复。

开始时执行：

```bash
git status --short --branch
git log --oneline -20
cargo test --workspace --all-features
```

然后检查：

```text
README.md
docs/
docs/execution/
crates/annotagent-core/
crates/annotagent-runtime/
crates/annotagent-application/
crates/annotagent-provider/
crates/annotagent-storage/
crates/annotagent-server/
crates/annotagent-skill-robocup/
apps/annotagent/
web/
examples/
workspace/
```

禁止：

* `git reset`
* `git rebase`
* `git commit --amend`
* 破坏性 `git checkout`
* 修改 Git remote
* push
* 使用、恢复或提交任何对话中出现过的 API Key
* 提交模型权重
* 用 Mock 结果冒充真实推理

---

# 三、重构后的最小产品架构

最终架构收敛为：

```text
AnnotAgent
│
├── Project
│   ├── Dataset
│   ├── Label Schema
│   ├── Enabled Skills
│   ├── Model Bindings
│   ├── Pipeline Drafts
│   ├── Published Pipeline Versions
│   ├── Runs
│   ├── Review
│   └── Export
│
├── Pipeline Builder Agent
│   ├── 理解标注目标
│   ├── 检查可用能力
│   ├── 构造 Pipeline Draft
│   ├── 调用静态校验
│   ├── 执行 Dry Run
│   ├── 分析结果与成本
│   ├── 修订 Draft
│   └── 提交给人工批准
│
├── Deterministic Workflow Runtime
│   ├── Typed DAG
│   ├── Artifact
│   ├── Cache
│   ├── Replay
│   ├── Batch
│   ├── Budget
│   └── History
│
├── Core Nodes
│   ├── Image Input
│   ├── Select & Map
│   ├── Crop
│   ├── Model Inference
│   ├── Validate
│   ├── Evidence Fusion
│   ├── Decision
│   ├── Human Review
│   └── Commit
│
├── Capability Skills
│   ├── Classification
│   ├── Detection
│   └── Segmentation
│
├── Domain Skills
│   └── robocup.ball
│
└── Model Backends
    ├── OpenAI-compatible VLM
    ├── HTTP Vision Worker
    ├── Mock
    └── Optional local integrations
```

核心原则：

```text
LLM 负责设计和修订流程；
Rust 负责约束、验证、执行和审计；
Published Pipeline 负责稳定运行；
人类负责批准和处理不确定结果。
```

---

# 四、需要保留、合并、降级和删除的功能

## 4.1 必须保留为一等产品能力

保留：

1. Project-centric Guided Workspace；
2. Label Schema 与 Pipeline 分离；
3. Draft、Dry Run、Publish；
4. Published Version 不可变；
5. Run 固定具体版本；
6. Artifact lineage；
7. Results / Debug 双模式；
8. Review；
9. Cache 和 Replay；
10. Batch、Checkpoint 和 Budget；
11. 通用 HTTP Vision Protocol；
12. Provider 和 Model Registry；
13. Capability Skill 与 Domain Skill；
14. RoboCup Ball 领域验证；
15. Mock 离线演示。

## 4.2 合并产品概念

### Map Label 与 Filter

Guided UI 中合并为：

```text
Select detections
```

内部仍可保留独立实现，但用户默认只看到：

```text
Keep football detections
Map model label “sports ball” to project label “football”
```

### Confidence Gate 与 Evidence Gate

Guided UI 中合并为：

```text
Decision
```

Decision 可以有不同模式：

```rust
pub enum DecisionMode {
    Confidence,
    Evidence,
    DomainPolicy,
}
```

Expert Mode 才显示真实内部节点类型。

### Candidate Match 与 Candidate Merge

Guided UI 中合并为：

```text
Combine model evidence
```

内部可以继续使用现有 Match 和 Merge 实现，但不得让普通用户先理解两个近似概念的区别。

### Workflow 与 Label Pipeline

产品语言统一使用：

```text
Automation
```

Expert Mode 和代码中仍可使用 Workflow、LabelPipeline。

## 4.3 降级为可选 Model Backend

以下不再作为产品首页的一等 Skill：

* YOLO；
* RF-DETR；
* LocateAnything；
* SAM；
* PIDNet；
* 未来其他具体模型。

它们全部属于：

```text
Model Backend
```

或：

```text
Experimental / Labs Integration
```

Capability Skill 只保留：

```text
Detection
Classification
Segmentation
```

正确关系：

```text
Detection Capability
├── VLM Detection Backend
├── YOLO HTTP Backend
├── RF-DETR HTTP Backend
├── LocateAnything Grounding Backend
└── Mock Detection Backend
```

其中 LocateAnything 可额外声明：

```text
open_vocabulary_detection
phrase_grounding
```

模型品牌不得成为 Core Node 类型。

## 4.4 降级为配置项

`Grid-assisted VLM Grounding` 不再作为独立 Skill 或主要节点。

将其改成 Detection Model Node 的可选输入预处理：

```yaml
grounding_assist:
  mode: grid
  enabled: true
  rows: 10
  columns: 10
```

用户语言：

```text
Use a positioning grid to improve coordinate accuracy
```

它仍然产生受审计的派生图片 Artifact，但不占据一条独立产品主线。

## 4.5 暂时从主界面隐藏

以下能力若没有真实 Runtime 实现，必须从默认 UI 隐藏或标记为 Labs：

* ONNX Backend；
* RF-DETR Training；
* 自动下载权重；
* Visual Prompt LocateAnything；
* 通用模型训练；
* 动态 Skill 插件安装；
* 多用户协作；
* 云端 Worker 调度。

不能让用户选择一个事实上无法执行的 Backend。

## 4.6 删除重复入口

不得同时存在：

* 全局 Workflow 页面和 Project Pipeline 页面；
* 独立 Artifact Inspector 页面和 Run Debug Inspector；
* 独立 Model Skill 页面和 Settings Models 页面；
* 多份重复 Pipeline Editor；
* 多份 Provider 设置；
* 同时编辑同一 Draft 的两套状态。

Artifact Inspector 只能作为：

```text
Run Detail → Debug
```

和：

```text
Review Detail → Execution details
```

中的上下文面板。

---

# 五、只保留一个主要 Agent

当前 Alpha 只保留一个对用户明确可见的 Agent：

```text
Pipeline Builder Agent
```

正式运行阶段主要使用确定性 Workflow Runtime。

现有 Detection Recovery 逻辑调整为：

```text
Deterministic Fallback Policy
```

而不是另一个自由规划 Agent。

运行时允许：

* 条件 Fallback；
* Evidence Decision；
* Domain Validator；
* Correction Memory；
* Human Review；
* Budget stop。

但本轮不要求每一个低置信度候选都调用一次 LLM 来决定下一步。

原因：

1. 便于调试；
2. 保证 Published Pipeline 可复现；
3. 降低费用；
4. 避免两个 Agent 同时修改流程；
5. 课程展示中只需要一条清晰、真实的 Agent Loop。

未来可以重新增加 Runtime Recovery Agent，但不属于当前 Release Blocking。

---

# 六、Pipeline Builder Agent 的职责

Pipeline Builder Agent 负责：

> 根据 Project Schema、Labels、可用 Skills、可用 Models、样本图片、成本约束和 Dry Run 结果，构造并迭代修订一个受 Registry 约束的 Pipeline Draft。

它不是一次性生成 JSON 的助手。

它必须执行真实多轮 Agent Loop：

```text
理解目标
→ 检查 Project
→ 检查 Label
→ 检查可用 Skills
→ 检查 Model Registry
→ 选择 Pipeline Template
→ 构造或修改 Draft
→ 调用 Rust Static Validator
→ 根据错误修订
→ Dry Run
→ 查看结果、失败、Review 数量、成本和 Artifact
→ 根据结果修订
→ 再次校验或 Dry Run
→ 提交给人工批准
```

至少一次完整测试必须出现：

```text
第一次 Draft 不合法
→ Static Validator 返回错误
→ LLM 修改 Draft
→ Draft 合法
→ Dry Run
→ Dry Run 结果不满足目标
→ LLM 再次修改
→ 提交人工审批
```

否则不能声称是 Agent。

---

# 七、LLM 不直接输出任意 DAG

不要让 LLM 直接返回任意 Workflow JSON 后写入数据库。

LLM 必须通过受控 Tool Calls 操作 Draft。

建议工具：

```text
inspect_project
inspect_label_schema
inspect_label
sample_images
inspect_sample_image

list_enabled_skills
load_skill_resource
list_available_capabilities
list_available_nodes
list_available_models
inspect_model

list_pipeline_templates
create_draft_from_template
create_empty_draft
add_pipeline_node
remove_pipeline_node
connect_pipeline_nodes
disconnect_pipeline_nodes
set_node_parameter
bind_model
set_label_mapping
set_decision_policy

validate_pipeline
estimate_pipeline_cost
dry_run_pipeline
inspect_dry_run_summary
inspect_failed_samples
inspect_review_samples
inspect_node_artifacts

submit_draft_for_human_approval
finish_advisor_session
```

LLM 只能使用已注册 Tool。

不得提供：

```text
run_shell
write_python
install_package
download_model
open_arbitrary_url
execute_code
```

LLM 不得直接访问数据库。

Tool 通过 Rust Application Service 操作真实 Draft。

---

# 八、限制 LLM 能构造的 Pipeline Grammar

当前 Alpha 不允许 LLM 构造任意复杂 DAG。

只允许以下文法：

```text
Shared Stages
    ↓
Label Pipeline
    ↓
Decision
    ├── Commit
    └── Review
```

共享阶段允许：

```text
Image Input
Model Inference
Optional preprocessing
```

单 Label Pipeline 允许：

```text
Source
→ Select & Map
→ Optional Crop
→ Optional Model Inference
→ Optional Validate
→ Optional Evidence Fusion
→ Decision
→ Commit / Review
```

正式约束：

1. 必须是有向无环图；
2. 一个 Label Pipeline 只能有一个正式 Commit 出口；
3. 每个不确定分支必须到 Review 或明确 Reject；
4. Commit 前必须经过 Decision；
5. 输入输出 Artifact 类型必须匹配；
6. Model 必须提供节点要求的 capability；
7. Skill 必须已经启用；
8. 不允许匿名代码节点；
9. 不允许 Workflow 内循环；
10. Fallback 最多两层；
11. 单图片模型调用上限可静态估算；
12. 无法静态估算时阻止自动发布。

后续再支持任意图。第一版就让 LLM自由构图，只会得到一种很现代的意大利面。

---

# 九、Pipeline 数据模型

复用现有数据结构，避免创建重名 DTO。

若现有类型不足，完善为：

```rust
pub struct PipelineDraft {
    pub id: WorkflowId,
    pub project_id: ProjectId,
    pub base_version: Option<WorkflowVersionId>,
    pub shared_stages: Vec<SharedWorkflowStage>,
    pub label_pipelines: Vec<LabelPipeline>,
    pub model_bindings: Vec<ModelBinding>,
    pub enabled_skills: Vec<SkillBinding>,
    pub status: WorkflowStatus,
    pub revision: u64,
    pub updated_at: DateTime<Utc>,
}
```

Agent Session：

```rust
pub struct PipelineBuilderSession {
    pub id: AgentSessionId,
    pub project_id: ProjectId,
    pub target_draft_id: WorkflowId,
    pub provider: ModelId,
    pub status: PipelineBuilderStatus,
    pub constraints: PipelineBuilderConstraints,
    pub turns: u32,
    pub tool_calls: u32,
    pub dry_runs: u32,
    pub usage: UsageSummary,
    pub stop_reason: Option<PipelineBuilderStopReason>,
}
```

状态：

```rust
pub enum PipelineBuilderStatus {
    Inspecting,
    Drafting,
    Validating,
    Testing,
    Revising,
    WaitingForHuman,
    Completed,
    Cancelled,
    BudgetExceeded,
    Failed,
}
```

停止原因：

```rust
pub enum PipelineBuilderStopReason {
    DraftReadyForHumanReview,
    HumanInputRequired,
    NoCompatibleModel,
    ValidationCouldNotBeResolved,
    DryRunTargetNotReached,
    MaximumTurnsReached,
    MaximumToolCallsReached,
    MaximumDryRunsReached,
    BudgetExceeded,
    Cancelled,
    ProviderError,
}
```

---

# 十、Agent 输入约束

用户在启动 Agent 前填写结构化目标：

```text
Target labels
Priority
Maximum cost
Maximum latency
Desired review rate
Available local workers
Whether external APIs are allowed
Whether human review is allowed
```

数据模型：

```rust
pub struct PipelineBuilderConstraints {
    pub priority: OptimizationPriority,
    pub max_cost_per_image: Option<Decimal>,
    pub max_model_calls_per_image: Option<u32>,
    pub max_expected_latency_ms: Option<u64>,
    pub target_review_rate: Option<f32>,
    pub allow_external_models: bool,
    pub allow_human_review: bool,
    pub maximum_agent_turns: u32,
    pub maximum_tool_calls: u32,
    pub maximum_dry_runs: u32,
    pub maximum_agent_cost: Decimal,
}
```

优先级：

```rust
pub enum OptimizationPriority {
    Fast,
    Balanced,
    Accurate,
    LowCost,
}
```

LLM 不能自行修改硬约束。

如果目标无法满足，应返回：

```text
NeedsHumanInput
```

或：

```text
NoValidWorkflow
```

不得静默放宽预算。

---

# 十一、Agent 的上下文管理

Agent 初始上下文只包含：

* AnnotAgent 系统约束；
* Project 摘要；
* Label Schema 摘要；
* 当前目标；
* Agent 工具定义；
* 预算；
* 已启用 Skill 的一行摘要。

只有需要时才加载：

* 具体 Skill Resource；
* Model Capability；
* 样本图片；
* Dry Run Artifact；
* Correction Memory 摘要。

不得把：

* 全部历史 Run；
* 全部 Artifact；
* 所有模型说明；
* 所有 Skill 文档；
* 完整图片库；

一次性塞进上下文。

Tool Result 必须区分：

```rust
pub struct AgentToolResult {
    pub persisted_payload: StoredPayloadRef,
    pub model_payload: serde_json::Value,
    pub display_summary: String,
}
```

LLM 获取足够完成决策的结构化结果。

GUI 只显示用户可理解的摘要。

数据库保留完整审计信息。

不得保存或展示模型隐藏思维链。

---

# 十二、保留规则式推荐作为无 Key 降级

实现两个 Advisor Backend：

```rust
pub enum PipelineAdvisorBackend {
    Llm,
    RuleBased,
    ScriptedMock,
}
```

## LLM

真实 OpenAI-compatible Provider。

## RuleBased

在没有 API Key 时：

* 根据 Annotation Kind；
* 可用 Capability；
* 已配置 Model；
* 已启用 Skill；

选择保守模板。

它不是主要 Agent，但保证产品离线可用。

## ScriptedMock

用于测试真实 Agent Loop：

* 第一次生成非法 Draft；
* Static Validator 返回错误；
* 第二次修正；
* Dry Run 返回 review rate 过高；
* 第三次增加 Crop Verify；
* 提交人工审批。

课程演示和 CI 必须可以完全离线运行。

---

# 十三、Capability Skill 收敛

只保留三个通用 Capability Skill：

```text
annotagent.classification
annotagent.detection
annotagent.segmentation
```

## Classification

支持：

* 整图分类；
* Crop 分类；
* Candidate 验证；
* 属性分类。

## Detection

支持：

* Closed-set detection；
* Open-vocabulary detection；
* Phrase grounding。

具体模型通过 capability 区分：

```rust
ObjectDetection
OpenVocabularyDetection
PhraseGrounding
```

## Segmentation

支持：

* Semantic segmentation；
* Prompted segmentation；
* Instance segmentation。

当前没有可用 Backend 时，该 capability 显示为 unavailable。

---

# 十四、模型不再是 Skill

以下全部是 Model Backend：

```text
qwen3.7-flash
YOLO worker
RF-DETR worker
LocateAnything worker
SAM worker
Mock models
```

Model Descriptor 继续保存：

* ID；
* Display Name；
* Provider；
* Capabilities；
* Version；
* Checkpoint hash；
* Label space；
* Score semantics；
* License；
* Health；
* Endpoint；
* Runtime requirements。

Skill 不应按模型品牌增长。

设置页面分组：

```text
Ready
Configured but unavailable
Experimental / Labs
Disabled
```

默认只突出可运行的模型。

当前没有权重或 Worker 未启动的：

* SAM；
* LocateAnything；
* RF-DETR；

进入：

```text
Labs
```

不得出现在 New Project 默认推荐中，除非 Health 为 available。

---

# 十五、RoboCup Ball Skill 的最小职责

`robocup.ball` 保留为唯一 Release Blocking Domain Skill。

它只提供：

1. 足球领域说明；
2. Hard-negative Validator；
3. Field Relation Validator；
4. Correction taxonomy；
5. Review reasons；
6. Pipeline Template hints；
7. Advisor Resource；
8. 可选本地 Refiner。

它不负责：

* 调用具体模型；
* 下载模型；
* 运行 SAM；
* 运行 RF-DETR；
* 运行 LocateAnything；
* 写固定 Qwen 模型 ID；
* 管理全局 Workflow。

Capability requirements：

```yaml
requires:
  - detection
  - review

optional:
  - classification
  - segmentation
  - open_vocabulary_detection
```

Agent 根据实际可用能力选择流程。

---

# 十六、RoboCup Ball 默认流程进一步简化

当前默认 Project 只标注：

```text
football bounding box
```

默认推荐流程：

```text
Image
→ Detection
→ Select football candidates
→ Optional Crop verification
→ RoboCup Ball Validator
→ Decision
    ├── Save annotation
    └── Review
```

以下只在模型可用时由 Agent 建议：

* SAM refinement；
* RF-DETR specialist-first；
* LocateAnything fallback；
* Dual-model evidence；
* Classification verifier。

不存在真实可用 Worker 时，不得默认生成这些节点。

不要让一个五张图片 Demo 的默认流程携带半个视觉研究领域。

---

# 十七、Pipeline Builder Agent 的 RoboCup 示例

用户目标：

```text
标注足球 bounding box；
优先降低白鞋误检；
允许人工 Review；
当前只有 qwen3.7-flash 可用。
```

Agent 应执行类似：

```text
inspect_project
inspect_label
list_available_models
list_enabled_skills
load_skill_resource(robocup.ball)
create_draft_from_template(vlm-detect-review)
validate_pipeline
dry_run_pipeline(images=3)
inspect_dry_run_summary
inspect_review_samples
set_node_parameter(crop_verification=true)
validate_pipeline
dry_run_pipeline(images=3)
submit_draft_for_human_approval
```

若 RF-DETR Worker 可用：

```text
list_available_models
inspect_model(rfdetr)
create_draft_from_template(specialist-first)
validate_pipeline
dry_run_pipeline
inspect_dry_run_summary
submit_draft_for_human_approval
```

Agent 不应无条件同时调用所有模型。

---

# 十八、Guided UI 集成

Pipeline Builder Agent 必须集成在：

```text
Project → Build → Automation
```

不得新增一个脱离 Project 的独立 Agent 页面。

## 18.1 默认入口

显示：

```text
How should AnnotAgent label this data?
```

输入：

* Target Label；
* Priority；
* Review preference；
* Cost limit。

主要操作：

```text
Ask AnnotAgent
```

## 18.2 Agent 执行视图

显示：

```text
Understanding your labels
Checking available models
Creating a draft
Checking the setup
Testing 3 samples
Revising the automation
Ready for review
```

不得展示隐藏思维链。

允许查看：

```text
Actions
Tool calls
Validation results
Dry run summary
Cost
Stop reason
```

## 18.3 建议结果

显示自然语言摘要：

```text
Recommended automation

1. Find football candidates using qwen3.7-flash
2. Crop uncertain candidates
3. Check RoboCup-specific false-positive risks
4. Automatically save strong results
5. Send uncertain results to Review
```

同时显示：

```text
Expected model calls
Estimated latency
Estimated cost
Expected review workload
Warnings
Unresolved bindings
```

操作：

```text
Review changes
Apply to draft
Discard
Edit manually
```

LLM 建议不得直接覆盖 Draft。

必须显示 Diff。

---

# 十九、Agent Draft Diff

实现结构化差异：

```rust
pub struct PipelineDraftDiff {
    pub added_nodes: Vec<NodeDiff>,
    pub removed_nodes: Vec<NodeDiff>,
    pub modified_nodes: Vec<NodeParameterDiff>,
    pub added_edges: Vec<EdgeDiff>,
    pub removed_edges: Vec<EdgeDiff>,
    pub model_binding_changes: Vec<ModelBindingDiff>,
    pub policy_changes: Vec<PolicyDiff>,
}
```

用户能看到：

```diff
+ Crop uncertain football candidates
+ Verify cropped candidates
- Automatically save all detections above 0.60
+ Review candidates below 0.85
```

必须支持：

* Apply all；
* Apply selected；
* Reject；
* Undo。

---

# 二十、Dry Run 反馈给 Agent

Dry Run Summary 至少包含：

```rust
pub struct AgentDryRunSummary {
    pub image_count: u32,
    pub successful_images: u32,
    pub empty_images: u32,
    pub failed_images: u32,
    pub detection_count: u32,
    pub auto_accepted_count: u32,
    pub review_count: u32,
    pub rejected_count: u32,
    pub warning_counts: BTreeMap<String, u32>,
    pub model_calls: u32,
    pub duration_ms: u64,
    pub cost: Decimal,
}
```

Agent 可以读取：

* Summary；
* 最多 N 个失败样本；
* 最多 N 个 Review 样本；
* 节点级统计；
* 结构化 Warning。

不要把全部图片和所有 Artifact 自动发回 LLM。

Agent 修改 Draft 必须说明对应证据，例如：

```text
3 个候选中有 2 个因 possible_white_shoe 进入 Review，
因此建议添加 Crop Classification。
```

这是可展示的结构化 rationale，不是隐藏思维链。

---

# 二十一、Published Runtime 保持确定性

用户批准后：

```text
Draft
→ Static Validation
→ Tested
→ Human Approved
→ Published Immutable Version
```

正式 Run：

* 不再调用 Pipeline Builder Agent；
* 不允许 Agent临时修改 Workflow；
* 固定 Model、Skill、Prompt、Node 和版本；
* 使用已有条件、Fallback 和 Decision；
* 保存完整 Artifact；
* 支持 Cache、Replay、Pause、Resume 和 Cancel。

如果用户希望修改：

```text
Published Version
→ Create new Draft
→ Agent revise
→ Test
→ Publish new Version
```

不得修改原版本。

---

# 二十二、运行时恢复进一步收敛

将当前 Detection Recovery Agent 改名或内部化为：

```text
Fallback Policy
```

它只执行已发布 Workflow 中声明的条件：

```text
empty result
low score
missing score
domain warning
worker unavailable
```

可执行动作：

```text
run configured fallback
send to review
reject
continue
```

它不能：

* 创建新节点；
* 修改 Workflow；
* 更换未绑定模型；
* 生成新 Prompt；
* 超出发布版本；
* 无限调用模型。

所有工具链设计行为统一归 Pipeline Builder Agent。

---

# 二十三、产品页面删减

全局导航保持：

```text
Home
Projects
Runs
Review
Settings
```

Project Workspace：

```text
Overview
Build
Runs
Review
Export
```

Build：

```text
Data
Labels
Automation
Test & Activate
```

Settings：

```text
Models
Capabilities
Storage
Usage
```

删除或迁移：

* 独立全局 Workflows；
* 独立 Skills 主页面；
* 独立 Models 主页面；
* 独立 Artifact Inspector；
* 重复 Debug 页面；
* 重复 Provider 配置；
* 模型品牌级 Skill 页面。

---

# 二十四、默认模式与 Expert Mode

Guided Mode 只显示：

```text
Find objects
Classify crops
Check the result
Automatically accept
Send to review
Save annotation
```

Expert Mode 显示：

```text
DetectionSetArtifact
ClassificationSetArtifact
EvidenceGate
Node ID
Model Binding
Artifact lineage
Tool Call
Raw config
Replay
```

两者使用同一份数据。

默认模式不得显示：

```text
ArtifactId
NodeId
CandidateClusterSet
ScoreSemantics
```

除非用户进入 Debug。

---

# 二十五、删除与迁移策略

不要直接删除已经工作的代码。

采用以下流程：

1. 找到所有引用；
2. 判断是否属于公开 API；
3. 提供迁移；
4. 标记 deprecated；
5. 更新 Project 和 Workflow；
6. 更新 Storage migration；
7. 更新测试；
8. 确认没有活跃引用；
9. 再删除代码。

必须生成：

```text
docs/LEAN_ARCHITECTURE_MIGRATION.md
```

记录：

* 保留；
* 合并；
* 降级；
* 删除；
* 数据迁移；
* API 迁移；
* UI 迁移；
* 向后兼容。

禁止为了“减法”删除有价值的底层能力。

主要减少的是：

* 产品暴露；
* 重复概念；
* 默认路径；
* 模型品牌耦合；
* 维护面。

---

# 二十六、测试要求

## 26.1 Agent Loop

必须测试：

```text
LLM turn 1
→ inspect project
→ create invalid draft

Rust validator
→ returns type mismatch

LLM turn 2
→ updates draft
→ validation passes
→ dry run

Dry run
→ review rate too high

LLM turn 3
→ adds crop verification
→ dry run improves
→ submits for human approval
```

验证：

* 至少两次模型调用；
* 至少四次 Tool Call；
* Tool Call 顺序合法；
* Agent 不直接 Publish；
* Agent 不直接启动正式 Run；
* 预算生效；
* Cancel 生效；
* 历史完整。

## 26.2 Security

测试 LLM 尝试：

* 调用未知 Tool；
* 绑定未知 Model；
* 使用未启用 Skill；
* 添加代码节点；
* 写 Shell；
* 修改 Published Version；
* 超出预算；
* 引用任意 URL。

全部必须被 Rust 拒绝。

## 26.3 Generic Project

无需 RoboCup：

```text
Image Classification
→ Agent recommends classifier
→ Validate
→ Dry Run
→ Human Publish
→ Run
```

页面和 Trace 不得出现 RoboCup。

## 26.4 RoboCup Project

```text
Football bbox
→ Agent loads robocup.ball
→ recommends detection + domain validation
→ Dry Run
→ revises review threshold or crop verification
→ Human Publish
```

## 26.5 Backend 可用性

当 SAM、LocateAnything、RF-DETR 不可用：

* Agent 不得推荐为默认执行节点；
* 可以在 Alternative 中说明；
* Draft 不得包含 unresolved unavailable model 后仍显示可发布；
* AnnotAgent 仍可使用 Qwen 或 Mock 完成流程。

## 26.6 Regression

现有：

* Batch；
* Pause；
* Resume；
* Cancel；
* Checkpoint；
* Replay；
* Review；
* Export；
* Run Restore；
* HTTP Vision Protocol；

不得回归。

---

# 二十七、Release Blocking Acceptance Matrix

以下全部满足后，才能声称 Lean Agent Alpha 完成。

## A. 架构减法

* [ ] 具体模型不再作为顶层 Skill。
* [ ] YOLO、RF-DETR、LocateAnything、SAM 都属于 Model Backend。
* [ ] Grid-assisted Grounding 是模型配置，不是独立产品主线。
* [ ] Guided UI 将 Filter 与 Map Label 合并呈现。
* [ ] Guided UI 将 Confidence Gate 与 Evidence Gate 合并呈现为 Decision。
* [ ] Candidate Match/Merge 不作为普通用户必须理解的两个概念。
* [ ] ONNX 未实现时不显示为可用。
* [ ] 独立 Artifact Inspector 已移除或重定向到 Run Debug。
* [ ] 没有重复 Workflow 编辑入口。
* [ ] 没有重复 Provider 设置入口。

## B. Agent 真实性

* [ ] Pipeline Builder 使用真实 LLM Tool Loop。
* [ ] Agent 会检查 Project。
* [ ] Agent 会检查 Models 和 Skills。
* [ ] Agent 通过 Tool 修改真实 Draft。
* [ ] Agent 会调用 Static Validation。
* [ ] Agent 会根据校验错误修改 Draft。
* [ ] Agent 会调用 Dry Run。
* [ ] Agent 会根据 Dry Run 结果再次修改 Draft。
* [ ] Agent 最终只提交人工审批。
* [ ] Agent 不能自动 Publish。
* [ ] Agent 不能自动启动正式 Run。
* [ ] Agent 有取消、轮数、工具数、Token 和费用停止条件。
* [ ] 不显示隐藏思维链。

## C. Pipeline 安全

* [ ] LLM 只能使用 Registry 中的节点。
* [ ] LLM 只能使用 Registry 中的模型。
* [ ] LLM 只能使用已启用 Skill。
* [ ] 不允许任意代码节点。
* [ ] 不允许 Shell。
* [ ] Pipeline Grammar 被 Rust 验证。
* [ ] Commit 前必须有 Decision。
* [ ] 不确定分支必须进入 Review 或 Reject。
* [ ] 发布版本不可变。

## D. 离线能力

* [ ] ScriptedMock 完成完整 Agent Loop。
* [ ] RuleBased Advisor 在无 Key 时可推荐模板。
* [ ] Generic Classification Demo 可离线运行。
* [ ] Generic Detection Demo 可离线运行。
* [ ] RoboCup Ball Demo 可离线运行。
* [ ] Mock 不被标记为真实模型结果。

## E. UX

* [ ] Agent 集成在 Project Automation。
* [ ] 不存在独立空白聊天页。
* [ ] 用户可以看到 Agent 当前阶段。
* [ ] 用户可以查看 Tool Actions。
* [ ] 用户可以查看 Validation 和 Dry Run Summary。
* [ ] 建议以 Diff 形式展示。
* [ ] 用户可 Apply selected。
* [ ] 用户可 Undo。
* [ ] Agent 不能直接覆盖 Draft。
* [ ] 默认模式不暴露内部 ID。

## F. RoboCup

* [ ] `robocup.ball` 仍是 Domain Skill。
* [ ] RoboCup Skill 不写死模型品牌。
* [ ] RoboCup Skill 提供 Hard-negative Validator。
* [ ] RoboCup Skill 提供 Field Relation Validator。
* [ ] RoboCup Review reasons 只在启用 Skill 时出现。
* [ ] Generic Project 不加载 RoboCup。

## G. 作业要求

* [ ] Agent Loop 核心由 Rust 实现。
* [ ] GUI 可观察和取消 Agent。
* [ ] TUI 可观察和取消 Agent。
* [ ] Provider 和模型可配置。
* [ ] Agent 进度实时显示。
* [ ] Agent Session 和 Tool Calls 可查看。
* [ ] Token 和费用按调用保存。
* [ ] RoboCup 至少两项领域定制真实存在。
* [ ] 离线课堂演示可运行。

---

# 二十八、Milestone

## Milestone 0：基线和删减清单

完成：

* 检查现有架构；
* 列出所有公开功能；
* 列出所有重复概念；
* 列出所有不可用 Backend；
* 创建迁移文档；
* 建立测试基线；
* 建立状态账本。

提交：

```text
docs: establish lean agent architecture baseline
```

## Milestone 1：Skill 与 Model 收敛

完成：

* Capability Skills 收敛为 Classification、Detection、Segmentation；
* 模型品牌迁移为 Backends；
* UI 分组；
* Labs 状态；
* Workflow migration；
* Registry migration；
* 测试。

提交：

```text
refactor(core): separate visual capabilities from model backends
```

## Milestone 2：Core Node 产品合并

完成：

* Select & Map 产品抽象；
* Decision 产品抽象；
* Evidence Fusion 产品抽象；
* Guided UI 更新；
* Expert 内部类型保留；
* Grid assist 配置化；
* 重复入口删除。

提交：

```text
refactor(workflow): simplify the public pipeline vocabulary
```

## Milestone 3：Pipeline Builder Agent Core

完成：

* Agent Session；
* Tool Registry；
* Agent Tools；
* Pipeline Grammar；
* Tool Call 历史；
* Budget；
* Stop conditions；
* ScriptedMock；
* 单元和集成测试。

提交：

```text
feat(agent): add constrained pipeline builder tool loop
```

## Milestone 4：LLM Provider 接入

完成：

* OpenAI-compatible Advisor Provider；
* Prompt 和 Tool Schema；
* 上下文管理；
* Tool Result；
* Retry；
* Timeout；
* Cancel；
* 用量和费用；
* 安全测试。

提交：

```text
feat(agent): connect the pipeline builder to llm tool calls
```

## Milestone 5：Validation 与 Dry Run 修订循环

完成：

* Agent 调用 Static Validation；
* Agent 根据错误修改 Draft；
* Agent 调用 Dry Run；
* Agent 查看 Summary；
* Agent 根据结果修改 Draft；
* 人工审批边界；
* 完整 Mock Loop。

提交：

```text
feat(agent): revise workflow drafts from validation and dry runs
```

## Milestone 6：Guided UX

完成：

* Project Automation 中的 Agent 入口；
* 结构化目标表单；
* Agent progress；
* Tool action trace；
* Draft Diff；
* Apply selected；
* Undo；
* Human Publish；
* TUI session view。

提交：

```text
feat(ui): guide users through agent-built automations
```

## Milestone 7：RoboCup Ball 精简

完成：

* 默认流程精简；
* Domain Validator；
* Domain Advisor Resource；
* 可用模型感知；
* Labs Backend 不误推荐；
* Mock Demo；
* 真实 Qwen smoke test，若有合法配置。

提交：

```text
feat(robocup): focus ball annotation on agent-selected capabilities
```

## Milestone 8：回归与发布

完成：

* 全部 Rust 测试；
* Web 测试；
* E2E；
* 100 图 Batch；
* Pause/Resume；
* Replay；
* Review；
* Export；
* 文档；
* 课程演示；
* Release Matrix。

提交：

```text
test(release): validate annotagent lean agent alpha
```

---

# 二十九、执行测试

每个 Milestone 都执行相关测试。

最终必须执行：

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

Web 使用仓库现有包管理器：

```bash
npm run typecheck
npm run test
npm run build
```

执行 E2E：

1. Generic classification Agent；
2. Generic detection Agent；
3. RoboCup Ball Agent；
4. Invalid Draft auto-revision；
5. Dry Run based revision；
6. Human approval；
7. Agent cancel；
8. Agent budget exceeded；
9. Agent refresh restore；
10. Draft Diff apply and undo；
11. Unavailable Backend not recommended；
12. Published Version immutable；
13. Generic Project no RoboCup；
14. Run、Review、Export regression。

---

# 三十、课程演示

创建：

```text
docs/DEMO_LEAN_AGENT_ALPHA.md
```

5 分钟演示：

```text
0:00–0:30
问题：视觉标注需要组合模型、裁剪、规则和人工，但用户不应手工设计所有工具链。

0:30–1:00
用户创建 football bounding-box Project。

1:00–1:30
Pipeline Builder Agent 检查 Label、Skills 和当前可用模型。

1:30–2:00
Agent 生成第一版 Draft。

2:00–2:25
Rust Static Validator 发现配置错误，Agent 根据错误修改 Draft。

2:25–3:00
Agent 对 3 张图片执行 Dry Run。

3:00–3:30
Dry Run 发现白鞋风险和 Review 比例过高，Agent 加入 Crop 验证和 RoboCup Validator。

3:30–4:00
用户查看 Diff，选择 Apply to Draft。

4:00–4:20
用户人工批准并发布不可变版本。

4:20–4:40
正式 Run 使用固定版本，不再由 Agent 临时修改。

4:40–5:00
展示 Tool Calls、Artifact、Token、费用、取消和 RoboCup 定制。
```

---

# 三十一、明确不做

本轮不做：

* 完整模型训练平台；
* 自动下载 RF-DETR、SAM、LocateAnything 权重；
* 自由无限 DAG；
* 运行时自由 LLM Recovery；
* 多 Agent 协作；
* 动态 Skill 市场；
* 云端多租户；
* 多用户权限；
* 通用 Rust ONNX Runtime；
* 视频标注；
* 完整 RoboCup 多任务；
* `robocup.robot`；
* `robocup.field`；
* 模型自动调参；
* 自动 Publish；
* 自动启动正式 Batch。

已有底层接口可以保留，但不得作为当前产品已交付能力宣传。

---

# 三十二、不得采用的假实现

禁止：

* 给固定模板加一个“AI Generated”标签就称为 Agent；
* LLM 一次返回 JSON 后直接结束；
* 前端伪造 Agent Tool Trace；
* 只让 LLM解释流程，不让它修改真实 Draft；
* Agent 不调用 Static Validator；
* Agent 不执行 Dry Run；
* Agent Dry Run 后从不修改流程；
* Agent 自动 Publish；
* Agent 直接操作数据库；
* LLM 生成代码节点；
* 把所有模型都包装成 Skill；
* 为不可用 Worker 保留主推荐；
* 仅增加新的页面，不删除重复入口；
* 删除底层审计、Cache 或 Replay；
* 重写已经通过测试的 Runtime；
* 用提高最大轮数掩盖死循环；
* 让模型输出覆盖 Tool 产生的精确 Artifact；
* push；
* 修改 remote；
* 提交 API Key。

---

# 三十三、长期状态文件

创建并持续维护：

```text
docs/execution/LEAN_AGENT_MASTER_PLAN.md
docs/execution/LEAN_AGENT_STATUS.md
docs/execution/LEAN_AGENT_DECISIONS.md
docs/execution/LEAN_AGENT_ACCEPTANCE.md
docs/execution/LEAN_AGENT_BLOCKERS.md
docs/execution/LEAN_AGENT_KNOWN_LIMITATIONS.md
```

`LEAN_AGENT_STATUS.md` 必须记录：

```text
当前 Milestone
已完成
正在进行
下一步
最近 Rust 测试
最近 Web 测试
最近 E2E
最近提交
Release Blocking 剩余项
Live-conditional 项
真实 Blocker
```

每完成一个 Milestone：

1. 更新状态；
2. 更新验收证据；
3. 执行测试；
4. 修复回归；
5. 创建独立本地提交；
6. 继续下一 Milestone；
7. 不等待用户确认。

---

# 三十四、最终报告格式

最终回复必须包含：

## 1. 删除和降级了什么

分别列出：

* 删除的重复入口；
* 合并的产品概念；
* 降级到 Labs 的 Backend；
* 保留的底层能力。

## 2. 精简后的架构

说明：

* Project；
* Pipeline Builder Agent；
* Workflow Runtime；
* Core Nodes；
* Capability Skills；
* Domain Skills；
* Model Backends。

## 3. LLM Agent Loop

说明：

* Tools；
* 多轮过程；
* Validation；
* Dry Run；
* Revision；
* Stop conditions；
* Human boundary。

## 4. Skill 与模型边界

说明：

* 为什么 YOLO、RF-DETR、LocateAnything、SAM 是 Backend；
* 为什么 Classification、Detection、Segmentation 是 Capability Skill；
* 为什么 `robocup.ball` 是 Domain Skill。

## 5. 产品体验

说明：

* Agent 如何进入 Project Automation；
* 如何显示建议；
* 如何显示 Diff；
* 如何人工批准；
* 如何进入 Expert Mode。

## 6. 测试结果

列出实际运行命令和真实结果。

不得把未执行测试写成通过。

## 7. Milestone 提交

列出：

```text
commit hash
commit message
milestone
```

## 8. Live-conditional

分别说明：

* Qwen；
* SAM；
* LocateAnything；
* RF-DETR；
* YOLO；
* 外部 Worker；
* 权重；
* 浏览器人工操作。

## 9. 未完成内容

明确区分：

```text
未实现
已实现但未验证
外部环境阻塞
明确不属于本轮
```

禁止使用：

```text
基本完成
理论上支持
应该可用
大概率正常
```

## 10. Git 状态

说明：

* 当前分支；
* 工作区；
* 领先远程提交数；
* 未 push；
* remote 未修改。

---

# 三十五、启动指令

将本文保存为：

```text
docs/execution/LEAN_AGENT_MASTER_PROMPT.md
```

然后从仓库根目录启动 Codex，并输入：

```text
阅读 docs/execution/LEAN_AGENT_MASTER_PROMPT.md，并将其作为本次长期任务的最高目标。

先核验 Git、当前代码、测试、页面、API、Workflow、Skill、Model Registry、Artifact 和已有 Agent 实现，不要盲信文档中的完成说明。

本次任务重点不是继续增加视觉模型，而是：

1. 删除或降级不必要的产品复杂度；
2. 将模型品牌收敛为 Model Backends；
3. 将通用能力收敛为 Classification、Detection、Segmentation Skills；
4. 保留 robocup.ball 作为领域 Skill；
5. 实现真实的 LLM Pipeline Builder Agent；
6. 让 Agent 通过工具修改真实 Draft；
7. 让 Agent 调用 Static Validation 和 Dry Run；
8. 让 Agent 根据结果修订 Draft；
9. 保留人工批准和不可变发布边界；
10. 保持 Runtime、Artifact、Cache、Replay、Review 和 Batch 不回归。

从 Milestone 0 开始持续执行。

普通技术选择自行决定，并记录到 LEAN_AGENT_DECISIONS.md。

每完成一个 Milestone：
1. 更新状态和验收证据；
2. 执行对应 Rust、Web 和 E2E 测试；
3. 修复回归；
4. 创建独立本地提交；
5. 继续下一 Milestone。

外部模型不可用时，继续完成：
- ScriptedMock Agent；
- RuleBased fallback；
- Runtime；
- Tool protocol；
- UI；
-测试；
-文档。

将真实模型验证标记为 live-conditional，不得用 Mock 冒充。

不要 push。
不要修改 Git remote。
不要使用、恢复或提交任何对话中出现过的 API Key。
不要提交模型权重。
不要执行 reset、rebase、amend 或破坏性 checkout。

只有 Release Blocking Acceptance Matrix 全部满足，或只剩明确记录的 live-conditional 项时，才输出最终报告。
```
