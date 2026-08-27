# AnnotAgent Agent + Skill 架构与 RoboCup Ball Skill Alpha

你现在是 AnnotAgent 仓库的长期主实现工程师。

本任务不是只写设计文档，也不是只增加若干 Skill 卡片或页面。你必须检查现有仓库、修正架构边界、实际实现功能、运行测试、修复错误、创建阶段性本地提交，并持续推进到 Release Blocking 验收完成，或者只剩明确记录的外部服务条件项。

不要因普通技术选择停下来询问我。应当根据现有代码、测试和本文目标自行决策，并记录到决策文档。

开始前阅读课程要求：

* `https://lab.cs.tsinghua.edu.cn/rust/projects/agent/requirements/`
* `https://lab.cs.tsinghua.edu.cn/rust/projects/agent/quick-start/`
* `https://lab.cs.tsinghua.edu.cn/rust/projects/agent/agent-architecture/`

课程要求是硬约束。

---

# 一、任务名称与核心目标

本次长期任务名称：

```text
AnnotAgent Agent + Skill Architecture Alpha
```

本次课程重点应用：

```text
RoboCup Ball Annotation Skill Alpha
```

课程项目推荐标题：

> AnnotAgent：基于可组合 Skills 的 RoboCup 足球视觉标注 Agent

产品名称始终是：

```text
AnnotAgent
```

RoboCup 只能作为：

* 一个 Domain Skill Pack；
* 一个具体应用场景；
* 一个或多个 Project；
* 一组领域知识、Validator、Policy 和 Workflow Template。

不得重新把产品全局名称改成：

```text
RoboCup AnnotAgent
```

---

# 二、先核验现状，不要盲信本文基线

开始时必须：

1. 执行 `git status --short --branch`；
2. 查看最近 15 个提交；
3. 确认当前分支、工作区是否干净、领先远程多少提交；
4. 阅读 README、架构文档、Known Limitations 和执行状态文档；
5. 检查当前 Web 路由、TUI、Runtime、Skill、Workflow、Provider 和 Storage；
6. 检查当前测试数量和失败情况；
7. 不覆盖用户未提交修改；
8. 不假定本文描述的功能都真实存在；
9. 不因现有文档写着“已完成”就跳过代码验证。

禁止：

* `git reset`
* `git rebase`
* `git commit --amend`
* 破坏性 `checkout`
* 修改 Git remote
* push
* 使用、恢复或提交任何曾在对话中出现的 API Key

创建并持续维护：

```text
docs/execution/AGENT_SKILL_MASTER_PLAN.md
docs/execution/AGENT_SKILL_STATUS.md
docs/execution/AGENT_SKILL_DECISIONS.md
docs/execution/AGENT_SKILL_ACCEPTANCE_EVIDENCE.md
docs/execution/AGENT_SKILL_BLOCKERS.md
docs/execution/AGENT_SKILL_KNOWN_LIMITATIONS.md
```

`AGENT_SKILL_STATUS.md` 至少包含：

```text
当前 Milestone
已完成
正在进行
下一步
最近测试
最近提交
真实 Blocker
Release Blocking 剩余项
```

每完成一个 Milestone：

1. 更新状态文档；
2. 更新验收证据；
3. 执行对应测试；
4. 创建独立本地提交；
5. 继续下一个 Milestone；
6. 不等待我确认。

---

# 三、产品与架构的最终层级

必须建立并维护以下层级：

```text
AnnotAgent
│
├── Agent Core
│   ├── Workflow Advisor Agent
│   ├── Annotation Recovery Agent
│   ├── Tool Selection
│   ├── Context Manager
│   ├── Correction Memory
│   ├── Budget / Retry / Stop Control
│   └── Human Approval Boundary
│
├── Workflow Runtime
│   ├── Typed DAG Executor
│   ├── Artifact Store
│   ├── Static Type Checker
│   ├── Cache
│   ├── Replay
│   ├── History
│   ├── Pause / Resume / Cancel
│   └── Versioned Execution
│
├── Core Nodes / Tools
│   ├── Image Input
│   ├── Crop
│   ├── Filter
│   ├── Map Label
│   ├── Attach Attribute
│   ├── Compute Image Statistics
│   ├── Confidence Gate
│   ├── Human Review
│   └── Commit
│
├── Capability Skills
│   ├── Classification Skill
│   ├── VLM Detection Skill
│   ├── YOLO Detection Skill
│   └── Future Prompted Segmentation Skill
│
├── Domain Skills
│   └── RoboCup Skill Pack
│       ├── robocup.ball
│       ├── robocup.robot       [future]
│       └── robocup.field       [future]
│
├── Models
│   ├── OpenAI-compatible VLM
│   ├── HTTP JSON Detector
│   ├── HTTP JSON Classifier
│   ├── Mock Models
│   └── Future ONNX / local workers
│
└── Projects
    ├── B-Human Football Dataset
    ├── Generic Classification Demo
    └── Generic Detection Demo
```

---

# 四、术语必须严格区分

不要把任何小功能都叫 Skill。

## 4.1 Tool

Tool 是最小执行动作，例如：

```text
crop_image
compute_region_statistics
query_correction_memory
commit_artifacts
```

Tool 回答：

> Agent 现在能执行什么动作？

## 4.2 Core Node

Core Node 是领域无关的 Workflow 原语，例如：

```text
Crop
Filter
Confidence Gate
Human Review
Commit
```

它可以调用 Tool，但不属于某个模型或领域。

## 4.3 Model

Model 是具体可调用实现，例如：

```text
qwen3.7-flash
yolo-coco-http
mock-classifier
local-ball-classifier
```

Model 不等于 Skill。

## 4.4 Capability Skill

Capability Skill 封装一种通用模型能力，例如：

```text
Classification Skill
VLM Detection Skill
YOLO Detection Skill
```

Capability Skill 提供：

* Node 定义；
* 输入输出类型；
* 配置 Schema；
* 模型 capability 要求；
* Prompt 或请求协议；
* Mock 实现；
* Workflow Templates；
* 文档。

它不包含 RoboCup 领域知识。

## 4.5 Domain Skill

Domain Skill 封装特定领域知识、策略和恢复逻辑，例如：

```text
robocup.ball
```

Domain Skill 提供：

* 专用 Validator；
* 专用 Recovery Policy；
* 专用 Review Policy；
* Correction taxonomy；
* 领域 Prompt Resource；
* Workflow Templates；
* 领域测试；
* 可选视觉映射。

## 4.6 Skill Pack

Skill Pack 是多个相关 Domain Skill 的聚合：

```text
robocup
├── robocup.ball
├── robocup.robot
└── robocup.field
```

本轮只有 `robocup.ball` 是 Release Blocking 功能。

`robocup.robot` 和 `robocup.field` 只能作为 Roadmap 或 Manifest 占位说明，不得用空实现声称完成。

---

# 五、依赖边界

依赖方向必须是：

```text
annotagent-core
    ↑
capability skills
    ↑
domain skills
    ↑
projects
```

更准确地说：

```text
Core 不依赖任何具体 Skill
Capability Skill 只依赖 Core 和通用 Backend 接口
Domain Skill 依赖 Core 和 Capability 接口
Project 选择并配置 Skills
Binary Composition Root 注册实现
```

禁止：

```rust
if skill_id == "robocup" {
    // special runtime behavior
}
```

禁止：

```rust
match label.as_str() {
    "football" => ...
    "white_shoe" => ...
}
```

禁止 Core、通用 Server、通用 Canvas 或通用 Workflow 编辑器硬编码：

```text
football
ball
white_shoe
white_sock
penalty_mark
field_line
RoboCup
team_color
```

这些词只能存在于：

```text
annotagent-skill-robocup
skills/robocup
examples/robocup
tests/robocup
docs/examples/robocup
```

允许 README 在 RoboCup Example 章节中出现。

增加自动检查：

```bash
rg -n \
  "football|white_shoe|white_sock|penalty_mark|field_line|team_color" \
  crates/annotagent-core \
  crates/annotagent-runtime \
  crates/annotagent-server \
  web/src/components
```

通用代码中不应出现这些领域分支。

---

# 六、AnnotAgent 必须是真正的 Agent

固定执行：

```text
Detector → Crop → Classifier → Gate → Commit
```

只是 Workflow Runtime，不足以证明 AnnotAgent 是 Agent。

AnnotAgent 必须至少真实实现以下两个 Agent Loop。

---

# 七、Agent Loop 1：Workflow Advisor Agent

Workflow Advisor Agent 负责回答：

> 对当前 Project、Label、可用 Skills、可用 Models 和约束，应该如何构造标注流程？

## 7.1 输入

```rust
pub struct WorkflowAdvisorRequest {
    pub project_schema: ProjectSchemaSnapshot,
    pub target_labels: Vec<LabelId>,
    pub enabled_skills: Vec<SkillRef>,
    pub node_catalog: NodeCatalogSnapshot,
    pub model_registry: ModelRegistrySnapshot,
    pub sample_images: Vec<ImageId>,
    pub constraints: WorkflowConstraints,
}
```

约束至少支持：

```text
cost priority
latency priority
accuracy priority
maximum model calls
maximum cost per image
human review target
available hardware
offline-only
```

## 7.2 可用工具

Advisor Agent 至少可以调用：

```text
inspect_project_schema
inspect_label
sample_dataset
inspect_sample_image
list_enabled_skills
load_skill_resource
list_available_nodes
list_models
inspect_model_capabilities
inspect_existing_workflows
create_workflow_draft
validate_workflow_draft
dry_run_workflow
inspect_dry_run_summary
inspect_failed_samples
inspect_artifacts
estimate_workflow_cost
revise_workflow_draft
request_human_publish_approval
```

## 7.3 Agent Loop

必须真实实现：

```text
读取目标
→ 检查 Project Schema
→ 检查可用 Skills
→ 检查可用 Models
→ 加载与目标 Label 相关的 Skill Resource
→ 生成 Workflow Draft
→ 调用 Rust Static Validator
→ 根据错误修复 Draft
→ Dry Run 1～10 张图片
→ 查看结果、失败、Review rate、成本和中间 Artifact
→ 必要时修改 Draft
→ 达到目标或预算边界
→ 提交给人工审批
```

不得实现成一次 LLM 请求返回 JSON 后直接保存。

## 7.4 人工边界

Advisor Agent 不得：

* 自动发布 Workflow；
* 自动启动正式 Batch Run；
* 注册未知 Tool；
* 生成并执行任意 Shell；
* 生成并执行任意 Python；
* 引用不存在的 Node；
* 引用不存在的 Model；
* 引用未启用 Skill；
* 自动修改 API Key；
* 自动覆盖 Published Version。

Agent 最终只能返回：

```rust
pub enum AdvisorOutcome {
    DraftReadyForReview,
    NeedsHumanInput,
    BudgetExceeded,
    NoValidWorkflow,
    Cancelled,
    Failed,
}
```

用户人工确认后才能 Publish。

## 7.5 停止条件

至少包括：

* Draft 静态验证通过且 Dry Run 达标；
* 达到最大 Advisor turn；
* 达到最大 Tool Call；
* 达到 Token 预算；
* 达到费用预算；
* 用户取消；
* 连续产生同一个无效 Draft；
* 没有满足 capability 的 Model；
* 需要人类提供不可推断配置。

---

# 八、Agent Loop 2：Annotation Recovery Agent

正常样本优先执行确定性 Published Workflow。

只有候选出现风险、冲突或 Validator Issue 时，进入 Recovery Agent。

## 8.1 输入

```rust
pub struct RecoveryRequest {
    pub project_id: ProjectId,
    pub run_id: RunId,
    pub image_id: ImageId,
    pub task_id: TaskId,
    pub candidate_artifact_ids: Vec<ArtifactId>,
    pub validation_issues: Vec<ValidationIssue>,
    pub enabled_skills: Vec<SkillRef>,
    pub available_tools: Vec<ToolDefinition>,
    pub remaining_budget: BudgetSnapshot,
}
```

## 8.2 可用动作

Recovery Agent 可以：

```text
inspect_candidate
inspect_parent_artifacts
inspect_validation_issue
load_domain_skill_resource
crop_candidate
compute_region_statistics
run_classifier
run_detector
query_correction_memory
compare_evidence
request_refinement
accept_artifact
reject_artifact
request_human_review
finish_recovery
```

## 8.3 Agent Loop

```text
候选进入 Validator
→ 产生结构化风险
→ Recovery Agent 加载相关 Domain Skill
→ 查看候选和父 Artifact
→ 查询 Correction Memory
→ 选择补充工具
→ 获取新证据
→ 对比证据
→ 接受、拒绝、再次验证或人工复核
→ 达到停止条件
```

## 8.4 不是每个 bbox 都调用 Agent

正常高置信度候选应走快速路径：

```text
Candidate
→ Validator pass
→ Gate
→ Commit
```

只有：

* hard negative 风险；
* 模型冲突；
* 低置信度；
* 几何异常；
* 历史高频错误；
* Workflow 明确要求；

才进入 Recovery Agent。

这样避免把每个矩形都升级成一次外交危机。

---

# 九、Skill Manifest 和 Registry

实现版本化 Skill Manifest。

建议结构：

```yaml
version: 1

id: robocup.ball
display_name: RoboCup Ball Annotation
kind: domain
skill_version: "1.0.0"

requires:
  capabilities:
    - object_detection
    - crop
    - classification
    - human_review

optional_capabilities:
  - robot_detection
  - field_segmentation
  - image_statistics

provides:
  validators:
    - robocup.ball_hard_negative
    - robocup.ball_field_relation

  policies:
    - robocup.ball_recovery
    - robocup.ball_review

  memories:
    - robocup.ball_correction_memory

  workflow_templates:
    - robocup.ball.vlm_detect_verify
    - robocup.ball.detector_crop_verify

  resources:
    - SKILL.md
    - tasks/football-detection.md
    - recovery/hard-negatives.md
    - prompts/football-verification.md
```

Core 定义：

```rust
pub enum SkillKind {
    Capability,
    Domain,
    Pack,
}
```

```rust
pub trait Skill: Send + Sync {
    fn manifest(&self) -> &SkillManifest;

    fn node_definitions(&self) -> Vec<NodeDefinition>;

    fn tool_definitions(&self) -> Vec<ToolDefinition>;

    fn validators(&self) -> Vec<Arc<dyn AnnotationValidator>>;

    fn policies(&self) -> Vec<Arc<dyn AgentPolicy>>;

    fn workflow_templates(&self) -> Vec<WorkflowTemplate>;

    fn resources(
        &self,
        request: SkillResourceRequest,
    ) -> Result<Vec<SkillResource>, SkillError>;

    fn correction_taxonomy(&self) -> Vec<CorrectionKind>;
}
```

通过 Registry 注册：

```rust
skill_registry.register(Arc::new(ClassificationSkill::new(...)))?;
skill_registry.register(Arc::new(VlmDetectionSkill::new(...)))?;
skill_registry.register(Arc::new(YoloDetectionSkill::new(...)))?;
skill_registry.register(Arc::new(RoboCupBallSkill::new(...)))?;
```

禁止 Runtime 根据 Skill ID 分支。

---

# 十、Capability Skill 1：Classification Skill

Classification Skill 是通用能力。

必须支持：

```text
whole-image classification
crop classification
candidate verification
attribute classification
```

统一输入：

```rust
pub enum ClassificationSubject {
    Image(ImageArtifactRef),
    Crop(CropArtifactRef),
    Candidate(ArtifactRef),
}
```

统一输出：

```rust
pub struct ClassificationSetArtifact {
    pub results: Vec<ClassificationResult>,
}
```

```rust
pub struct ClassificationResult {
    pub subject_artifact_id: ArtifactId,
    pub scores: Vec<ClassScore>,
    pub selected_labels: Vec<LabelId>,
    pub confidence: f32,
    pub provenance: ArtifactProvenance,
}
```

必须保留 `subject_artifact_id`，确保 Crop 分类结果正确挂回原 Detection。

支持 Model Backend：

```text
mock
openai_compatible_vlm
http_json_classifier
```

本轮最低完成：

* Mock 分类；
* OpenAI-compatible VLM 分类；
* HTTP JSON Classifier 协议；
* single-label；
* multi-label；
* verifier 二分类模板；
* 配置 Schema；
* 单元测试；
* 离线 Demo。

---

# 十一、Capability Skill 2：VLM Detection Skill

VLM Detection Skill 是通用能力，不得包含 RoboCup Prompt。

输入：

```text
Image
Label Schema
Optional prompt resource
Maximum objects
```

输出：

```rust
pub struct DetectionSetArtifact {
    pub detections: Vec<DetectionArtifactItem>,
}
```

```rust
pub struct DetectionArtifactItem {
    pub id: DetectionId,
    pub model_label: String,
    pub project_label: Option<LabelId>,
    pub bbox: NormalizedRect,
    pub confidence: f32,
    pub evidence: Option<String>,
}
```

必须：

* 使用结构化输出或 Tool Call；
* 校验坐标；
* 拒绝 NaN、Infinity、负宽高、越界；
* 支持合法空 DetectionSet；
* 支持 Tool Call 完整历史；
* 保存 Provider usage；
* 不要求 VLM 重新抄写确定性 Tool 输出。

---

# 十二、Capability Skill 3：YOLO Detection Skill

YOLO Detection Skill 也是通用能力。

本轮不要求 Rust 进程直接加载所有 YOLO 权重。

最低实现：

```text
Mock YOLO Backend
HTTP JSON Detection Backend
class mapping
confidence threshold
NMS parameters
DetectionSet Artifact
Workflow Template
```

输出只允许是 Detection Artifact，不得直接提交最终 Annotation。

模型类别与 Project Label 必须分离：

```yaml
class_mapping:
  sports ball: football
  person: person
```

YOLO Skill 不拥有 Crop。

用户界面可以提供模板：

```text
YOLO Detect & Crop
```

但内部必须是：

```text
YOLO Detection Node
→ Core Filter Node
→ Core Crop Node
```

不得实现成一个不可检查的巨大黑箱 Skill。

真实 YOLO Worker 作为 `live-conditional`：

* 有可用 Worker 或权重时执行 5 张图 smoke test；
* 没有时 Mock 和协议测试仍必须完整通过；
* 不得因此阻塞其他 Milestone。

---

# 十三、Core Nodes

以下必须属于 Core，而不是 Skill：

```text
ImageInput
Crop
Filter
MapLabel
AttachAttribute
ComputeImageStatistics
ConfidenceGate
HumanReview
Commit
```

## 13.1 Crop

输入：

```text
ImageArtifact
DetectionSet 或 BoundingBox Artifact
```

输出：

```rust
pub struct CropSetArtifact {
    pub crops: Vec<CropArtifactItem>,
}
```

每个 Crop 必须包含：

```rust
pub struct CropArtifactItem {
    pub crop_id: CropId,
    pub image_id: ImageId,
    pub parent_artifact_id: ArtifactId,
    pub parent_detection_id: Option<DetectionId>,
    pub source_region: NormalizedRect,
    pub padding_ratio: f32,
    pub width: u32,
    pub height: u32,
    pub cache_key: String,
}
```

## 13.2 Filter

支持：

* project label；
* model label；
* confidence；
* geometry；
* attribute；
* validator issue。

## 13.3 Confidence Gate

输出明确分支：

```text
pass
review
reject
```

## 13.4 Commit

只能提交已通过强制 Runtime 校验的 Artifact。

模型或 Skill 不得绕过 Validator 直接写最终 Annotation。

---

# 十四、强类型 Artifact 和 Lineage

每个节点必须通过 Artifact 传递数据。

最低支持：

```rust
pub enum ArtifactPayload {
    Image(ImageArtifact),
    DetectionSet(DetectionSetArtifact),
    CropSet(CropSetArtifact),
    ClassificationSet(ClassificationSetArtifact),
    AnnotationCandidateSet(AnnotationCandidateSetArtifact),
    ValidationResult(ValidationResultArtifact),
    ReviewDecision(ReviewDecisionArtifact),
}
```

Artifact 至少包含：

```rust
pub struct Artifact {
    pub id: ArtifactId,
    pub project_id: ProjectId,
    pub run_id: RunId,
    pub image_id: Option<ImageId>,
    pub node_id: NodeId,
    pub payload: ArtifactPayload,
    pub parent_artifact_ids: Vec<ArtifactId>,
    pub provenance: ArtifactProvenance,
    pub created_at: DateTime<Utc>,
    pub cache_key: Option<String>,
}
```

要求：

* 完整持久化；
* 父子引用；
* 来源 Node；
* Model Version；
* Skill Version；
* Workflow Version；
* Prompt Version；
* Cost；
* Duration；
* Replay；
* Cache；
* GUI Inspector。

确定性 Tool 产生精确几何后：

* 模型通过 Artifact ID 接受或拒绝；
* 不要求模型重新输出坐标；
* 不允许精确结果在“让 VLM 抄一遍”时漂移。

---

# 十五、Tool Call 协议必须正确

OpenAI-compatible Provider 必须遵守完整 Tool Call 历史：

```text
assistant message with tool_calls
→ one tool message per tool_call_id
→ next assistant request
```

必须：

* 持久化完整 assistant tool-call 消息；
* 保留 `tool_call_id`；
* 多工具调用顺序正确；
* 每个 tool call 恰好一个 tool result；
* Provider 请求前检查历史是否合法；
* Tool Result 分成：

  * persisted full result；
  * model-facing structured result；
  * UI-facing summary。

不得只把一句：

```text
tool executed successfully
```

发回模型。

必须增加协议测试：

1. 单 Tool Call；
2. 多 Tool Call；
3. 缺失 Tool Result；
4. 重复 Tool Result；
5. 错误 Tool ID；
6. 几何 Artifact 引用；
7. Tool 超时；
8. Provider 取消。

---

# 十六、RoboCup Skill Pack

创建：

```text
annotagent-skill-robocup
skills/robocup/
```

结构建议：

```text
skills/robocup/
├── manifest.yaml
├── SKILL.md
├── ball/
│   ├── manifest.yaml
│   ├── SKILL.md
│   ├── hard-negatives.md
│   ├── recovery-policy.md
│   ├── prompts/
│   └── templates/
├── robot/
│   └── README-roadmap.md
└── field/
    └── README-roadmap.md
```

本轮完整实现：

```text
robocup.ball
```

未来扩展：

```text
robocup.robot
robocup.field
```

不得为未来子 Skill 添加空 Rust trait 实现并声称支持。

---

# 十七、robocup.ball 的领域知识

`robocup.ball` 至少处理以下错误模式：

```text
white_shoe_as_ball
white_sock_as_ball
penalty_mark_as_ball
field_line_intersection_as_ball
missed_small_ball
duplicate_ball
inaccurate_ball_bbox
ball_outside_field
```

必须包含领域说明：

* RoboCup 足球通常是小目标；
* 白色外观容易与鞋、袜、点球点和线交点混淆；
* 机器人脚部附近候选风险较高；
* 场地区域关系可以提供辅助证据；
* 候选过小或过大都可能异常；
* 小目标低分不一定应直接删除；
* 高置信度也不能覆盖明确 hard-negative 证据。

这些知识按需加载，不要永久塞入所有模型请求。

---

# 十八、定制 1：RoboCup Ball Hard-negative Validator

实现：

```rust
pub struct RoboCupBallHardNegativeValidator;
```

风险特征至少包括：

* 候选 bbox 与 robot bbox 重叠；
* 候选是否位于 robot bbox 下部；
* 候选与 field region 的关系；
* 候选与 penalty mark 的距离；
* 候选与 field line 或 line intersection 的关系；
* bbox 长宽比；
* bbox 相对图片面积；
* 白色像素比例；
* crop 分类结果；
  -模型间冲突；
* Correction Memory 中的同类错误频次。

输出：

```rust
pub enum RoboCupBallIssueCode {
    PossibleWhiteShoe,
    PossibleWhiteSock,
    PossiblePenaltyMark,
    PossibleFieldLineIntersection,
    OutsideField,
    SuspiciousScale,
    ConflictingEvidence,
}
```

若缺少 robot detection 或 field region：

* 降级；
* 输出 evidence unavailable；
* 不得 panic；
* 不得把缺失证据伪装为通过。

---

# 十九、定制 2：RoboCup Ball Correction Memory

实现项目级 Correction Memory。

记录：

```rust
pub struct CorrectionRecord {
    pub id: CorrectionId,
    pub project_id: ProjectId,
    pub skill_id: SkillId,
    pub task_id: TaskId,
    pub image_id: ImageId,
    pub original_artifact_ids: Vec<ArtifactId>,
    pub corrected_annotation: Option<AnnotationSnapshot>,
    pub reason_code: String,
    pub note: Option<String>,
    pub geometric_features: GeometricFeatures,
    pub visual_features: VisualFeatures,
    pub related_annotation_ids: Vec<AnnotationId>,
    pub created_at: DateTime<Utc>,
}
```

人工操作后可记录：

```text
white_shoe_as_ball
white_sock_as_ball
penalty_mark_as_ball
field_line_intersection_as_ball
missed_small_ball
other
```

检索基于：

* project；
* skill；
* task；
* label；
* reason；
* recency；
* frequency；
* 几何摘要；
* 关联 robot；
* 区域关系。

第一阶段不需要向量数据库。

Memory 必须真实影响后续行为：

* 提高某类候选进入 Crop Verify 的概率；
* 调整 Review Gate；
* 增加 Advisor 建议；
* 在 Trace 中解释策略变化；
* 不允许只有数据库记录而没有决策影响。

必须有测试：

```text
第一次运行：
白鞋误检
→ 人工拒绝
→ 写入 correction memory

第二次相似运行：
加载 memory
→ 风险提高
→ 自动触发 crop verification
→ 不再直接 auto-accept
```

---

# 二十、定制 3：Field-region Constraint

实现：

```rust
pub struct RoboCupBallFieldRelationValidator;
```

使用可选 field region Artifact：

* 候选中心是否位于 field region；
* bbox 主要面积是否在 field region；
* 距离边界是否合理；
* 场外候选是否应 Review 或 Reject。

若没有 field region：

```text
severity = warning
code = field_evidence_unavailable
```

此定制可以复用用户已有 boundary segmentation，但 Core 不得依赖具体分割实现。

Field region 可以来自：

* 导入标注；
* 语义分割 Skill；
* 人工 polygon；
* Mock Artifact。

---

# 二十一、RoboCup Ball Recovery Policy

实现：

```rust
pub struct RoboCupBallRecoveryPolicy;
```

输入：

* Validator issues；
* Correction Memory；
  -剩余预算；
  -可用 Capability；
  -模型绑定；
  -当前 Artifact lineage。

输出受控动作：

```rust
pub enum RecoveryAction {
    Accept,
    Reject,
    RunCropClassifier,
    RequestAdditionalDetection,
    ComputeImageStatistics,
    RequestHumanReview,
    Stop,
}
```

示例路径：

## 普通候选

```text
高置信度
+ 无领域风险
→ Accept
```

## 白鞋风险

```text
靠近机器人下部
+ 白色比例高
+ memory 中 white_shoe_as_ball 高频
→ Crop
→ Classification
→ Reject 或 Review
```

## 点球点风险

```text
接近 field line
+ 圆形白色区域
+ 无运动或球纹理证据
→ 不允许直接自动接受
→ Crop Verify 或 Review
```

## 预算不足

```text
风险存在
+ 无足够预算调用补充模型
→ Human Review
```

必须在 Trace 中保存：

```text
skill loaded
validator issue
memory evidence
available actions
selected action
tool result
final decision
```

不得保存或展示隐藏思维链。

---

# 二十二、Capability 绑定，不要写死模型

`robocup.ball` 依赖的是能力：

```text
object_detection
crop
classification
human_review
```

不得写死：

```text
qwen3.7-flash
yolo11
```

Project 负责绑定：

```yaml
model_bindings:
  object_detection:
    model: qwen3.7-flash

  crop_classification:
    model: qwen3.7-flash
```

或者：

```yaml
model_bindings:
  object_detection:
    model: robocup-yolo-http

  crop_classification:
    model: local-ball-classifier
```

Skill 领域逻辑不因模型更换而重写。

---

# 二十三、RoboCup Ball Workflow Templates

至少提供两个模板。

## 23.1 VLM Detect and Verify

```text
Image
→ VLM Detection
→ Filter football
→ RoboCup Hard-negative Validator
    ├── low risk → Gate → Commit
    └── high risk → Crop
                    → Classification
                    → Recovery Policy
                    → Commit / Reject / Review
```

## 23.2 Detector Crop Verify

```text
Image
→ Detection Capability
→ Filter football
→ Crop
→ Classification
→ RoboCup Hard-negative Validator
→ Correction Memory Policy
→ Gate
→ Commit / Review
```

模板只引用 capability 和 Core Node，不绑定具体模型。

---

# 二十四、Project 配置示例

创建：

```text
examples/robocup-ball/
```

示例 Project：

```yaml
version: 1

project:
  name: B-Human Football Dataset

skills:
  - annotagent.vlm_detection
  - annotagent.classification
  - robocup.ball

schema:
  tasks:
    - id: footballs
      kind: bounding_box
      labels:
        - id: football
          display_name: Football

model_bindings:
  object_detection:
    provider: openai_compatible
    model: qwen3.7-flash

  crop_classification:
    provider: openai_compatible
    model: qwen3.7-flash

workflow:
  template: robocup.ball.vlm_detect_verify
```

再提供离线 Mock Project：

```text
examples/robocup-ball-mock/
```

它必须无需 API Key 完成完整 Agent Loop。

---

# 二十五、Generic 项目必须继续工作

至少保留三个领域无关示例：

```text
1. Generic Whole-image Classification
2. Generic Detection
3. Generic Detection + Crop Classification
```

验收：

* 不启用 RoboCup Skill；
* 页面不出现 RoboCup 文案；
* Runtime 不加载 RoboCup Resource；
* Trace 不出现 RoboCup Validator；
* 能完成 Dry Run、Publish 和 Run；
* 导出正常。

这用于证明可扩展 Core 不是只在架构图里存在。

---

# 二十六、多个 Skill 的组合

Project 数据模型必须支持：

```yaml
skills:
  - annotagent.vlm_detection
  - annotagent.classification
  - robocup.ball
```

Skill Registry 必须：

* 检查重复 ID；
* 检查版本；
* 检查 capability 依赖；
* 检查冲突；
* 合并 Node Catalog；
* 合并 Tool Catalog；
* 合并 Validator；
* 合并 Workflow Template；
* 按需加载 Resource。

禁止用：

```rust
project.skill: String
```

作为唯一 Skill 表达。

迁移为：

```rust
pub struct EnabledSkill {
    pub id: SkillId,
    pub version_requirement: VersionReq,
    pub config: SkillConfig,
}
```

已有单 Skill 项目必须可以迁移。

---

# 二十七、Skill Resource 按需加载

初始 Agent 上下文只包含：

* AnnotAgent 系统规则；
* Project 摘要；
  -已启用 Skill 的一行摘要；
  -当前目标；
  -可用 Tool。

只有处理特定任务时，才加载：

```text
robocup.ball/SKILL.md
robocup.ball/hard-negatives.md
robocup.ball/recovery-policy.md
```

不得把全部 Skill 文档放入每次请求。

实现：

```text
load_skill_resource(skill_id, resource_id)
```

Tool 必须检查：

* Skill 已启用；
* Resource 存在；
* Project 有权限；
* 文件位于受控 Skill Root；
* 不允许路径穿越。

---

# 二十八、工作流与 Agent 的关系

必须明确区分：

## Workflow Runtime

负责稳定执行已发布流程：

```text
节点
→ Artifact
→ Gate
→ Commit
```

## Agent

负责：

```text
设计 Workflow
验证 Workflow
Dry Run 后修订 Workflow
异常时选择恢复动作
利用 Memory 改变策略
请求人工审批
```

不要把所有节点都变成 LLM Agent。

不要让正常标注流程每一步都由 LLM 自由规划。

正确设计：

```text
Deterministic Workflow for normal path
+
Agent Loop for planning and uncertain recovery
```

---

# 二十九、Run、Task 和 Agent 状态

状态必须分层。

## Workflow

```rust
pub enum WorkflowStatus {
    Draft,
    Invalid,
    Valid,
    Tested,
    Published,
    Archived,
}
```

## Run

```rust
pub enum RunStatus {
    Pending,
    Running,
    Paused,
    AwaitingReview,
    Completed,
    CompletedWithReview,
    Partial,
    Failed,
    Cancelled,
    Interrupted,
    BudgetExceeded,
}
```

## Task

```rust
pub enum TaskRunStatus {
    Pending,
    Running,
    Succeeded,
    SucceededEmpty,
    NeedsReview,
    Skipped,
    Failed,
    Cancelled,
}
```

## Agent Session

```rust
pub enum AgentSessionStatus {
    Planning,
    ExecutingTool,
    WaitingForModel,
    WaitingForHuman,
    Completed,
    Cancelled,
    BudgetExceeded,
    Failed,
}
```

合法空结果必须是：

```text
SucceededEmpty
```

不是失败。

---

# 三十、预算、费用和停止控制

Advisor Agent 和 Recovery Agent 分别配置：

```toml
[advisor]
max_model_turns = 8
max_tool_calls = 20
max_dry_runs = 3
max_cost = "1.0"

[recovery]
max_model_turns = 4
max_tool_calls = 8
max_cost_per_candidate = "0.05"
```

必须记录：

* input token；
* output token；
* image token；
* cached token；
* request count；
  -费用；
  -模型；
* Provider；
* Skill；
* Node；
* Agent Session。

达到预算后：

* 不开始新模型调用；
* 高风险候选进入 Human Review；
* 不得把预算不足误报为模型失败；
* GUI 和 TUI 显示具体预算触发原因。

---

# 三十一、Correction Memory 的安全边界

Memory 只能保存受控结构化内容和人工 note。

不得：

* 将图片内文字当成系统指令；
* 将人工 note 直接拼接为最高优先级 Prompt；
* 将模型生成的自由文本永久视为事实；
* 跨 Project 泄露纠错记录；
* 自动执行 Memory 中出现的命令。

Prompt 构建时将 Memory 表示为：

```text
Project-specific historical correction evidence
```

不是系统指令。

---

# 三十二、Web 产品要求

全局产品仍然是 AnnotAgent。

RoboCup 只在：

* Project 已启用 `robocup.ball`；
* Skill 详情；
* RoboCup 示例项目；
* Run Trace 中加载该 Skill；
* Review correction reason；

时出现。

## 32.1 Settings / Capabilities

Skill 页面分组：

```text
Capability Skills
- Classification
- VLM Detection
- YOLO Detection

Domain Skills
- RoboCup Ball Annotation
```

每个 Skill 显示：

* 类型；
  -版本；
  -提供的 Nodes；
* Validators；
* Policies；
* Templates；
* Capability Requirements；
* 使用中的 Projects。

## 32.2 Project Build

Project Build 中用户：

1. 定义 Label；
2. 启用 Capability Skills；
3. 启用 Domain Skills；
4. 配置 Model Bindings；
5. 让 Advisor 建议 Draft；
6. 人工编辑；
7. Dry Run；
8. Publish。

Advisor 页面必须显示：

```text
Loaded Skills
Available Capabilities
Selected Models
Suggested Pipeline
Validation Issues
Dry Run Results
Agent Actions
Cost
```

不得展示隐藏思维链。

## 32.3 Run Detail

Run Trace 明确区分：

```text
Workflow Node
Agent Recovery Step
Skill Resource Load
Validator
Memory Query
Human Review
Commit
```

## 32.4 Review

RoboCup Ball Review 提供纠错原因：

```text
White shoe
White sock
Penalty mark
Field-line intersection
Missed small ball
Wrong box
Other
```

人工修正后：

* 写 Annotation Revision；
* 可写 Correction Memory；
* 显示该 Memory 将影响什么策略。

---

# 三十三、TUI 要求

TUI 主标题：

```text
AnnotAgent
Composable Annotation Agent Runtime
```

打开项目后：

```text
Project: B-Human Football Dataset
Skills: vlm-detection, classification, robocup.ball
Workflow: robocup-ball-vlm-verify@v1
```

TUI 至少支持：

```text
/skills
/skills show robocup.ball
/advisor
/advisor cancel
/run
/pause
/resume
/cancel
/memory
/history
/gui
```

Trace 能查看：

* Skill 加载；
* Agent Tool Call；
* Validator；
* Memory；
* Decision；
* Token；
  -费用；
  -停止原因。

---

# 三十四、课程要求映射

在：

```text
docs/COURSE_REQUIREMENTS.md
```

明确映射。

## R1：Rust 核心

Rust 实现：

* Agent Loop；
* Tool 调度；
* Skill Registry；
* Workflow Runtime；
* State Machine；
* Validator；
* Correction Memory；
* Budget；
* Storage；
* HTTP Server。

## R2：交互界面

* Ratatui TUI；
* Web GUI。

## R3：模型配置

* Provider；
* endpoint；
* API Key；
* Model；
* context；
* reasoning；
* pricing；
* capability binding。

## R4：实时进度和打断

* Event Bus；
* SSE/WebSocket；
* Advisor 和 Run 进度；
* Pause；
* Resume；
* Cancel。

## R5：历史

* Agent Session；
* Tool Calls；
* Skill Resources；
* Artifacts；
* Run；
* Revisions；
* Memory；
* import/export。

## R6：Token 和费用

* 每次模型调用；
* Advisor；
* Recovery；
* Node；
* Project；
* Run；
  -预算停止。

## 场景定制

至少明确三项：

1. RoboCup Ball Hard-negative Validator；
2. RoboCup Ball Correction Memory；
3. Field-region Constraint；
4. RoboCup Ball Recovery Policy。

---

# 三十五、与通用工作流平台的区别

在 README 和课程文档中诚实描述：

AnnotAgent 不是依靠“能拼接模型”形成区别。

核心区别是：

1. AnnotAgent 本身执行 Agent Loop；
2. Skill 分为 Capability 与 Domain；
3. Domain Skill 能影响 Validator、Recovery 和 Memory；
4. 每个结果有 Artifact lineage；
5. Agent 能根据 Dry Run 和历史纠错修改策略；
6. Published Workflow 是不可变执行合约；
7. RoboCup Ball Skill 固化具体错误模式和恢复策略。

不要声称其他平台不能组合模型。

不要把课程展示变成竞品攻击。

---

# 三十六、Milestone 计划

## Milestone 0：基线、状态文件与边界测试

完成：

* 仓库核验；
* 状态文档；
* 当前架构图；
* Core 禁止领域词测试；
* 当前 Agent Loop 真实性评估；
* 当前 Skill 数据模型评估；
* 当前 Tool Call 协议测试基线。

提交建议：

```text
docs: establish agent-skill alpha baseline
```

## Milestone 1：Skill 类型、Registry 与多 Skill Project

完成：

* SkillKind；
* Skill Manifest；
* Skill Registry；
* EnabledSkill；
* Capability requirements；
  -多 Skill Project；
* migration；
* Dummy Capability Skill；
* Dummy Domain Skill；
  -依赖和冲突校验。

验收：

* Generic Project 加载两个 Capability Skills；
* RoboCup Project 加载 Capability + Domain Skill；
* Core 不修改即可注册 Dummy Domain Skill。

提交建议：

```text
feat(core): introduce layered capability and domain skills
```

## Milestone 2：Tool Call、Artifact 与 Lineage

完成：

* 标准 Tool Call 历史；
* Vision Artifact；
* Artifact persistence；
* parent linkage；
  -完整 Tool Result；
* Cache key；
* Replay 基础；
  -协议测试。

提交建议：

```text
feat(runtime): add typed artifacts and correct tool-call history
```

## Milestone 3：Classification Skill

完成：

* whole-image；
* crop；
* verifier；
* Mock；
* OpenAI-compatible；
* HTTP JSON；
  -配置 Schema；
  -模板；
  -测试；
* Generic Demo。

提交建议：

```text
feat(skills): add reusable classification capability
```

## Milestone 4：Detection Skills 与 Core Crop

完成：

* VLM Detection Skill；
* YOLO Detection Skill；
* Mock Detector；
* HTTP JSON Detector；
* class mapping；
* Core Crop；
* parent link；
* shared execution；
* Detection + Crop Demo。

提交建议：

```text
feat(skills): add detection capabilities and composable crop flow
```

## Milestone 5：Workflow Advisor Agent

完成：

* Advisor Session；
* Tool Catalog；
* Context；
* Draft；
* Static Validation；
* Dry Run；
* Revision；
* Human approval；
  -预算；
  -停止条件；
* Agent Trace；
  -端到端 Mock 测试。

关键测试：

```text
第一次 Draft 类型错误
→ Rust Validator 返回错误
→ Advisor 修改 Draft
→ Dry Run 发现 review rate 过高
→ Advisor 调整 Pipeline
→ Draft Ready for Human Review
→ 不自动 Publish
```

提交建议：

```text
feat(agent): implement iterative workflow advisor
```

## Milestone 6：RoboCup Ball Skill

完成：

* manifest；
* resources；
* hard-negative validator；
* field relation；
* recovery policy；
* templates；
* review reasons；
* Mock 图片和脚本；
  -普通球；
  -白鞋；
  -点球点；
  -线交点案例。

提交建议：

```text
feat(robocup): add ball annotation domain skill
```

## Milestone 7：Correction Memory 与自适应 Recovery

完成：

* CorrectionRecord；
  -检索；
  -阈值影响；
* Recovery Agent；
* Memory Trace；
  -人工修正；
  -第二次运行策略变化；
  -端到端测试。

提交建议：

```text
feat(agent): adapt robocup recovery from correction memory
```

## Milestone 8：Web、TUI 和 Guided UX

完成：

* Skill 分层展示；
* Project Skill 配置；
* Advisor；
* Agent Trace；
* Recovery Trace；
* Memory；
* Review reason；
* TUI；
* URL 状态恢复；
* active run；
  -取消；
  -无障碍。

不得破坏 AnnotAgent 主品牌。

提交建议：

```text
feat(ui): integrate advisor and domain-skill recovery workflows
```

## Milestone 9：批处理、可靠性和课程演示

完成：

* 100 张合成图片；
* Pause；
* Resume；
* Cancel；
  -服务重启 reconciliation；
  -重复启动阻止；
  -预算；
  -历史；
  -导出；
  -课程 5 分钟演示；
  -文档；
* Release Matrix。

提交建议：

```text
test(release): validate annotagent agent and robocup skill alpha
```

---

# 三十七、Release Blocking Acceptance Matrix

以下全部满足后才能声称 Alpha 完成。

## A. AnnotAgent Agent 性

* [ ] Workflow Advisor 至少有一次真实多轮 Tool Loop。
* [ ] Advisor 会根据 Static Validator 错误修改 Draft。
* [ ] Advisor 会根据 Dry Run 结果修改 Draft。
* [ ] Advisor 不会自动 Publish。
* [ ] Recovery Agent 会根据 Validator Issue 选择补充 Tool。
* [ ] Recovery Agent 会根据 Tool Result 改变下一步。
* [ ] Agent 有明确停止条件。
* [ ] Agent 支持取消、Token 和费用预算。
* [ ] Trace 不展示隐藏思维链。

## B. Skill 架构

* [ ] Tool、Core Node、Model、Capability Skill、Domain Skill 明确区分。
* [ ] Core 不依赖 RoboCup。
* [ ] Project 支持多个 Enabled Skills。
* [ ] Capability 依赖可以校验。
* [ ] Dummy Domain Skill 无需修改 Core 即可运行。
* [ ] 不存在 `if skill_id == "robocup"`。
* [ ] Skill Resource 按需加载。
* [ ] Skill 有版本号。
* [ ] Run 固定 Skill Version。

## C. 通用能力

* [ ] Classification Skill 支持整图分类。
* [ ] Classification Skill 支持 Crop 分类。
* [ ] VLM Detection 输出 DetectionSet。
* [ ] YOLO Detection Skill 支持 Mock。
* [ ] YOLO Detection Skill 支持 HTTP JSON 协议。
* [ ] Crop 是 Core Node。
* [ ] Crop 保留 parent Artifact。
* [ ] 相同 shared detector 每图只执行一次。
* [ ] Generic Project 不加载 RoboCup。

## D. RoboCup Ball 定制

* [ ] `robocup.ball` 通过 Skill Registry 注册。
* [ ] 普通足球候选走快速路径。
* [ ] 白鞋候选触发 hard-negative 风险。
* [ ] 白袜候选触发 hard-negative 风险。
* [ ] 点球点候选不得直接自动接受。
* [ ] 场线交点候选不得直接自动接受。
* [ ] Field region 可作为领域证据。
* [ ] 缺少 field region 时安全降级。
* [ ] Recovery Policy 根据证据选择 Crop Verify 或 Review。
* [ ] Trace 显示领域 Validator 和 Policy。

## E. Correction Memory

* [ ] 人工修正写入 Memory。
* [ ] Memory 按 Project 隔离。
* [ ] 第二次相似候选的决策真实改变。
* [ ] GUI 显示 Memory 影响原因。
* [ ] Memory 不被当成系统指令。
* [ ] Memory 不跨 Project 泄露。

## F. Artifact 与正确性

* [ ] Tool Call 历史协议正确。
* [ ] Tool 精确几何不由 VLM 重新抄写。
* [ ] Artifact 有父子引用。
* [ ] Artifact 有 Model、Skill 和 Workflow provenance。
* [ ] Node Inspector 可查看输入输出。
* [ ] Replay 可复用上游 Artifact。
* [ ] 合法空结果为 `SucceededEmpty`。
* [ ] optional task 失败不拖死 required task。
* [ ] 无重复 Annotation commit。

## G. 产品

* [ ] 全局品牌是 AnnotAgent。
* [ ] 空工作区不出现 RoboCup。
* [ ] Generic Project 不出现 RoboCup。
* [ ] RoboCup 只在已启用 Skill 的 Project 中出现。
* [ ] Skill 页面区分 Capability 与 Domain。
* [ ] Advisor 显示加载了哪些 Skills。
* [ ] Review 有 RoboCup correction reasons。
* [ ] TUI 和 GUI 可查看 Agent Trace。
* [ ] TUI 和 GUI 可取消 Agent。

## H. 作业要求

* [ ] Rust 核心逻辑真实存在。
* [ ] TUI 可用。
* [ ] GUI 可用。
* [ ] Provider、Model、Context、Reasoning 和价格可配置。
* [ ] 实时进度可见。
* [ ] Pause、Resume、Cancel 可用。
* [ ] 历史可查看和导出。
* [ ] Token 和费用按调用记录。
* [ ] 至少三项 RoboCup 场景定制有代码和测试。
* [ ] 课程演示可离线运行。
* [ ] 真实 Qwen smoke test 有结果或明确 live-conditional 说明。

---

# 三十八、必须提供的离线演示

至少提供：

```bash
annotagent demo generic-classification
annotagent demo generic-detection-crop
annotagent demo robocup-ball
```

`robocup-ball` 必须稳定重现：

## Case 1：正常足球

```text
候选
→ Validator pass
→ Gate
→ Commit
```

## Case 2：白鞋误检

```text
候选位于 robot 下部
→ PossibleWhiteShoe
→ 查询 Memory
→ Crop Classification
→ Reject 或 Review
```

## Case 3：点球点误检

```text
白色圆形候选
→ PossiblePenaltyMark
→ 不直接接受
→ Review
```

## Case 4：Memory 改变行为

```text
第一次：
白鞋候选 → 人工拒绝 → 写入 Memory

第二次：
相似候选 → Memory 风险提升 → 自动进入 Crop Verify
```

演示必须显示：

* Agent steps；
* Skill loaded；
* Tool calls；
* Validator；
* Memory；
  -最终 decision；
* Token；
  -费用；
  -停止原因。

---

# 三十九、真实模型验证

真实 Qwen 验证使用当前配置系统，不提交 Key。

最低 smoke test：

* 5 张图片；
* 至少 1 个正常足球；
* 至少 1 个 hard negative；
* VLM Detection；
* Crop Classification；
* Tool Call 历史；
* usage；
* cost；
* timeout；
* cancel。

若没有有效 Key或外部服务不可用：

* 记录为 `live-conditional`；
* 不阻塞 Mock、Runtime、Skill、GUI 和测试；
* 不伪造成功；
* 不在文档中写“已验证”。

真实 YOLO Worker 同理：

* 有服务时跑 5 张；
* 无服务时完成 Mock 和 HTTP 协议测试。

---

# 四十、自动测试

Rust：

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

增加浏览器测试：

1. 空工作区不出现 RoboCup；
2. Generic Project 不出现 RoboCup；
3. RoboCup Project 显示 `robocup.ball`；
4. Advisor 生成 Draft；
5. Invalid Draft 被修复；
6. Advisor 不自动 Publish；
7. Run Trace 显示 Skill 和 Agent Step；
8. 白鞋候选进入 Recovery；
9. Review 写入 Correction Memory；
10. 第二次运行行为改变；
11. active run 恢复；
12. cancel；
13. URL refresh；
14. Skill 页面分层。

---

# 四十一、课程演示脚本

创建：

```text
docs/DEMO_AGENT_SKILL.md
```

5 分钟脚本：

```text
0:00–0:30
真实问题：RoboCup 足球小目标容易和白鞋、点球点混淆。

0:30–1:00
架构：AnnotAgent Agent Core + Capability Skills + RoboCup Ball Domain Skill。

1:00–1:40
Generic Classification Demo，证明 Core 不依赖 RoboCup。

1:40–2:20
创建 RoboCup Project，启用 VLM Detection、Classification 和 robocup.ball。

2:20–3:00
Advisor 检查 Project、Skills 和 Models，生成 Draft，Static Validator 返回问题，Agent 自动修复。

3:00–3:35
Dry Run：普通足球直接通过，白鞋候选触发 Recovery Agent。

3:35–4:10
Recovery Agent 查询 Memory、调用 Crop Classification，进入 Review。

4:10–4:35
人工拒绝并记录 white_shoe_as_ball。

4:35–4:50
再次运行，相似候选因 Memory 自动走更严格路径。

4:50–5:00
展示 Token、费用、历史和停止条件。
```

---

# 四十二、不得做的假实现

禁止：

* 只把固定 Workflow 改名为 Agent；
* 只在页面上增加“Agent”徽章；
* 一次 LLM 请求后直接称为 Agent Loop；
* 把所有小函数都包装成 Skill；
* 把 Crop 放进 YOLO Skill；
* 只保存 Correction Memory，不让它影响行为；
* 只在 Prompt 中写白鞋风险，不实现 Validator；
* 用前端假 Trace 冒充 Tool Calls；
* 让 VLM 重新输出 Tool 已产生的精确几何；
* 用增大最大步数掩盖协议错误；
* 在 Core 中写 RoboCup 分支；
* 在 Generic Project 中加载 RoboCup；
* 创建空的 `robocup.robot` 和 `robocup.field` Rust 实现后声称支持；
* 用 disabled 按钮声称功能完成；
* 修改视觉系统回到 RoboCup 主品牌；
* push；
* 修改 remote；
* 提交 API Key。

---

# 四十三、最终报告格式

完成后最终报告必须包含：

## 1. 实际实现的 Agent Loop

分别说明：

* Workflow Advisor Agent；
* Annotation Recovery Agent；
* 每个 Agent 的 Tools；
  -停止条件；
  -人工边界。

## 2. Tool、Node、Model 和 Skill 边界

说明：

* 什么属于 Core；
* 什么属于 Capability Skill；
* 什么属于 Domain Skill；
* 如何证明 Core 没有写死 RoboCup。

## 3. Capability Skills

说明实际完成：

* Classification；
* VLM Detection；
* YOLO Detection；
* Mock；
* HTTP；
* OpenAI-compatible。

## 4. RoboCup Ball Skill

说明：

* hard-negative；
* field relation；
* recovery policy；
* templates；
* review reasons。

## 5. Correction Memory

说明：

* 保存内容；
  -检索方式；
  -如何影响第二次运行；
  -对应测试。

## 6. Artifact 与 Tool Call

说明：

-完整 Tool Call 历史；

* Artifact lineage；
  -几何数据如何传递；
* Replay 和 Cache。

## 7. 产品和课程要求

说明：

* AnnotAgent 主品牌；
* RoboCup Skill 上下文；
* R1–R6；
  -至少三项专用定制。

## 8. 测试

列出实际运行命令和真实结果。

不得报告未执行的测试为通过。

## 9. Milestone 提交

列出：

```text
commit hash
commit message
Milestone
```

## 10. Live-conditional 项

明确说明：

* Qwen；
* YOLO Worker；
  -浏览器人工交互；
  -任何外部依赖。

## 11. 未完成内容

不得使用：

```text
基本完成
理论上支持
应该可用
大概率没问题
```

## 12. Git 状态

说明：

-当前分支；
-工作区是否干净；
-领先远程提交数；
-未 push；
-remote 未修改。

---

# 四十四、启动指令

将本文保存为：

```text
docs/execution/AGENT_SKILL_MASTER_PROMPT.md
```

然后从仓库根目录开始执行：

```text
阅读 docs/execution/AGENT_SKILL_MASTER_PROMPT.md，并将其作为本次长期任务的最高目标。

先检查 Git、代码、测试和现有执行状态，不要盲信文档中的完成声明。

从 Milestone 0 开始持续执行。普通技术决策自行完成并记录到 AGENT_SKILL_DECISIONS.md。每完成一个 Milestone，更新状态和验收证据，运行测试，创建独立本地提交，然后继续下一 Milestone。

本轮近期重点是：
1. 建立正确的 Agent、Tool、Node、Model、Capability Skill、Domain Skill 边界；
2. 实现真实 Workflow Advisor Agent；
3. 实现真实 Annotation Recovery Agent；
4. 完成 Classification、VLM Detection 和 YOLO Detection Capability Skills；
5. 完成 robocup.ball Domain Skill；
6. 让 Correction Memory 真实改变后续决策；
7. 保持 AnnotAgent 为全局产品身份；
8. 满足课程 R1–R6 与场景定制要求。

除真实外部阻塞外不要停下来询问。
外部服务不可用时完成 Mock、协议、Runtime、UI、测试和文档，并把真实模型验证标记为 live-conditional。

不要 push。
不要修改 Git remote。
不要使用、恢复或提交任何对话中出现过的 API Key。
不要执行 reset、rebase、amend 或破坏性 checkout。

只有 Release Blocking Acceptance Matrix 全部满足，或只剩明确记录的 live-conditional 项时，才输出最终报告。
```
