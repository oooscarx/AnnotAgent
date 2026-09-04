# AnnotAgent Builder Plan Preservation + Registry-Driven Pipeline Synthesis

你现在需要修复 AnnotAgent Pipeline Builder Agent 的一个已确认编排缺陷。

本任务不是增加更多模型，也不是再次调大 Tool Call Budget，更不是在 Prompt 中写一句“请优先使用 SAM”。

当前真实问题是：

```text
Pipeline Builder 使用 glm-5.2
→ 查询到 Ready 的 EfficientSAM Model Profile
→ 查询到完整、类型兼容的 Artifact Conversion Path
→ 证明可以构造 VLM coarse detection + prompted segmentation pipeline
→ 第 12 次 Tool Call 触发 discovery_limit_reached
→ Runtime 进入保底流程
→ 保底流程只允许 robocup.ball.vlm-bootstrap 模板
→ 已发现的 SAM 路径被丢弃
→ 最终保存成最小 VLM Draft
```

因此：

```text
LLM 已经发现正确方案
≠ Runtime 能够保存并实现该方案
```

根因不是 LLM 不知道 SAM，而是：

1. Discovery 结果只存在于临时 Tool Observation；
2. 没有持久化的 Working Plan 或 Plan Candidate；
3. Artifact Conversion Path 没有自动转为可物化的 Pipeline Fragment；
4. discovery limit 被错误地当成终止条件，而不是阶段转换；
5. fallback 仅允许硬编码模板；
6. fallback 会覆盖已经发现的更优方案；
7. 模板系统被错误地当成可生成 Pipeline 的唯一来源；
8. Builder 没有为 Draft materialization、validation 和 finalization 保留预算；
9. `FromScratch` 构建仍会受到旧默认 Workflow 和旧模板偏好的影响；
10. UI 只显示最终降级 Draft，没有显示“已经发现但被丢弃”的方案。

本任务完成后，AnnotAgent 必须做到：

```text
发现一个可行节点链
→ 持久化为 Plan Candidate
→ 根据 Registry 和 Contract 自动物化为 Draft
→ Static Validation
→ 可选 Dry Run
→ 提交人工审核
```

即使 Discovery Budget 到达上限，也必须：

```text
停止继续发现
→ 使用当前最佳 Plan Candidate
→ 创建 Runnable Draft 或 Blocked Draft
→ 保留诊断和未解析 Binding
→ 完成收尾
```

不得重新退回与已发现方案无关的最小模板。

---

# 一、任务名称

本次聚焦修复名称：

```text
AnnotAgent Builder Plan Preservation Alpha
```

核心目标：

```text
Discovery must produce durable planning state.
Budget limits must force materialization, not erase planning.
Templates may seed plans, but must not constrain all valid pipelines.
```

---

# 二、开始前核验仓库

首先执行：

```bash
git status --short --branch
git log --oneline -20
```

运行基线：

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
npm --prefix web run typecheck
npm --prefix web test
npm --prefix web run build
```

重点检查：

```text
Pipeline Builder Session
Pipeline Builder Phase
Discovery budget
Tool Call budget
Finalization reserve
Builder system prompt
Tool action mask
Context Snapshot
Node Registry
Model Registry
Skill Registry
Artifact Conversion Registry
find_artifact_conversion_path
list_compatible_models
inspect_model_profile
inspect_node_definition
Workflow Template Registry
Draft creation tools
Static Validator
Sample Test / Dry Run
Builder fallback / salvage
Builder stop reasons
Builder API DTO
Builder GUI
Builder TUI
```

搜索：

```bash
rg -n \
  "discovery_limit_reached|vlm-bootstrap|fallback|salvage|finalization|conversion_path|PlanCandidate|PipelineFragment" \
  crates apps web skills
```

核验当前真实行为，不要盲信文档。

必须先建立一个 Scripted Mock 或集成 Fixture，稳定复现：

```text
1. Builder 找到 Ready VLM Detection Model；
2. Builder 找到 Ready PromptedSegmentation Model；
3. Builder 找到合法 Conversion Path；
4. Builder 达到 Discovery Limit；
5. Runtime 丢失该路径；
6. Runtime 保存 vlm-bootstrap Draft。
```

修复前该测试必须失败。

禁止：

```text
git reset
git rebase
git commit --amend
破坏性 checkout
修改 Git remote
push
使用或恢复任何对话中出现过的 API Key
删除被历史 Run 引用的 Published Workflow
用 Mock 冒充真实 GLM、Qwen 或 EfficientSAM
```

---

# 三、明确构建模式

为 Pipeline Builder Session 增加或整理：

```rust
pub enum PipelineBuildMode {
    FromScratch,
    ImproveExisting {
        base_workflow_version_id: WorkflowVersionId,
    },
    RepairDraft {
        draft_id: WorkflowDraftId,
    },
    ResolveBindings {
        draft_id: WorkflowDraftId,
    },
}
```

语义：

## `FromScratch`

* 创建新的空 Working Draft；
* 不继承当前默认 Published Workflow；
* 不把旧 Workflow 当作 fallback；
* 可以读取 Template Catalog，但模板只是候选来源；
* 已归档和历史 Published Version 不影响排序；
* 用于用户要求“删除之前流程，从头生成”的场景。

注意：

```text
FromScratch
≠ 删除历史 Published Version
```

历史版本继续保留审计信息，但：

* 不作为默认版本；
* 不参与本次 Draft base；
* 不影响 Builder fallback；
* 可以标记为 Archived 或 Retired for new runs。

## `ImproveExisting`

只在用户明确选择现有版本时，以该版本作为 base。

## `RepairDraft`

继续修改指定 Draft。

## `ResolveBindings`

模型安装或 Provider 配置后，从已有 Blocked Draft 继续，不重新发现全部 Pipeline。

Builder Session、API、URL、History 和 Trace 必须保存 `build_mode`。

---

# 四、创建持久化 Working Draft

Builder Session 创建时，立即创建一个真实持久化 Working Draft：

```rust
pub struct BuilderWorkingDraft {
    pub draft_id: WorkflowDraftId,
    pub session_id: AgentSessionId,
    pub project_id: ProjectId,
    pub build_mode: PipelineBuildMode,
    pub revision: u64,
    pub state: WorkingDraftState,
}
```

初始 Draft 可以为空，但必须存在。

目的：

* Discovery 不再发生在“没有任何可保存对象”的真空中；
* Agent 每发现一个有价值片段，都能应用到 Working Draft；
* Budget 到达时至少能保存当前结构；
* Retry 可以从同一个 Draft 继续；
* UI 可以实时显示正在形成的 Pipeline。

不得继续允许：

```text
Tool Calls = 12
Draft count = 0
```

---

# 五、引入持久化 Plan Candidate

实现：

```rust
pub struct PipelinePlanCandidate {
    pub id: PlanCandidateId,
    pub session_id: AgentSessionId,
    pub project_id: ProjectId,

    pub target_labels: Vec<LabelId>,
    pub target_annotation_kinds: Vec<TaskKind>,

    pub source: PlanCandidateSource,
    pub status: PlanCandidateStatus,

    pub fragments: Vec<PipelineFragment>,
    pub proposed_model_bindings: Vec<ProposedModelBinding>,
    pub unresolved_requirements: Vec<UnresolvedModelRequirement>,

    pub evidence_refs: Vec<BuilderObservationRef>,
    pub capability_coverage: CapabilityCoverage,
    pub geometry_safety: PlanGeometrySafety,
    pub estimated_cost: Option<Decimal>,
    pub estimated_model_calls: Option<u32>,

    pub deterministic_score: PlanCandidateScore,
    pub rejection_reasons: Vec<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

来源：

```rust
pub enum PlanCandidateSource {
    LlmProposed,
    ConversionPath,
    WorkflowTemplate,
    ExistingDraft,
    DeterministicSynthesis,
    Salvage,
}
```

状态：

```rust
pub enum PlanCandidateStatus {
    Discovered,
    Selected,
    Materialized,
    Validated,
    Blocked,
    Rejected,
    Superseded,
}
```

重要规则：

1. Plan Candidate 是持久化对象；
2. Builder Session 结束后仍可查看；
3. Discovery limit 不得删除 Candidate；
4. Candidate 保存来源 Observation；
5. Candidate 保存所需模型和 Capability；
6. Candidate 可以是 Runnable 或 Blocked；
7. Candidate 不等于 Published Workflow；
8. Candidate 可以被物化为 Working Draft；
9. 一个 Session 可以有多个 Candidate；
10. Runtime 必须能够确定性选择最佳 Candidate。

---

# 六、将 Artifact Conversion Path 变成一等 Pipeline Fragment

当前 `find_artifact_conversion_path` 不能只返回：

```text
DetectionSet can be converted to BoundingBox through ...
```

它必须返回并持久化一个可以直接物化的结构：

```rust
pub struct PipelineFragment {
    pub id: PipelineFragmentId,
    pub source_artifact_type: ArtifactType,
    pub target_artifact_type: ArtifactType,

    pub nodes: Vec<NodeBlueprint>,
    pub edges: Vec<EdgeBlueprint>,

    pub required_capabilities: Vec<ModelCapability>,
    pub compatible_model_profiles: Vec<ModelProfileId>,
    pub unresolved_requirements: Vec<UnresolvedModelRequirement>,

    pub contract_hashes: Vec<String>,
    pub registry_revision: String,
}
```

例如发现：

```text
DetectionSet
→ BoxPromptSet
→ MaskSet
→ DetectionSet
```

必须返回：

```text
NodeBlueprint:
core.detections_to_box_prompts

NodeBlueprint:
capability.segment
required capability: PromptedSegmentation
compatible ready model: EfficientSAM profile

NodeBlueprint:
core.mask_to_bbox
```

而不是仅返回自然语言描述。

成功发现一条合法路径后，Runtime 自动：

1. 保存 `PipelineFragment`；
2. 创建或更新 `PlanCandidate`；
3. 发布 `PlanCandidateDiscovered` Event；
4. 将 Candidate 摘要加入 Builder Context；
5. 将 Builder 标记为 `feasible_candidate_found`；
6. 缩小后续可见工具；
7. 要求下一步进入 Drafting。

不要依赖 LLM 再额外记得调用一个“保存发现结果”的工具。

工具成功本身就应沉淀规划状态。

---

# 七、增加 Registry-Driven Pipeline Synthesizer

模板不能继续作为唯一的 Pipeline 生成路径。

实现：

```rust
pub struct RegistryPipelineSynthesizer;
```

输入：

```rust
pub struct PipelineSynthesisRequest {
    pub project_id: ProjectId,
    pub target_labels: Vec<LabelId>,
    pub target_annotation_kind: TaskKind,

    pub enabled_skills: Vec<SkillRef>,
    pub ready_models: Vec<ModelProfileSummary>,
    pub node_registry_revision: String,
    pub conversion_registry_revision: String,

    pub project_geometry_policy: ProjectGeometryPolicy,
    pub constraints: PipelineBuilderConstraints,
}
```

输出：

```rust
pub struct PipelineSynthesisResult {
    pub candidates: Vec<PipelinePlanCandidate>,
    pub unsupported_reasons: Vec<String>,
    pub unresolved_requirements: Vec<UnresolvedModelRequirement>,
}
```

Synthesizer 使用：

* Node input/output contracts；
* Model capabilities；
* Ready status；
* Skill requirements；
* Artifact conversion paths；
* Geometry Safety；
* Project annotation kind；
  -成本和调用限制；
  -允许的 Pipeline Grammar。

它至少能构造以下候选：

## VLM 保守路径

```text
Image
→ VLM coarse detection
→ Select & map
→ Domain validation
→ Mandatory review
→ Commit
```

## VLM + Prompted Segmentation

```text
Image
→ VLM coarse detection
→ Select & map
→ Domain semantic validation
→ Detections to box prompts
→ Prompted segmentation
→ Mask to bbox
→ Geometry evaluation
→ Geometry decision
    ├── pass → Commit
    └── review → Human review → Commit
```

## Specialist Detection

```text
Image
→ Specialist detection
→ Select & map
→ Domain validation
→ Geometry decision
→ Commit / Review
```

## Open-vocabulary Cold Start

```text
Image
→ Open-vocabulary detection
→ Select & map
→ Verification
→ Decision
→ Commit / Review
```

具体模型由 Ready Model Profile 绑定。

Synthesizer 不得硬编码：

```text
EfficientSAM
Qwen
RF-DETR
YOLO
LocateAnything
```

只依赖 Capability、Contract 和状态。

---

# 八、模板降级为 Candidate Seed

保留现有 Workflow Templates，但改变其地位。

模板只能：

```text
提供一个初始 Candidate
```

不能：

```text
作为 Budget limit 时唯一允许的最终 Draft
```

特别是：

```text
robocup.ball.vlm-bootstrap
```

可以继续作为一个模板，但必须移除任何类似以下的硬编码：

```rust
if discovery_limit_reached {
    return robocup_ball_vlm_bootstrap();
}
```

或：

```rust
allowed_salvage_templates = ["robocup.ball.vlm-bootstrap"];
```

新的优先顺序：

```text
1. 已经物化并通过校验的 Working Draft
2. 已选中的 Plan Candidate
3. 最高分 Runnable Plan Candidate
4. 最高分 Blocked Plan Candidate
5. Registry Synthesizer 产生的最小安全 Candidate
6. Workflow Template Candidate
7. Unsupported / Setup Required
```

模板不得覆盖已经发现的更完整 Candidate。

---

# 九、重新定义 Discovery Limit

`discovery_limit_reached` 不再是失败终态。

它应该是一个阶段事件：

```text
FeasibilityAnalysis
→ DraftSalvage
```

增加：

```rust
pub enum PipelineBuilderPhase {
    ContextLoading,
    FeasibilityAnalysis,
    CandidateSelection,
    Drafting,
    DraftSalvage,
    Validating,
    DryRunning,
    Revising,
    Finalizing,
    WaitingForHuman,
    Completed,
    Cancelled,
    Failed,
}
```

当 Discovery Limit 到达时：

```text
1. 禁止继续调用 Discovery Tools；
2. 冻结当前 Context Snapshot；
3. 收集全部 Plan Candidates；
4. 使用确定性排序选择最佳 Candidate；
5. 将其物化为 Working Draft；
6. 执行 Static Validation；
7. 保存 Runnable Draft 或 Blocked Draft；
8. 提交人工审核或 Setup Requirement。
```

不得：

```text
清空 Candidate
→ 重新选默认模板
```

---

# 十、增加 Finalization Budget Escrow

总 Tool Call Budget 必须保留收尾额度。

例如：

```rust
pub struct PipelineBuilderBudget {
    pub max_total_tool_calls: u32,
    pub max_discovery_tool_calls: u32,
    pub max_draft_mutations: u32,
    pub max_validation_repairs: u32,
    pub max_dry_runs: u32,

    pub reserved_materialization_calls: u32,
    pub reserved_validation_calls: u32,
    pub reserved_finalization_calls: u32,
}
```

推荐默认：

```text
max total calls                 24
max discovery calls              8
reserved materialization         3
reserved validation              3
reserved finalization            3
```

如果当前产品仍保留 12 次总预算，则至少：

```text
Context / Discovery       ≤ 5
Materialization           2
Validation                2
Finalization              2
Reserve                   1
```

核心不是具体数字，而是：

> Discovery 不能消费 Draft 和 Finalization 的配额。

当剩余预算进入保留区：

```text
Inspect tools 全部移除
Conversion path discovery 移除
只暴露：
- select_plan_candidate
- materialize_plan_candidate
- validate_pipeline
- create_blocked_draft
- submit_draft_for_human_approval
- finish_with_setup_requirements
```

---

# 十一、成功发现路径后立即停止继续探索

增加运行时进展规则：

```rust
pub struct BuilderProgressRules {
    pub maximum_calls_without_candidate: u32,
    pub maximum_calls_after_runnable_candidate: u32,
    pub draft_deadline_tool_call: u32,
}
```

默认行为：

```text
发现第一个完整 Runnable Candidate
→ 最多允许再比较 1 个 Alternative
→ 随后必须选择并物化
```

如果 Candidate 同时满足：

* 目标 Artifact；
* Geometry Safety；
* Ready Model bindings；
* Project constraints；
  -有效 Review / Commit path；

Runtime 将其标记：

```text
CandidateSufficiency::Complete
```

之后移除所有普通 discovery tools。

不能允许模型已经找到：

```text
Qwen Detection
→ EfficientSAM
→ MaskToBBox
```

之后继续检查十四个无关节点，仿佛完整方案只是闲聊时偶然提到。

---

# 十二、Plan Candidate 的确定性排序

实现明确排序，不依赖 fallback 随意选择。

排序优先级：

1. `Runnable` 高于 `Blocked`；
2. 满足 Project output type；
3. 满足 Geometry Safety；
4. 所有 required Model 都是 `Ready`；
5. 有完整 Review / Commit 路径；
6. Domain Validator 已启用；
7. 未使用 Fixture 或 Mock；
8. 无 unresolved binding；
9. 满足用户 Accuracy / Cost / Latency priority；
10. 更少模型调用；
11. 更低估算成本；
12. 更短合法节点链；
13. Template 不能获得隐藏额外权重。

保存结构化分数：

```rust
pub struct PlanCandidateScore {
    pub runnable: bool,
    pub goal_coverage: f32,
    pub geometry_safety: f32,
    pub binding_completeness: f32,
    pub domain_coverage: f32,
    pub estimated_cost: Option<Decimal>,
    pub estimated_model_calls: Option<u32>,
    pub deterministic_total: i64,
    pub reasons: Vec<String>,
}
```

不要制造伪概率。

---

# 十三、SAM 路径的正确物化

在当前 Project 满足以下条件时：

```text
Target: football bounding box
Ready VLM Detection model
Ready PromptedSegmentation model
robocup.ball enabled
DetectionSet → BoxPromptSet → MaskSet → DetectionSet path exists
```

应当能够产生类似 Draft：

```text
core.image_input
    ↓
vlm_detection.detect
    model: current compatible VLM profile
    geometry: CoarseHypothesis
    ↓
core.select_and_map
    target label: football
    ↓
robocup.ball_hard_negative
    ↓
core.detections_to_box_prompts
    ↓
capability.segment
    capability: PromptedSegmentation
    model: current Ready prompted-segmentation profile
    ↓
core.mask_to_bbox
    ↓
core.evaluate_geometry
    ↓
core.decision
    mode: Geometry
    ├── pass → core.commit
    └── review → core.human_review → core.commit
```

具体 Node ID 使用 Registry 中的实际 ID。

如果 RoboCup Hard-negative Validator 需要先做语义确认，应位于几何精修前。

如果 Validator Contract 要求另一顺序，以 Registry Contract 为准。

不得在生产代码中写死 `EfficientSAM`，但集成 Fixture 可以使用该 Ready Model Profile 验证。

---

# 十四、没有 PromptedSegmentation 时的行为

如果没有 Ready 的 PromptedSegmentation Model：

```text
Image
→ VLM coarse detection
→ Select
→ Domain validation
→ Mandatory review
→ Commit
```

同时保存：

```text
Optional improvement:
PromptedSegmentation capability is unavailable.
```

可以创建：

```text
Blocked alternative candidate
```

但不能将不可用模型放进 Runnable Draft。

关键区别：

```text
模型不可用
→ 安全降级
```

不是：

```text
模型已经发现但被 Runtime 丢弃
→ 错误降级
```

---

# 十五、增加 Builder Working Memory

Discovery Observation 不得只存在于不断增长的对话消息。

实现：

```rust
pub struct BuilderWorkingMemory {
    pub session_id: AgentSessionId,
    pub context_revision: String,

    pub project_facts: BTreeMap<String, BuilderFact>,
    pub model_facts: BTreeMap<ModelProfileId, ModelFact>,
    pub node_facts: BTreeMap<NodeDefinitionId, NodeFact>,
    pub conversion_paths: BTreeMap<ConversionPathId, PipelineFragment>,

    pub plan_candidates: Vec<PlanCandidateId>,
    pub selected_candidate_id: Option<PlanCandidateId>,
    pub working_draft_id: WorkflowDraftId,
}
```

Tool Result 完整内容继续持久化。

模型上下文只接收：

* Working Memory Digest；
* 新增事实；
* Candidate 摘要；
* Draft Diff；
* Validation Summary；
* Dry Run Summary。

阶段切换时，不要把全部旧 Catalog Observation 逐字重发。

---

# 十六、Builder System Prompt 更新

更新 Builder Prompt，但不能只靠 Prompt。

加入：

```text
You are editing a persistent working draft.

Planning rules:

1. Start from the provided registry and compatibility snapshot.
2. When a valid model and artifact conversion path have been found,
   stop broad discovery.
3. A successful conversion-path discovery is saved automatically as
   a plan candidate.
4. Select and materialize a plan candidate before the drafting
   deadline.
5. Do not continue inspecting unrelated nodes after a complete
   runnable candidate exists.
6. Workflow templates are optional seeds, not the only allowed plans.
7. Prefer a registry-composed candidate over a simpler template when:
   - all required models are ready;
   - artifact contracts match;
   - geometry safety is stronger;
   - project constraints are satisfied.
8. If discovery budget is reached, use the best saved candidate.
9. Never discard a discovered prompted-segmentation path merely
   because a bootstrap template exists.
10. If no runnable candidate exists, create a blocked draft with
    unresolved requirements.
11. All proposals remain drafts.
12. You cannot publish or start a full dataset run.
```

---

# 十七、Fallback 和 Salvage 必须区分

定义：

## Workflow Fallback

正式 Published Workflow 执行过程中，根据声明条件切换节点。

## Builder Salvage

Builder 达到阶段限制后，将最佳规划结果保存为 Draft。

这两个概念不能共用：

```text
fallback_template
```

增加：

```rust
pub enum BuilderSalvageOutcome {
    RunnableDraftMaterialized,
    BlockedDraftMaterialized,
    ExistingDraftPreserved,
    UnsupportedRequest,
}
```

Builder Salvage 不允许调用某个 Domain Template 作为无条件默认。

---

# 十八、Builder Outcome 与 Stop Reason

实现或整理：

```rust
pub enum PipelineBuilderOutcome {
    DraftReadyForHumanReview,
    BlockedDraftReady,
    ProviderSetupRequired,
    UnsupportedRequest,
    Cancelled,
    Failed,
}
```

```rust
pub enum PipelineBuilderStopReason {
    CandidateMaterialized,
    DiscoveryLimitTriggeredSalvage,
    DraftValidated,
    SetupRequired,
    NoFeasibleCandidate,
    ValidationRepairLimit,
    DryRunLimit,
    TotalBudgetExhausted,
    Cancelled,
    ProviderError,
}
```

示例：

```text
Outcome:
DraftReadyForHumanReview

Stop reason:
DiscoveryLimitTriggeredSalvage

Selected candidate:
VLM coarse detection + prompted segmentation

Draft preserved:
Yes
```

而不是：

```text
Agent could not complete that action.
```

如果最终 Draft 已经产生，阶段限制不应显示成失败。

---

# 十九、API 与 UI

Builder Session API 增加：

```text
build_mode
phase
working_draft_id
plan_candidates
selected_candidate_id
candidate_count
runnable_candidate_count
blocked_candidate_count
discovered_conversion_paths
salvage_outcome
outcome
stop_reason
```

## UI 执行阶段

显示：

```text
Checking project
Checking available models
Finding compatible processing paths
Plan candidate saved
Creating draft
Checking draft
Ready for review
```

## Candidate 面板

显示：

```text
Recommended candidate

VLM coarse detection
→ RoboCup validation
→ Prompted segmentation
→ Geometry evaluation
→ Review / Commit

Models:
- current VLM profile
- current prompted-segmentation profile

Status:
Runnable
```

Alternative：

```text
VLM detection
→ Mandatory review

Status:
Runnable, lower automation
```

## 达到 Discovery Limit

如果成功 Salvage：

```text
Discovery limit reached

AnnotAgent stopped searching and created a draft from the best
compatible plan already found.

[Review draft]
```

不能显示成失败。

## 已发现但未物化

UI 必须显示：

```text
Plan discovered but not applied
```

这本身应被视为 Builder 异常，并进入诊断，而不是静默消失。

---

# 二十、清理旧流程时的正确行为

用户选择：

```text
Build from scratch
```

执行：

1. 创建新的 `FromScratch` Session；
2. 创建新的 Working Draft；
3. 不以旧 default workflow 为 base；
4. 旧 Draft 可以归档；
5. 旧 Published Version 保留审计；
6. 取消旧版本作为 Project default；
7. 当前新生成流程不使用旧版本 fallback；
8. 历史 Run 继续指向旧 Published Version；
9. 不删除历史 Artifact；
10. 不删除旧校准和评测记录，但它们只能作为历史证据。

不要通过破坏不可变历史来实现“从头生成”。

---

# 二十一、必须增加的测试

## Case 1：精确复现当前缺陷

配置：

```text
Builder model:
glm-style scripted planner

Ready models:
- image-capable structured VLM
- prompted-segmentation model

Ready nodes:
- VLM detection
- select/map
- detections to box prompts
- prompted segmentation
- mask to bbox
- geometry evaluation
- decision
- review
- commit

Domain skill:
robocup.ball
```

脚本行为：

1. 查询 VLM；
2. 查询 PromptedSegmentation Model；
3. 查找 Conversion Path；
4. 得到完整路径；
5. 尝试继续 Discovery；
6. 触发 discovery limit。

预期：

* Conversion Path 被持久化；
* Plan Candidate 被创建；
* Runtime 进入 DraftSalvage；
* Candidate 被物化；
* Draft 含 PromptedSegmentation；
* 不退回 `robocup.ball.vlm-bootstrap`；
* Static Validation 通过；
* Outcome 为 `DraftReadyForHumanReview`；
* Stop Reason 为 `DiscoveryLimitTriggeredSalvage`；
* 不返回 Budget Exhausted。

## Case 2：发现路径后继续重复 Inspect

预期：

* discovery tools 被移除；
  -重复 Inspect 被拒绝或缓存；
  -下一步只能 Candidate Selection / Drafting。

## Case 3：模板与 Registry Candidate 竞争

候选：

```text
A. vlm-bootstrap + mandatory review
B. VLM + Ready PromptedSegmentation + geometry decision
```

在 Accuracy/Balanced 模式下：

* B 被选择；
* A 作为 Alternative；
  -模板没有隐藏优先级。

在 LowCost 模式下：

* 可以选择 A；
  -必须给出成本理由；
  -不能因为 fallback hardcode 选择 A。

## Case 4：没有 PromptedSegmentation Model

预期：

* 安全 VLM + mandatory review Draft；
* SAM Candidate 为 Blocked Alternative；
  -无假 Binding；
  -不循环 Discovery。

## Case 5：Model 在 Discovery 后变为 unavailable

预期：

* Candidate 标记 Stale 或 Blocked；
  -物化时重新校验 Registry revision；
  -生成安全 Review Draft；
  -不使用过期 Ready 状态。

## Case 6：FromScratch

存在旧 Published VLM bootstrap。

启动：

```text
BuildMode::FromScratch
```

预期：

-旧版本不作为 base；
-旧版本不作为强制 fallback；
-新 Candidate 根据当前 Registry 生成；
-历史版本不删除。

## Case 7：ImproveExisting

显式选择旧版本。

预期：

-旧版本作为 base；
-发现的 PromptedSegmentation Fragment 以 Patch 应用；
-不是完全重建。

## Case 8：Draft Deadline

LLM 一直不主动创建 Draft。

预期：

* Runtime 根据最佳 Candidate确定性物化；
  -仍然得到 Draft；
  -不以“LLM 没调用 create”失败。

## Case 9：Blocked Draft

Conversion Path 完整，但没有兼容 Model。

预期：

-创建节点结构；
-添加 unresolved model requirement；
-Outcome 为 `ProviderSetupRequired`；
-安装模型后可 Retry。

## Case 10：Builder UI

预期：

-显示 Candidate discovered；
-显示 Candidate selected；
-显示 Draft materialized；
-显示 Discovery limit salvage；
-不显示通用失败；
-HTML entity 和转义字符不泄漏。

---

# 二十二、真实当前 Project 验收

在自动测试通过后，对当前足球 Project 执行一次真实 Builder 测试。

要求：

1. 不修改历史 Published Version；
2. 创建 `FromScratch` Session；
3. 使用当前配置的 Builder LLM；
4. 读取当前 Ready Model Profiles；
5. 确认 PromptedSegmentation Model Profile 的：

   * status；
   * capability；
   * contract；
   * model instance；
   * fixture/production eligibility；
6. 如果模型是真实 Ready 且 production-eligible：

   * 生成 PromptedSegmentation Pipeline；
7. 如果仅为 Fixture：

   * 不进入正式 Runnable Draft；
   * 创建 Blocked Alternative；
   * 主 Draft 使用 Mandatory Review；
8. 执行 Static Validation；
9. 执行 Sample Test，前提是所有真实 Binding 可用；
10. 不自动 Publish。

最终记录：

```text
Builder model
Tool call count
Discovery calls
Plan candidates
Selected candidate
Node chain
Model bindings
Validation status
Sample Test status
Outcome
Stop reason
```

不得仅凭名称 `EfficientSAM` 判断是否可以进入正式 Workflow。

必须确认：

```text
fixture_only = false
production_eligible = true
ModelInstanceStatus = Ready
```

---

# 二十三、验收指标

当前缺陷修复后必须满足：

| 指标                 | 当前错误行为         | 目标           |
| ------------------ | -------------- | ------------ |
| 完整 Conversion Path | 临时 Observation | 持久化 Fragment |
| Plan Candidate     | 无              | 至少 1 个       |
| Discovery Limit    | 丢弃方案           | 触发 Salvage   |
| Final Draft        | 硬编码 bootstrap  | 最佳 Candidate |
| Ready SAM 路径       | 被丢弃            | 被物化          |
| Draft count        | 0 或错误 Draft    | 1            |
| Template 权限        | 唯一 fallback    | 普通候选来源       |
| FromScratch        | 受旧默认影响         | 与旧版本隔离       |
| Tool calls         | 到上限失败          | 到上限仍有 Draft  |
| UI                 | 通用失败           | 方案、收尾和下一步    |
| Published history  | 可能想删除          | 保留且不影响新建     |

---

# 二十四、Milestone

## Milestone 0：回归 Fixture

完成：

* 复现发现 SAM 路径后丢失；
  -记录当前 Tool Sequence；
  -记录 fallback 代码；
  -建立状态文档。

提交：

```text
test(builder): reproduce discovered pipeline loss at discovery limit
```

## Milestone 1：Working Draft 与 Plan Candidate

完成：

* BuildMode；
  -Working Draft；
  -Plan Candidate；
  -Pipeline Fragment；
  -持久化；
  -Events；
  -API；
  -测试。

提交：

```text
feat(builder): persist working plans and drafts during discovery
```

## Milestone 2：Conversion Path Materialization

完成：

* Conversion Path 输出 Blueprint；
  -自动 Candidate；
  -Fragment materializer；
  -Contract validation；
  -测试。

提交：

```text
feat(workflow): materialize registry conversion paths into draft fragments
```

## Milestone 3：Registry-Driven Synthesizer

完成：

-目标驱动 graph synthesis；
-候选排序；
-模型绑定；
-Geometry Safety；
-Template seed；
-测试。

提交：

```text
feat(builder): synthesize pipelines from registry contracts and capabilities
```

## Milestone 4：Budget Salvage

完成：

* Discovery limit phase transition；
  -finalization reserve；
  -action mask；
  -deterministic salvage；
  -outcome；
  -stop reason；
  -测试。

提交：

```text
fix(builder): salvage the best discovered plan before budget termination
```

## Milestone 5：Prompt、API 和 UI

完成：

* Builder Prompt；
  -candidate UI；
  -salvage state；
  -FromScratch；
  -错误文本；
  -Retry；
  -Web/TUI 测试。

提交：

```text
fix(ui): expose preserved builder plans and salvage outcomes
```

## Milestone 6：当前 Project 与 Release 回归

完成：

-真实 Builder 测试；
-Static Validation；
-Sample Test，条件允许时；
-全部 Rust/Web/E2E；
-文档；
-验收证据。

提交：

```text
test(release): validate registry-driven pipeline synthesis and plan preservation
```

---

# 二十五、Release Blocking Acceptance Matrix

## A. 计划持久化

* [ ] Builder Session 创建时已有 Working Draft。
* [ ] Conversion Path 会持久化为 Pipeline Fragment。
* [ ] Fragment 会产生 Plan Candidate。
* [ ] Candidate 在 Session 结束后仍可查看。
* [ ] Candidate 保存模型、Contract 和 Observation 来源。
* [ ] Candidate 不因 Discovery Limit 丢失。

## B. Registry 合成

* [ ] Pipeline 不只来自 Template。
* [ ] Registry 可以合成合法节点链。
* [ ] Artifact Conversion Path 可物化。
* [ ] Model Capability 和状态参与选择。
* [ ] Geometry Safety 参与选择。
* [ ] Domain Skill 参与选择。
* [ ] 模型品牌不进入 Core 分支。
* [ ] 模板没有隐藏优先级。

## C. Budget 和 Salvage

* [ ] Discovery Budget 与 Finalization Budget 分离。
* [ ] Discovery Limit 是阶段转换，不是失败。
* [ ] 达到 Limit 后移除 Inspect Tools。
* [ ] Runtime 选择最佳 Candidate。
* [ ] 最佳 Candidate 被物化。
* [ ] 至少保留 Validation 和 Submit 配额。
* [ ] 有 Candidate 时不回退无关模板。
* [ ] 无 Runnable Candidate 时创建 Blocked Draft。

## D. SAM 路径

* [ ] Ready PromptedSegmentation Model 可进入 Candidate。
* [ ] DetectionSet → BoxPromptSet → MaskSet → DetectionSet 可物化。
* [ ] 路径包含 Geometry Evaluation。
* [ ] 路径包含 Commit / Review。
* [ ] SAM unavailable 时安全降级。
* [ ] Fixture 模型不能进入 production runnable Draft。
* [ ] `robocup.ball` 不写死 EfficientSAM。

## E. Build Mode

* [ ] FromScratch 不继承旧 Workflow。
* [ ] FromScratch 不使用旧 default fallback。
* [ ] ImproveExisting 使用明确 base version。
* [ ] RepairDraft 从当前 Draft 继续。
* [ ] ResolveBindings 不重新发现全部 Catalog。
* [ ] 历史 Published Version 保持不可变。

## F. 产品

* [ ] UI 显示 Plan Candidate。
* [ ] UI 显示 Selected Candidate。
* [ ] UI 显示 Materialization。
* [ ] Discovery Limit Salvage 不显示为失败。
* [ ] 用户可以打开保存的 Draft。
* [ ] 用户可以比较 Alternative。
* [ ] Builder 不自动 Publish。
* [ ] Builder 不自动开始完整 Dataset Run。

## G. 回归

* [ ] Static Validator 不回归。
* [ ] Geometry Safety 不回归。
* [ ] Provider Registry 不回归。
* [ ] Model Bundle 不回归。
* [ ] Artifact lineage 不回归。
* [ ] Batch 不回归。
* [ ] Replay 不回归。
* [ ] Review 不回归。
* [ ] Export 不回归。

---

# 二十六、最终测试

执行：

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
cargo build --workspace --all-features
```

Web：

```bash
npm --prefix web run typecheck
npm --prefix web test
npm --prefix web run build
npm --prefix web run test:e2e
```

必须包含以下 E2E：

1. GLM-style discovery；
2. Ready PromptedSegmentation；
3. discovery limit；
4. candidate preservation；
5. materialized SAM path；
6. template alternative；
7. FromScratch；
8. ImproveExisting；
9. unavailable SAM；
10. fixture-only SAM；
11. blocked draft；
12. retry after model setup；
13. UI candidate trace；
14. no auto-publish。

---

# 二十七、不得采用的假修复

禁止：

* 把 Discovery Limit 从 12 改成 24；
  -把总 Tool Call 从 48 改成 96；
  -只修改 Builder Prompt；
  -只告诉 GLM“找到 SAM 后快点创建 Draft”；
  -把 EfficientSAM 写死进 RoboCup 模板；
  -新增一个专用 SAM fallback template 作为唯一修复；
  -仍然只允许模板产生 Pipeline；
  -达到 Limit 后清空 Candidate；
  -让 fallback 覆盖 Working Draft；
  -删除历史 Published Workflow；
  -把 Fixture 模型当成正式模型；
  -自动 Publish；
  -自动运行完整数据集；
  -用 Mock 冒充真实 EfficientSAM；
  -修改 Git remote；
  -push；
  -使用或恢复任何对话中出现过的 API Key。

---

# 二十八、最终报告格式

最终报告必须包含：

## 1. 根因

说明：

* GLM 发现了什么；
  -发现结果保存在哪里；
  -为什么 Discovery Limit 后丢失；
  -哪个 fallback 覆盖了方案。

## 2. Working Plan

说明：

* Working Draft；
  -Plan Candidate；
  -Pipeline Fragment；
  -Builder Working Memory。

## 3. Registry Synthesis

说明：

-如何从 Artifact Contract 构造节点链；
-模板角色如何降级；
-模型和 Skill 如何参与。

## 4. Budget Salvage

说明：

-阶段预算；
-finalization reserve；
-action mask；
-deterministic salvage；
-outcome 和 stop reason。

## 5. SAM Pipeline

说明实际生成的：

```text
Detection
→ Prompt
→ Segmentation
→ Mask-to-BBox
→ Geometry Evaluation
→ Decision
```

以及使用的 Model Profile 状态。

## 6. Build Mode

说明：

-FromScratch；
-ImproveExisting；
-RepairDraft；
-ResolveBindings。

## 7. 当前 Project 实测

列出：

```text
Builder model
Tool calls
Plan candidates
Selected candidate
Final node chain
Model bindings
Validation
Sample Test
Outcome
Stop reason
```

## 8. 测试

列出实际执行命令和真实结果。

不得把未执行测试写成通过。

## 9. Milestone 提交

列出：

```text
commit hash
commit message
milestone
```

## 10. 未完成内容

明确区分：

```text
未实现
已实现但未真实模型验证
外部环境限制
不属于本轮
```

禁止使用：

```text
基本完成
理论上支持
应该不会再丢
大概率能生成
```

## 11. Git 状态

说明：

* 当前分支；
  -工作区是否干净；
  -领先远程提交数；
  -未 push；
  -remote 未修改。

---

# 二十九、启动指令

将本文保存为：

```text
docs/execution/BUILDER_PLAN_PRESERVATION_MASTER_PROMPT.md
```

然后从 AnnotAgent 仓库根目录启动 Codex，输入：

```text
阅读 docs/execution/BUILDER_PLAN_PRESERVATION_MASTER_PROMPT.md，
并将其作为本次聚焦修复的最高目标。

先复现当前缺陷：

glm-5.2 已经找到 Ready 的 PromptedSegmentation Model 和完整
Artifact Conversion Path，但在 discovery_limit_reached 后，
Runtime 丢弃该方案并退回 robocup.ball.vlm-bootstrap。

本次任务不得通过提高预算解决。

必须实现：

1. Builder Session 开始时创建持久化 Working Draft；
2. Discovery 结果持久化为 Plan Candidate；
3. Conversion Path 自动产生可物化 Pipeline Fragment；
4. Registry 能从 Node、Artifact、Model 和 Skill Contract 合成 Pipeline；
5. Workflow Template 只作为 Candidate Seed，不是唯一合法来源；
6. Discovery Limit 转入 DraftSalvage，而不是失败；
7. 达到 Limit 后选择并物化当前最佳 Candidate；
8. 为 Materialization、Validation 和 Finalization 保留预算；
9. FromScratch 不受旧默认 Workflow 和旧 fallback 影响；
10. 历史 Published Version 保持不可变；
11. Ready PromptedSegmentation Model 存在时，不能丢弃合法 SAM 路径；
12. Model 仅为 Fixture 时，不能进入正式 Runnable Draft；
13. UI 必须显示 Candidate、Materialization 和 Salvage 结果；
14. Builder 仍然只能创建 Draft，不能自动 Publish。

先写失败回归测试，再修改实现。

每完成一个 Milestone：
1. 更新状态和验收证据；
2. 执行对应 Rust、Web 和 E2E 测试；
3. 修复回归；
4. 创建独立本地提交；
5. 继续下一 Milestone。

最后对当前 RoboCup Ball Project 创建一个新的 FromScratch Builder
Session，使用当前 Builder LLM 生成 Pipeline，记录完整 Tool Trace、
Candidate、Draft、Validation 和 Sample Test。

不要删除被历史 Run 引用的 Published Workflow。
不要把 EfficientSAM 硬编码进 Core 或 robocup.ball。
不要把 Fixture 模型当成正式模型。
不要 push。
不要修改 Git remote。
不要使用或恢复任何对话中出现过的 API Key。
不要执行 reset、rebase、amend 或破坏性 checkout。

只有 Release Blocking Acceptance Matrix 全部满足，或只剩明确记录的
真实外部模型条件项时，才输出最终报告。
```

