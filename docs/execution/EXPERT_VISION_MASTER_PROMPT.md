# AnnotAgent Expert Vision SDK + Evidence-Driven Pipeline Builder Alpha

你现在是 AnnotAgent 仓库的长期主实现工程师。

本次任务的目标不是继续为 SAM、YOLO、RF-DETR、LocateAnything、PIDNet、Grounding DINO 等模型分别增加专用页面和专用分支，而是建立一套统一的专家视觉模型接入机制，并让 Pipeline Builder Agent 能够根据真实能力、健康状态和 Dry Run 证据，合法地把这些模型拼入标注调用链。

本次任务必须解决两个问题：

1. 新增一个专家视觉模型时，开发者不需要修改 AnnotAgent Core；
2. Pipeline Builder Agent 不再盲目相信 VLM 输出的几何结果，而是根据错误类型和质量证据，决定是否加入检测器、分块、Crop、分类器、SAM、语义分割、领域 Validator 或人工 Review。

最终效果：

```text
注册专家模型
→ 自动发现 Capability
→ 生成 Model Profile
→ 注册合法输入输出 Contract
→ 出现在兼容模型选择器中
→ Pipeline Builder Agent 检查其状态
→ Agent 构造合法节点链
→ Rust 静态校验
→ Dry Run
→ 根据几何、语义、成本和失败证据修订
→ 人工批准
→ 发布不可变 Workflow Version
```

产品名称始终是：

```text
AnnotAgent
```

RoboCup 只是 Domain Skill 和示例 Project。

模型品牌不得成为 Core Node 类型。

---

# 一、必须先核验当前实现

开始前执行：

```bash
git status --short --branch
git log --oneline -20
cargo test --workspace --all-features
```

检查：

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
```

重点核验：

* 当前 Model Registry；
* 当前 Vision Worker Registry；
* 当前 Provider Registry；
* 当前 HTTP Vision Protocol；
* 当前 SAM Worker 和 Rust Adapter；
* 当前 YOLO Worker；
* 当前 RF-DETR Worker；
* 当前 LocateAnything Worker；
* 当前 Classification Skill；
* 当前 Detection Skill；
* 当前 Segmentation Skill；
* 当前 Workflow Advisor；
* 当前 Advisor Prompt；
* 当前 Agent Tool Registry；
* 当前 Workflow Static Validator；
* 当前 Artifact 类型；
* 当前 Model Health；
* 当前 Score Semantics；
* 当前 Dry Run Summary；
* 当前 Review Reason；
* 当前默认 RoboCup Workflow；
* 当前测试和 Known Limitations。

不要因为代码文件存在就声称能力可用。

必须区分：

```text
Adapter implemented
Worker implemented
Worker process running
Weights configured
Health check passing
Smoke test passing
Pipeline path registered
Advisor allowed to select
```

这些是六种不同状态，不得混成一个 `supported = true`。

禁止：

```text
git reset
git rebase
git commit --amend
破坏性 checkout
修改 Git remote
push
提交模型权重
使用或恢复任何对话中出现过的 API Key
用 Mock 结果冒充真实模型
```

---

# 二、长期状态文档

创建并持续维护：

```text
docs/execution/EXPERT_VISION_MASTER_PLAN.md
docs/execution/EXPERT_VISION_STATUS.md
docs/execution/EXPERT_VISION_DECISIONS.md
docs/execution/EXPERT_VISION_ACCEPTANCE.md
docs/execution/EXPERT_VISION_BLOCKERS.md
docs/execution/EXPERT_VISION_KNOWN_LIMITATIONS.md
```

`EXPERT_VISION_STATUS.md` 必须持续记录：

```text
当前 Milestone
已完成
正在进行
下一步
最近 Rust 测试
最近 Web 测试
最近 Worker Contract 测试
最近浏览器验证
最近本地提交
Release Blocking 剩余项
Live-conditional 项
真实 Blocker
```

每完成一个 Milestone：

1. 更新状态；
2. 更新验收证据；
3. 执行相关测试；
4. 修复回归；
5. 创建独立本地提交；
6. 继续下一 Milestone；
7. 不等待用户确认。

---

# 三、最终分层

必须固定以下层级：

```text
AnnotAgent
│
├── LLM / VLM Provider Registry
│   ├── OpenAI-compatible Provider
│   ├── Qwen Provider Profile
│   ├── OpenRouter Provider Profile
│   └── Credentials
│
├── Expert Vision Worker Registry
│   ├── SAM Worker
│   ├── YOLO Worker
│   ├── RF-DETR Worker
│   ├── LocateAnything Worker
│   ├── PIDNet Worker
│   ├── Grounding DINO Worker
│   └── Future Worker
│
├── Model Profiles
│   ├── Provider-backed LLM / VLM model
│   ├── Worker-backed expert vision model
│   └── Mock model
│
├── Capability Skills
│   ├── Detection
│   ├── Classification
│   └── Segmentation
│
├── Core Nodes
│   ├── Image preparation
│   ├── Artifact conversion
│   ├── Evidence and validation
│   ├── Decision
│   ├── Review
│   └── Commit
│
├── Domain Skills
│   └── robocup.ball
│
├── Pipeline Builder Agent
│   ├── inspect
│   ├── construct
│   ├── validate
│   ├── dry run
│   ├── diagnose
│   ├── revise
│   └── submit for approval
│
└── Deterministic Workflow Runtime
```

---

# 四、Provider 与 Expert Vision Worker 不得混为一谈

## 4.1 Provider

Provider 管理 LLM/VLM API 服务：

* Base URL；
* API 协议；
* API Key；
* Workspace；
* Header；
  -连接策略；
  -模型发现；
* Token；
  -费用；
  -限流；
  -健康状态。

例如：

```text
Alibaba DashScope
OpenAI
OpenRouter
Custom OpenAI-compatible
Local vLLM
```

## 4.2 Expert Vision Worker

Vision Worker 管理本地或远程专用视觉推理服务：

* Worker Endpoint；
* Worker Protocol；
  -模型权重身份；
* GPU / CPU；
* Capability；
  -输入输出 Contract；
* Label Space；
* Score Semantics；
* License；
  -健康状态；
  -版本；
  -最近延迟。

例如：

```text
SAM 2 Worker
YOLO Worker
RF-DETR Worker
LocateAnything Worker
PIDNet Worker
Grounding DINO Worker
```

## 4.3 Model Profile

Model Profile 是用户和 Workflow 实际选择的模型。

它可以由两种连接来源支持：

```rust
pub enum ModelConnection {
    ProviderModel {
        provider_id: ProviderId,
        remote_model_id: String,
    },
    VisionWorkerModel {
        worker_id: VisionWorkerId,
        worker_model_id: String,
    },
    Mock {
        fixture_id: String,
    },
}
```

Pipeline Builder Agent 只选择 Model Profile。

它不能读取 API Key，也不能修改 Provider 或 Worker Endpoint。

---

# 五、模型品牌不得成为 Core Node

禁止：

```rust
NodeKind::Sam
NodeKind::Yolo
NodeKind::RfDetr
NodeKind::LocateAnything
NodeKind::PidNet
NodeKind::GroundingDino
```

禁止：

```rust
if model_id == "sam2" { ... }
if model_id.starts_with("rfdetr") { ... }
```

Core 只认识 Capability：

```rust
pub enum ModelCapability {
    VisionLanguage,
    ImageClassification,
    ObjectDetection,
    OpenVocabularyDetection,
    PhraseGrounding,
    SemanticSegmentation,
    PromptedSegmentation,
    InstanceSegmentation,
    KeypointDetection,
}
```

正确表达：

```text
Node: Prompted Segmentation
Model Binding: sam2-local
```

而不是：

```text
Node: SAM 2.1
```

用户界面可以展示模型名，但 Runtime 分支只能基于 Capability、Contract 和配置工作。

---

# 六、统一 Expert Model Manifest

所有专家视觉模型必须通过 Manifest 注册。

建议：

```rust
pub struct ExpertModelManifest {
    pub schema_version: String,
    pub model_id: String,
    pub display_name: String,
    pub architecture: Option<String>,
    pub model_version: String,
    pub connection: ModelConnection,
    pub capabilities: BTreeSet<ModelCapability>,
    pub input_contracts: Vec<ArtifactContract>,
    pub output_contracts: Vec<ArtifactContract>,
    pub prompt_contracts: Vec<PromptContract>,
    pub score_semantics: ScoreSemantics,
    pub geometry_semantics: GeometrySemantics,
    pub label_space: Option<Vec<String>>,
    pub checkpoint: Option<CheckpointIdentity>,
    pub runtime_requirements: RuntimeRequirements,
    pub license: LicenseMetadata,
    pub availability: ModelAvailability,
}
```

几何语义：

```rust
pub enum GeometrySemantics {
    NotApplicable,
    CoarseHypothesis,
    PredictedGeometry,
    MaskRefinedGeometry,
    CalibratedGeometry,
    HumanVerified,
}
```

重点：

* VLM Detection 默认是 `CoarseHypothesis`；
* YOLO、RF-DETR 默认是 `PredictedGeometry`；
* SAM Mask 转换后的框是 `MaskRefinedGeometry`；
* 人工修改后是 `HumanVerified`；
* `confidence = 0.98` 不等于 `GeometrySemantics::CalibratedGeometry`。

模型置信度和几何精度必须是两个独立概念。

---

# 七、统一 HTTP Vision Worker Protocol

现有协议应向后兼容扩展，不重新建立另一套接口。

至少支持：

```text
GET  /health
GET  /v1/capabilities
GET  /v1/models
GET  /v1/contracts
POST /v1/infer
POST /v1/cancel
POST /v1/warmup
```

`warmup` 可选。

## 7.1 Capability 响应

```json
{
  "protocol_version": "1",
  "worker_id": "sam2-local",
  "models": [
    {
      "model_id": "sam2.1-hiera-small",
      "capabilities": ["prompted_segmentation"],
      "input_contracts": [
        {
          "image": "ImageArtifact",
          "prompts": "BoxPromptSet|PointPromptSet"
        }
      ],
      "output_contracts": ["MaskSetArtifact"],
      "score_semantics": "relative_confidence",
      "geometry_semantics": "mask_refined_geometry"
    }
  ]
}
```

## 7.2 推理请求

请求只传受控图片数据和 Artifact，不传宿主机任意路径。

```json
{
  "protocol_version": "1",
  "request_id": "uuid",
  "run_context": {
    "project_id": "...",
    "run_id": "...",
    "image_id": "...",
    "node_id": "..."
  },
  "model_id": "sam2.1-hiera-small",
  "operation": "prompted_segmentation",
  "inputs": {
    "image": {
      "content_type": "image/jpeg",
      "data": "..."
    },
    "prompts": {
      "kind": "box",
      "items": []
    }
  },
  "options": {}
}
```

## 7.3 推理响应

```json
{
  "protocol_version": "1",
  "request_id": "uuid",
  "model_id": "sam2.1-hiera-small",
  "outputs": {
    "artifact_type": "mask_set",
    "items": []
  },
  "usage": {
    "duration_ms": 42,
    "device": "cuda"
  }
}
```

所有响应必须经过 Rust 校验：

* 非法坐标；
* NaN；
* Infinity；
  -未知 Label；
  -重复 ID；
  -超大 Mask；
  -错误维度；
  -未知 Artifact；
  -协议版本不兼容；
  -响应体过大。

---

# 八、提供 Expert Vision Worker SDK

为了让后续添加模型方便，建立：

```text
sdk/python/annotagent_vision_worker/
```

至少包含：

```text
Protocol models
Pydantic request/response schema
Health endpoint helpers
Capability endpoint helpers
Image decoding
Cancellation registry
Coordinate normalization
Artifact serialization
Error mapping
Conformance tests
Example worker
```

提供 CLI：

```bash
annotagent worker scaffold \
  --name my-detector \
  --capability object_detection \
  --language python
```

生成：

```text
workers/my-detector/
├── app.py
├── model.py
├── manifest.yaml
├── requirements.txt
├── tests/
└── README.md
```

再提供预设脚手架：

```bash
annotagent worker scaffold --preset sam2
annotagent worker scaffold --preset yolo
annotagent worker scaffold --preset rfdetr
annotagent worker scaffold --preset locate-anything
annotagent worker scaffold --preset pidnet
annotagent worker scaffold --preset grounding-dino
```

Preset 只生成 Worker 模板和 Manifest。

不得修改 Rust Core。

---

# 九、首批专家模型映射

## 9.1 SAM

Capability：

```text
PromptedSegmentation
```

输入：

```text
ImageArtifact
BoxPromptSet 或 PointPromptSet
```

输出：

```text
MaskSetArtifact
```

SAM 不负责发现目标类别。

SAM 不能解决：

```text
没有候选框
目标语义识别错误
VLM 后端请求失败
```

SAM 适合解决：

```text
已经有粗框
目标语义大体正确
但边界或框的几何不够精确
```

合法节点链：

```text
DetectionSet
→ Convert detections to box prompts
→ Prompted segmentation
→ Mask quality validation
→ Mask to bounding box
→ Refined DetectionSet
```

## 9.2 YOLO

Capability：

```text
ObjectDetection
```

输出：

```text
DetectionSetArtifact
```

适合：

* 固定 Label Space；
  -低延迟；
  -批量检测；
  -已有训练权重。

## 9.3 RF-DETR

Capability：

```text
ObjectDetection
```

输出同样是：

```text
DetectionSetArtifact
```

YOLO 和 RF-DETR 必须使用相同 Node 和 Artifact Contract。

区别只存在于 Model Profile、分数语义、版本和 Worker。

## 9.4 LocateAnything

Capability：

```text
OpenVocabularyDetection
PhraseGrounding
```

输入自然语言 Query。

输出：

```text
DetectionSetArtifact
```

若模型没有可信 score：

```text
score.value = None
score.semantics = NotProvided
```

不得伪造分数。

## 9.5 PIDNet

Capability：

```text
SemanticSegmentation
```

输出：

```text
SemanticMaskArtifact
```

可以通过转换节点生成：

```text
Polygon
Field region
Region constraint
```

## 9.6 Grounding DINO

Capability：

```text
OpenVocabularyDetection
PhraseGrounding
```

与 LocateAnything 共用 Capability Contract。

模型品牌不产生新的 Core Node。

---

# 十、补齐公开可组合节点链

当前必须提供以下领域无关节点。

## 10.1 图像准备

```text
core.resize
core.tile
core.crop
core.project_coordinates
```

`tile` 用于小目标。

配置：

```text
tile size
overlap
maximum tiles
merge policy
```

## 10.2 模型推理

```text
capability.detect
capability.classify
capability.segment
```

操作由 Model Capability 决定。

## 10.3 Prompt 转换

```text
core.detections_to_box_prompts
core.points_to_point_prompts
core.annotations_to_prompts
```

SAM 类模型只有在 Prompt 转换路径存在时才允许进入 Workflow。

## 10.4 几何转换

```text
core.mask_to_bbox
core.mask_to_polygon
core.mask_to_polyline
core.keypoints_to_bbox
core.merge_tiles
```

## 10.5 结果关联

```text
core.select_and_map
core.attach_result
core.combine_evidence
```

## 10.6 质量和决策

```text
core.evaluate_geometry
core.validate
core.decision
core.human_review
core.commit
```

不得把 SAM 的“Mask 转紧致框”藏在 SAM Worker 内部作为不可审计黑箱。

Worker 输出 Mask。

Rust Core 通过显式节点转换为 BBox。

这样用户和 Agent 才能查看：

```text
VLM 原框
SAM Mask
SAM 紧致框
最终框
```

---

# 十一、建立 Artifact Conversion Registry

实现：

```rust
pub struct ArtifactConversionRegistry;
```

记录合法转换：

```text
DetectionSet → BoxPromptSet
BoxPromptSet + Image → MaskSet
MaskSet → DetectionSet
MaskSet → PolygonSet
DetectionSet → CropSet
CropSet → ClassificationSet
ClassificationSet + DetectionSet → AnnotationCandidateSet
```

提供：

```rust
fn find_conversion_path(
    from: ArtifactType,
    to: ArtifactType,
    available_nodes: &NodeRegistry,
) -> Vec<ConversionPath>;
```

Pipeline Builder Agent 使用工具：

```text
find_artifact_conversion_path
```

例如：

```text
当前：
VLM Detection 输出 DetectionSet

目标：
更精确的 Bounding Box

系统发现：
DetectionSet
→ BoxPromptSet
→ PromptedSegmentation
→ MaskSet
→ MaskToBBox
→ DetectionSet
```

Agent 才能合法建议 SAM。

如果缺少任何一个节点、Capability 或健康 Model：

```text
conversion path unavailable
```

Agent 不得将该链加入 Draft。

---

# 十二、明确区分错误类型

新增统一诊断：

```rust
pub enum AnnotationFailureClass {
    InfrastructureFailure,
    ProviderFailure,
    NoCandidate,
    SemanticError,
    GeometryError,
    MissingScore,
    DomainRisk,
    InvalidArtifact,
    BudgetLimit,
}
```

## 12.1 Infrastructure / Provider Failure

例如：

```text
Qwen timeout
API Key 无效
Worker 未启动
```

正确动作：

```text
重试
切换可用 Provider
使用配置好的 fallback
人工处理
```

错误动作：

```text
增加 SAM
```

SAM 无法修复一个根本没产生候选的 API 请求。

## 12.2 No Candidate

正确候选：

```text
Tile image
Zoom / crop search region
Use open-vocabulary detector
Use specialist detector
Human review
```

SAM 通常不适合，因为没有 Prompt。

## 12.3 Semantic Error

例如：

```text
白鞋被认为是足球
```

正确候选：

```text
Crop classification
Domain Validator
Second detector
Correction Memory
Review
```

SAM 只能优化白鞋的边界，无法把白鞋变成足球。边界更准地框错目标，仍然是错。

## 12.4 Geometry Error

例如：

```text
框偏大
框偏移
目标只占框的一小部分
人工总是收紧 bbox
```

正确候选：

```text
Prompted segmentation
Mask to bbox
Geometry refinement
Specialist detector
```

这里才适合 SAM。

## 12.5 Missing Score

正确候选：

```text
Evidence Decision
Secondary verification
Review
```

不能把缺失分数填成 0.9。

## 12.6 Domain Risk

正确候选：

```text
Domain Validator
Correction Memory
Domain-specific review
```

---

# 十三、VLM 几何可信度模型

建立：

```rust
pub struct GeometryQualityReport {
    pub artifact_id: ArtifactId,
    pub geometry_semantics: GeometrySemantics,
    pub clipped_to_image: bool,
    pub aspect_ratio_outlier: bool,
    pub area_ratio: Option<f32>,
    pub foreground_occupancy: Option<f32>,
    pub edge_support: Option<f32>,
    pub mask_support: Option<f32>,
    pub center_shift_from_refiner: Option<f32>,
    pub area_change_from_refiner: Option<f32>,
    pub iou_with_refiner: Option<f32>,
    pub manual_center_shift: Option<f32>,
    pub manual_area_change: Option<f32>,
    pub historical_correction_rate: Option<f32>,
    pub issue_codes: Vec<String>,
}
```

Dry Run 汇总：

```rust
pub struct GeometryQualitySummary {
    pub total_candidates: u32,
    pub coarse_geometry_count: u32,
    pub geometry_review_count: u32,
    pub human_adjustment_count: u32,
    pub mean_manual_center_shift: Option<f32>,
    pub mean_manual_area_change: Option<f32>,
    pub mean_refiner_iou: Option<f32>,
    pub inaccurate_bbox_reason_count: u32,
}
```

Pipeline Builder Agent 不得根据一句泛化知识直接断言：

```text
VLM 一定框不准
```

正确规则：

> VLM 输出默认作为未经标定的粗几何假设；是否需要几何 Refiner，应结合模型类型、目标大小、Dry Run、人工修正和可用 Capability 决定。

---

# 十四、Advisor 的强制系统规则

更新 Pipeline Builder Agent 的系统约束和 Skill Resource。

必须包含以下含义：

```text
1. VLM 的语义置信度不等于几何精度。
2. VLM 生成的 Bounding Box 默认是 coarse, uncalibrated hypothesis。
3. 高 confidence 不能证明 bbox 边界紧致。
4. Provider 调用失败不是几何误差证据。
5. 没有候选时，不要建议依赖候选 Prompt 的 SAM。
6. 语义误检时，优先考虑 Crop Classification、Domain Validator 或第二检测器，而不是 SAM。
7. 已有语义正确但几何质量差的粗框，并且健康的 Prompted Segmentation Backend 可用时，可以建议：
   Detection → Box Prompt → Prompted Segmentation → Mask to BBox。
8. 对小目标，可以优先建议 Tile 或 Zoom，再考虑检测器。
9. 有训练好的 Specialist Detector 且 Label Space 覆盖目标时，优先考虑 Specialist。
10. 没有专用模型时，可以使用 Open-vocabulary Detection 冷启动。
11. 不得选择 unavailable、disabled、unconfigured 或 incompatible Model Profile。
12. Labs Model 可以作为未应用的 Alternative，但不能进入可发布 Draft。
13. 不得伪造 Capability、置信度、模型健康状态或 Benchmark。
14. 所有建议必须通过 Rust Static Validation。
15. 所有建议必须先成为 Draft，并由人批准。
```

不要只在 `advisor.md` 加一句“VLM 不准确”。

必须让 Runtime、Artifact、Dry Run 和 Model Registry 提供支持这些判断的真实数据。

---

# 十五、Pipeline Builder Agent 工具

Agent 至少拥有以下受控工具。

## 15.1 检查项目

```text
inspect_project
inspect_label_schema
inspect_label
sample_dataset
inspect_sample_image
inspect_existing_pipeline
```

## 15.2 检查模型和能力

```text
list_available_capabilities
list_compatible_models
inspect_model_profile
inspect_worker_health
inspect_model_contracts
inspect_label_space
inspect_score_semantics
inspect_geometry_semantics
```

## 15.3 查找合法流程

```text
list_node_definitions
inspect_node_definition
find_artifact_conversion_path
list_pipeline_templates
check_capability_path
```

## 15.4 修改 Draft

```text
create_pipeline_draft
create_draft_from_template
add_pipeline_node
remove_pipeline_node
connect_pipeline_nodes
disconnect_pipeline_nodes
bind_model_profile
set_node_configuration
set_label_mapping
set_decision_policy
undo_last_draft_change
```

## 15.5 校验和试跑

```text
validate_pipeline
estimate_pipeline_cost
dry_run_pipeline
inspect_dry_run_summary
inspect_failure_classes
inspect_geometry_quality
inspect_failed_samples
inspect_review_samples
inspect_node_artifacts
compare_dry_runs
```

## 15.6 结束

```text
submit_draft_for_human_approval
finish_agent_session
```

禁止：

```text
execute_shell
execute_python
install_dependency
download_weights
create_provider
modify_api_key
publish_pipeline
start_full_dataset_run
```

---

# 十六、Agent 推荐策略

Pipeline Builder Agent 应当根据证据采用以下策略。

## Case A：只有 VLM，尚无质量证据

建议：

```text
Image
→ VLM Detection
→ Geometry Validation
→ Domain Validation
→ Decision
→ Commit / Review
```

不要自动加入 SAM。

先 Dry Run。

## Case B：VLM 产生候选，但人工频繁收紧框

如果：

```text
inaccurate_bbox_reason_count 高
manual center shift 高
manual area change 高
```

且存在健康的 Prompted Segmentation Model：

建议：

```text
VLM Detection
→ Box Prompt
→ SAM / Prompted Segmentation
→ Mask Quality
→ Mask to BBox
→ Validation
→ Decision
```

## Case C：VLM 完全没有产生候选

不要建议 SAM。

候选：

```text
Tile
Open-vocabulary Detector
Specialist Detector
Alternative VLM
Review
```

## Case D：VLM 把白鞋识别成足球

不要把 SAM 当成语义修复。

建议：

```text
Crop
→ Classification Verify
→ RoboCup Ball Validator
→ Decision
```

SAM 可以在语义确认后用于紧致框，但不能代替语义验证。

## Case E：已有 YOLO / RF-DETR 专用模型

优先建议：

```text
Specialist Detection
→ Domain Validation
→ Decision
```

低分、空结果或领域风险再调用 VLM 或 Open-vocabulary fallback。

## Case F：LocateAnything 输出无可信分数

使用：

```text
Evidence Decision
Crop Verify
Secondary Detector
Review
```

不要接普通 Confidence Gate 并编造默认分数。

## Case G：小目标

考虑：

```text
Resize / Tile
→ Detection
→ Merge Tiles
→ Optional Refinement
```

不得默认把整张高分辨率图直接塞给所有模型，然后对账单和召回率同时感到惊讶。

---

# 十七、专家模型接入向导

在：

```text
Settings → Vision Workers
```

实现：

```text
Add expert model
```

流程：

## Step 1：选择方式

```text
Use preset
Connect generic HTTP worker
Use mock worker
```

Presets：

```text
SAM
YOLO
RF-DETR
LocateAnything
PIDNet
Grounding DINO
Custom
```

## Step 2：连接

```text
Endpoint
Remote access policy
Timeout
Authentication reference
```

## Step 3：发现能力

调用：

```text
/health
/v1/capabilities
/v1/models
/v1/contracts
```

显示真实结果。

## Step 4：配置模型身份

```text
Model ID
Architecture
Version
Checkpoint hash
Training dataset version
Label space
License
```

## Step 5：运行样本测试

用户明确选择一张示例图。

展示：

```text
Input
Raw output summary
Converted Artifact
Coordinates
Score semantics
Geometry semantics
Duration
Warnings
```

## Step 6：注册

只有以下条件满足才可设为 Available：

```text
Health pass
Protocol compatible
Required contracts valid
Sample conversion pass
Model identity complete
```

没有权重的 Worker 必须显示：

```text
Missing weights
```

不能显示成 Available。

---

# 十八、模型可用性状态

```rust
pub enum ModelAvailability {
    Unconfigured,
    MissingWeights,
    Disabled,
    Unknown,
    Available,
    Unreachable,
    IncompatibleProtocol,
    InvalidContract,
    FailedSmokeTest,
}
```

Pipeline Builder Agent 只能把：

```text
Available
```

的模型加入可发布 Draft。

以下状态可以出现在 Alternative 中：

```text
Unconfigured
MissingWeights
Disabled
Unknown
```

但必须标明：

```text
Requires setup
Not applied to draft
```

---

# 十九、迁移现有模型实现

不得重写现有 Worker。

将现有实现迁移到统一模型体系。

## 19.1 SAM

把现有专用 `sam_prompted_refiner` 兼容层迁移为：

```text
core.detections_to_box_prompts
→ capability.segment
→ core.mask_to_bbox
```

保留旧 Workflow 的读取兼容。

旧节点可以在载入时迁移为新节点链。

## 19.2 YOLO

迁移为：

```text
Capability: ObjectDetection
Connection: VisionWorkerModel
```

## 19.3 RF-DETR

与 YOLO 使用同一 Detection Node。

## 19.4 LocateAnything

迁移为：

```text
OpenVocabularyDetection
PhraseGrounding
```

保留 `score = None`。

## 19.5 Grid-assisted VLM Grounding

保留为 VLM Detection Node 的图像预处理配置：

```yaml
grounding_assist:
  mode: grid
  enabled: true
```

不再作为独立专家模型。

---

# 二十、RoboCup Ball Skill 的职责

`robocup.ball` 不得写死：

```text
SAM
YOLO
RF-DETR
LocateAnything
Qwen
```

它只声明能力偏好：

```yaml
required:
  - detection
  - review

optional:
  - classification
  - prompted_segmentation
  - open_vocabulary_detection
  - semantic_segmentation
```

它提供：

* hard-negative knowledge；
* white shoe / white sock / penalty mark / field-line intersection；
* field relation；
* review reasons；
* geometry issue taxonomy；
* Advisor Resource；
* Validator；
* Correction Memory。

Agent 根据 Registry 中当前可用 Model 决定具体实现。

---

# 二十一、GUI 中展示推荐原因

Advisor 建议 SAM 时必须显示：

```text
Why this is recommended

In the latest sample test:
- 5 of 8 candidate boxes required manual resizing
- average box area decreased by 41%
- a prompted segmentation model is available
- its output can be converted back to bounding boxes

Proposed change:
Detection
→ Prompted segmentation
→ Mask to bounding box
```

不建议 SAM 时也应解释：

```text
Prompted segmentation was not added

The detection backend failed before producing candidate boxes.
SAM requires a box or point prompt, so it cannot repair this failure.
```

或者：

```text
Prompted segmentation was not added

No healthy prompted-segmentation model is configured.
```

这比让用户猜 GLM 为什么突然对 SAM 失去兴趣稍微文明一些。

---

# 二十二、Dry Run 增加质量统计

Dry Run Summary 增加：

```text
Provider failures
Worker failures
No-candidate count
Semantic review count
Geometry review count
Missing-score count
Domain-risk count
Manual resize count
Average center shift
Average area adjustment
Refiner usage count
Refiner success count
Refiner fallback count
```

Agent 只能根据结构化数据修改流程。

例如：

```text
4 张图片全部 VLM Provider 失败
```

结论必须是：

```text
Provider / model availability issue
```

不能得出：

```text
VLM 框不准，需要 SAM
```

---

# 二十三、测试场景

必须实现以下确定性测试。

## Case 1：VLM Provider 失败

```text
VLM 没有产生 DetectionSet
→ failure class = ProviderFailure
→ Agent 不建议 SAM
→ 建议修复 Provider 或使用可用 Detection Model
```

## Case 2：VLM 粗框偏大

```text
VLM 产生粗框
→ Geometry Quality 差
→ SAM available
→ Agent 查找到合法转换路径
→ 加入 Prompted Segmentation + MaskToBBox
```

## Case 3：SAM 不可用

```text
Geometry Quality 差
→ SAM unavailable
→ Agent 不把 SAM 加入 Draft
→ 提供 Alternative 和 Setup Action
→ 当前 Draft 使用 Review
```

## Case 4：白鞋误检

```text
VLM 正确产生框但语义错误
→ RoboCup PossibleWhiteShoe
→ Agent 加 Crop Classification
→ 不把 SAM 作为主要修复
```

## Case 5：RF-DETR 已配置

```text
Specialist label space 包含 football
→ Advisor 优先 Specialist Detection
→ VLM 只作为 fallback 或 verifier
```

## Case 6：LocateAnything 无分数

```text
Open-vocabulary Detection
→ score None
→ Agent 使用 Evidence Decision
→ 不使用普通 Confidence Gate
```

## Case 7：小目标

```text
样本目标面积很小
→ Agent 建议 Tile
→ Tile Detection
→ Merge Tiles
```

## Case 8：新增未知专家模型

创建测试 Worker：

```text
TestEdgeDetector
Capability: ObjectDetection
```

要求：

* 不修改 Core；
* 通过 Manifest 注册；
  -通过 Contract 测试；
  -出现在兼容模型列表；
  -Agent 可以绑定；
  -可以完成 Dry Run。

这项是扩展性的核心验收。

---

# 二十四、Milestones

## Milestone 0：基线和模型盘点

完成：

* 核验现有 Worker；
  -核验现有 Adapter；
  -核验 Registry；
  -核验 Advisor；
  -建立状态文档；
  -列出迁移清单；
  -建立测试基线。

提交：

```text
docs: establish expert vision integration baseline
```

## Milestone 1：Expert Model Manifest

完成：

* Model Connection；
* Expert Model Manifest；
* Geometry Semantics；
* Artifact Contracts；
* Model Availability；
* migrations；
  -单元测试。

提交：

```text
feat(models): add capability-driven expert model manifests
```

## Milestone 2：Worker SDK 和 Protocol Contract

完成：

* Python Worker SDK；
* scaffold 命令；
* conformance tests；
* capability discovery；
* model discovery；
* contract discovery；
  -安全校验。

提交：

```text
feat(workers): add an extensible expert vision worker sdk
```

## Milestone 3：Artifact Conversion Registry

完成：

* Prompt Artifacts；
* Mask Artifacts；
* Conversion Registry；
* Path Finder；
* MaskToBBox；
* Coordinate Projection；
* tests。

提交：

```text
feat(workflow): compose expert models through typed artifact conversions
```

## Milestone 4：几何质量和错误分类

完成：

* Failure Class；
* Geometry Quality Report；
* Dry Run Summary；
* manual correction metrics；
* refiner metrics；
* API；
* tests。

提交：

```text
feat(evaluation): distinguish semantic, geometry and provider failures
```

## Milestone 5：现有模型统一迁移

完成：

* SAM；
* YOLO；
* RF-DETR；
* LocateAnything；
  -现有 HTTP Worker；
  -兼容旧 Workflow；
  -迁移测试。

提交：

```text
refactor(models): register existing vision backends through capabilities
```

## Milestone 6：Pipeline Builder Agent

完成：

-新的 Advisor system rules；
-模型和 Contract 工具；
-Conversion Path 工具；
-Geometry Quality 工具；
-错误诊断；
-合法 Draft 修改；
-多轮测试。

提交：

```text
feat(agent): build evidence-driven expert vision pipelines
```

## Milestone 7：Expert Model Setup UX

完成：

* Add Expert Model；
  -Preset；
  -Discovery；
  -Health；
  -Model Identity；
  -Sample Test；
  -Registration；
  -Labs 状态；
  -无障碍。

提交：

```text
feat(settings): guide expert vision model onboarding
```

## Milestone 8：RoboCup Workflow 和 Release

完成：

* RoboCup Ball capability binding；
* VLM 粗框 + SAM 条件式 refinement；
  -Specialist-first；
  -Open-vocabulary fallback；
  -Review；
  -Mock 演示；
  -真实 Worker smoke test，若环境可用；
  -文档；
  -Release 验收。

提交：

```text
test(release): validate expert vision pipeline alpha
```

---

# 二十五、Release Blocking Acceptance Matrix

## A. 扩展架构

* [ ] 新专家模型可通过 Manifest 注册。
* [ ] 新模型不需要修改 Core enum。
* [ ] 新模型不需要修改 Core Runtime 分支。
* [ ] Worker SDK 可以生成可运行模板。
* [ ] Worker Contract 有自动测试。
* [ ] Capability、输入和输出可发现。
* [ ] Model Availability 真实反映健康状态。

## B. 专家模型

* [ ] SAM 使用 Prompted Segmentation Capability。
* [ ] YOLO 使用 Object Detection Capability。
* [ ] RF-DETR 使用 Object Detection Capability。
* [ ] LocateAnything 使用 Open-vocabulary Detection Capability。
* [ ] PIDNet 可表示为 Semantic Segmentation Capability。
* [ ] Grounding DINO 可表示为 Open-vocabulary Detection Capability。
* [ ] 模型品牌不进入 Core Node。
* [ ] 没有权重的模型不显示 Available。

## C. SAM 合法流程

* [ ] DetectionSet 可以转换为 BoxPromptSet。
* [ ] BoxPromptSet 可以输入 Prompted Segmentation。
* [ ] SAM 输出 MaskSet。
* [ ] MaskSet 可以转换为 Bounding Box。
* [ ] 原框、Mask、细化框均可审计。
* [ ] SAM 不可用时 Agent 不加入执行 Draft。
* [ ] Provider 失败时 Agent 不误建议 SAM。
* [ ] No Candidate 时 Agent 不误建议 SAM。

## D. VLM 质量认知

* [ ] VLM 几何默认标记为 CoarseHypothesis。
* [ ] 语义 confidence 与 geometry quality 分离。
* [ ] Dry Run 记录 geometry metrics。
* [ ] 人工 bbox 调整形成几何质量证据。
* [ ] Agent 根据证据决定是否使用 Refiner。
* [ ] Agent 不仅依赖 Prompt 中的泛化判断。
* [ ] 失败类型被正确区分。

## E. Advisor

* [ ] Agent 检查 Model Availability。
* [ ] Agent 检查 Capability。
* [ ] Agent 检查输入输出 Contract。
* [ ] Agent 使用 Conversion Path。
* [ ] Agent 调用 Static Validation。
* [ ] Agent 调用 Dry Run。
* [ ] Agent读取 Geometry Quality。
* [ ] Agent根据结果修订 Draft。
* [ ] Agent不能选择 unavailable Model。
* [ ] Agent最终只提交人工审批。
* [ ] Agent不展示隐藏思维链。

## F. 产品

* [ ] Settings 可以添加 Expert Model。
* [ ] 可以发现 Worker 能力。
* [ ] 可以进行样本测试。
* [ ] 可以看到模型版本和权重状态。
* [ ] 可以看到 Score Semantics。
* [ ] 可以看到 Geometry Semantics。
* [ ] Advisor 解释为什么加入或不加入 SAM。
* [ ] Guided Mode 不暴露不必要的内部类型。
* [ ] Expert Mode 可以查看完整 Artifact Chain。

## G. RoboCup

* [ ] `robocup.ball` 不写死模型品牌。
* [ ] 白鞋误检优先使用语义验证而非 SAM。
* [ ] 粗框问题可以条件式使用 SAM。
* [ ] Specialist 可用时优先 Specialist。
* [ ] Open-vocabulary 可作为冷启动或 fallback。
* [ ] Generic Project 不加载 RoboCup。

---

# 二十六、文档

新增：

```text
docs/EXPERT_VISION_MODELS.md
docs/EXPERT_VISION_WORKER_SDK.md
docs/ARTIFACT_CONVERSIONS.md
docs/GEOMETRY_QUALITY.md
docs/ADVISOR_MODEL_SELECTION.md
docs/SAM_PIPELINE.md
docs/YOLO_BACKEND.md
docs/RFDETR_BACKEND.md
docs/LOCATE_ANYTHING_BACKEND.md
docs/PIDNET_BACKEND.md
docs/GROUNDING_DINO_BACKEND.md
docs/EXPERT_MODEL_ONBOARDING.md
```

更新：

```text
README.md
docs/DESIGN.md
docs/CORE_AND_SKILLS.md
docs/AGENT_LOOP.md
docs/GUIDED_EXPERIENCE.md
docs/ROBOCUP_SKILL.md
docs/KNOWN_LIMITATIONS.md
docs/COURSE_REQUIREMENTS.md
```

---

# 二十七、不得采用的假实现

禁止：

* 只修改 `advisor.md`；
* 只写“VLM 不精确”而没有质量指标；
* 把所有 VLM 结果无条件交给 SAM；
* Provider 调用失败时建议 SAM；
* 没有候选框时建议 SAM；
* 用 SAM 解决语义误检；
* 为每个模型增加 Core enum；
* 为每个模型增加顶层 Skill；
* 前端硬编码 SAM、YOLO、RF-DETR 流程；
* Worker 文件存在就显示 Available；
* 自动下载第三方权重；
* 提交模型权重；
* 伪造 confidence；
* 用 confidence 代表 bbox 精度；
* 只做 UI 卡片但无法执行；
* 用 Mock 冒充真实 Worker；
* 破坏现有 Batch、Replay、Review 和 Export；
* push；
* 修改 remote；
* 提交 API Key。

---

# 二十八、最终测试

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
cargo build --workspace --all-features
```

Web：

```bash
npm run typecheck
npm run test
npm run build
```

Python Worker：

```bash
python -m pytest
```

E2E 至少覆盖：

1. 注册 Generic Expert Worker；
2. 注册 SAM Worker；
3. SAM 无权重状态；
4. 注册 YOLO Worker；
5. 注册 RF-DETR Worker；
6. 注册 LocateAnything Worker；
7. Capability discovery；
8. Contract discovery；
9. VLM Provider failure 不建议 SAM；
10. VLM 粗框证据触发 SAM；
11. 白鞋风险触发分类验证；
12. LocateAnything 无分数使用 Evidence Decision；
13. RF-DETR Specialist-first；
14. 小目标触发 Tile；
15. Agent Draft validation；
16. Dry Run revision；
17. Human approval；
18. Generic Project 无 RoboCup；
19. 旧 Workflow migration；
20. Run、Review、Replay、Export 回归。

---

# 二十九、最终报告

最终报告必须包含：

## 1. 新的模型扩展方式

说明：

* Provider；
* Vision Worker；
* Model Profile；
* Capability；
* Manifest；
* Worker SDK。

## 2. 新增专家模型过程

以一个虚构的自定义模型为例，说明需要新增哪些文件，以及是否修改 Core。

## 3. SAM 流程

说明：

* 粗框；
* Prompt；
* Mask；
* Mask to BBox；
* Geometry Quality；
  -不可用回退。

## 4. VLM 准确性处理

说明：

* 为什么不把 confidence 当几何精度；
  -如何区分 Provider、Semantic 和 Geometry 错误；
  -如何收集 Dry Run 证据；
* Advisor 如何使用证据。

## 5. Advisor Agent

说明：

* Tools；
  -Conversion Path；
  -模型选择；
  -Static Validation；
  -Dry Run；
* Revision；
  -人工边界。

## 6. 模型接入状态

分别说明：

```text
SAM
YOLO
RF-DETR
LocateAnything
PIDNet
Grounding DINO
```

必须区分：

```text
Adapter
Worker
Weights
Health
Smoke test
Pipeline selectable
```

## 7. 测试

列出真实执行结果。

## 8. Milestone 提交

列出：

```text
commit hash
commit message
milestone
```

## 9. Live-conditional

说明真实 Worker、GPU、权重和外部 Provider 限制。

## 10. Git 状态

说明：

* 当前分支；
* 工作区；
  -领先远程提交数；
  -未 push；
  -remote 未修改。

---

# 三十、启动指令

将本文保存为：

```text
docs/execution/EXPERT_VISION_MASTER_PROMPT.md
```

然后从仓库根目录启动 Codex，并输入：

```text
阅读 docs/execution/EXPERT_VISION_MASTER_PROMPT.md，并将其作为本次长期任务的最高目标。

先核验当前 SAM、YOLO、RF-DETR、LocateAnything、HTTP Vision Protocol、Model Registry、Advisor、Artifact、Dry Run 和测试，不要盲信文件存在即代表能力可用。

本次重点不是继续给每个模型增加专用分支，而是：

1. 建立统一 Expert Model Manifest；
2. 建立 Expert Vision Worker SDK；
3. 通过 Capability 和 Artifact Contract 接入模型；
4. 补齐 Detection → Prompted Segmentation → Mask → BBox 的公开节点链；
5. 建立 Artifact Conversion Registry；
6. 区分 Provider Failure、No Candidate、Semantic Error 和 Geometry Error；
7. 把 VLM 框标记为未经标定的粗几何假设；
8. 使用 Dry Run 和人工修正数据评估几何质量；
9. 让 Pipeline Builder Agent 根据真实证据选择专家模型；
10. 让 Agent 明确解释为什么加入或不加入 SAM；
11. 保持 robocup.ball 只依赖 Capability；
12. 保持 Batch、Artifact、Replay、Review 和 Export 不回归。

从 Milestone 0 开始持续执行。

普通技术选择自行决定，并记录到 EXPERT_VISION_DECISIONS.md。

每完成一个 Milestone：
1. 更新状态和验收证据；
2. 执行对应 Rust、Web、Worker 和 E2E 测试；
3. 修复回归；
4. 创建独立本地提交；
5. 继续下一 Milestone。

真实 Worker 没有权重或 GPU 时：
- 完成 Manifest；
-完成协议；
-完成 Mock；
-完成 Contract 测试；
-完成 Pipeline；
-完成 UI；
-完成 Agent 测试；
-将真实推理标记为 live-conditional。

不得用 Mock 冒充真实模型。

不要 push。
不要修改 Git remote。
不要下载或提交模型权重。
不要使用、恢复或提交任何对话中出现过的 API Key。
不要执行 reset、rebase、amend 或破坏性 checkout。

只有 Release Blocking Acceptance Matrix 全部满足，或只剩明确记录的 live-conditional 项时，才输出最终报告。
```
