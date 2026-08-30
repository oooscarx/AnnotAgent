# AnnotAgent Open-Vocabulary + Specialist Detection Alpha

你现在是 AnnotAgent 仓库的长期主实现工程师。

本次任务是在现有 AnnotAgent Agent + Skill 架构和 Guided Experience 基础上，引入两类互补的视觉检测能力：

1. LocateAnything：开放词汇检测与短语定位能力；
2. RF-DETR：固定类别专用目标检测能力。

本任务不是简单增加两个模型按钮，也不是把两个 Python Demo 包装成 HTTP 接口后宣布完成。

最终必须形成：

```text
Project Label Schema
→ Agent 检查可用模型与能力
→ 选择冷启动、专用检测或混合策略
→ 执行 LocateAnything / RF-DETR
→ 生成标准 Detection Artifacts
→ 保留每个模型的独立证据
→ 必要时执行 Crop、分类、领域 Validator 和 Recovery
→ 自动接受、拒绝或进入人工 Review
→ 保存完整 lineage、成本、耗时和版本
```

产品名称始终是：

```text
AnnotAgent
```

LocateAnything、RF-DETR、YOLO 和 Qwen 都只是可注册的 Model Backend。

它们不是 AnnotAgent 产品本身，也不应该成为 Core 中的特殊分支。

---

# 一、本次版本目标

本次版本名称：

```text
AnnotAgent Open-Vocabulary + Specialist Detection Alpha
```

本次要建立的完整能力链是：

```text
无专用训练数据
→ LocateAnything 开放词汇定位
→ 人工复核形成可信标注
→ 导出训练数据
→ 注册 RF-DETR 专用模型
→ RF-DETR 承担低成本批量检测
→ 低分、空结果、冲突和领域风险触发 LocateAnything fallback
→ Agent 根据新证据决定接受、拒绝或 Review
```

本轮 Release Blocking 范围包括：

* 通用 Open-vocabulary Grounding Capability；
* 通用 Object Detection Capability；
* LocateAnything HTTP Backend；
* RF-DETR HTTP Backend；
* Detection Artifact 标准化；
* 可选置信度与分数语义；
  -候选匹配与多模型证据；
* Evidence Gate；
* Agent 动态 fallback；
* Label Pipeline 模板；
* Run Detail 中的模型证据和 lineage；
* Mock、协议、Runtime、Web 和 TUI 测试；
* 小规模真实模型 smoke test，外部环境不可用时标记为 live-conditional。

本轮不要求：

* 在 Rust 主进程中直接加载大型 Python 模型；
* 自动下载模型权重；
* 实现完整 RF-DETR 训练平台；
* 自动调参；
* 分布式 GPU 调度；
* 云端模型市场；
* LocateAnything 视觉样例提示微调；
* 将所有 OCR、GUI grounding 和文档定位能力一次性加入产品；
* 将模型权重提交到 Git。

---

# 二、先检查现有仓库

开始时必须执行：

```bash
git status --short --branch
git log --oneline -20
```

然后核验：

* 当前分支；
* 工作区是否干净；
* 当前领先远程多少提交；
* 当前 Core、Runtime、Application、Server、Web 和 TUI；
* 当前 Skill Registry；
* 当前 Model Registry；
* 当前 VLM Detection Skill；
* 当前 YOLO Detection Skill；
* 当前 Classification Skill；
* 当前 `robocup.ball` Domain Skill；
* 当前 Artifact 数据模型；
* 当前 Workflow 和 Label Pipeline；
* 当前 Guided Project Experience；
* 当前 Run Detail、Review 和 Artifact Inspector；
* 当前 HTTP Vision Backend；
* 当前 Tool Call 协议；
* 当前测试数量和测试结果；
* 当前 Known Limitations；
* 当前长程状态文档。

必须阅读：

```text
README.md
docs/DESIGN.md
docs/PRODUCT_HIERARCHY.md
docs/CORE_AND_SKILLS.md
docs/KNOWN_LIMITATIONS.md
docs/execution/
crates/annotagent-core/
crates/annotagent-runtime/
crates/annotagent-application/
crates/annotagent-provider/
crates/annotagent-server/
crates/annotagent-storage/
crates/annotagent-skill-robocup/
web/src/
apps/annotagent/src/
```

不要盲信已有“完成”说明。

必须通过代码、测试、API 和浏览器行为确认。

禁止：

```text
git reset
git rebase
git commit --amend
破坏性 git checkout
修改 Git remote
push
提交模型权重
提交 API Key
使用或恢复任何对话中出现过的 API Key
```

---

# 三、维护长期执行状态

创建并持续维护：

```text
docs/execution/DETECTION_BACKENDS_MASTER_PLAN.md
docs/execution/DETECTION_BACKENDS_STATUS.md
docs/execution/DETECTION_BACKENDS_DECISIONS.md
docs/execution/DETECTION_BACKENDS_ACCEPTANCE.md
docs/execution/DETECTION_BACKENDS_BLOCKERS.md
docs/execution/DETECTION_BACKENDS_KNOWN_LIMITATIONS.md
```

`DETECTION_BACKENDS_STATUS.md` 必须包含：

```text
当前 Milestone
已完成
正在进行
下一步
最近 Rust 测试
最近 Web 测试
最近 Worker 协议测试
最近浏览器验证
最近本地提交
Release Blocking 剩余项
Live-conditional 项
真实 Blocker
```

每完成一个 Milestone：

1. 更新状态；
2. 更新验收证据；
3. 执行对应测试；
4. 修复回归；
5. 创建独立本地提交；
6. 继续下一 Milestone；
7. 不等待用户确认。

---

# 四、核心架构原则

必须继续维持：

```text
AnnotAgent
├── Agent Core
├── Workflow Runtime
├── Core Nodes
├── Capability Skills
├── Domain Skills
├── Model Registry
├── Model Backends
├── Artifacts
├── Runs
└── Review
```

本次新增关系：

```text
Capability Skills
├── Open-vocabulary Grounding Skill
├── Object Detection Skill
├── Classification Skill
└── Future Prompted Segmentation Skill

Model Backends
├── LocateAnything Backend
├── RF-DETR Backend
├── YOLO Backend
├── OpenAI-compatible VLM Backend
└── Mock Backend
```

准确关系如下：

| 实体                              | 职责                                       |
| ------------------------------- | ---------------------------------------- |
| Open-vocabulary Grounding Skill | 定义开放词汇检测和短语定位的通用输入输出                     |
| Object Detection Skill          | 定义固定类别检测的通用输入输出                          |
| LocateAnything Backend          | 实现开放词汇检测能力                               |
| RF-DETR Backend                 | 实现固定类别检测能力                               |
| YOLO Backend                    | 实现固定类别检测能力                               |
| `robocup.ball`                  | 使用检测、分类、Crop、Review 等能力解决 RoboCup 足球领域问题 |
| Workflow Advisor Agent          | 根据项目和可用模型建议 Pipeline                     |
| Recovery Agent                  | 在低置信度、空结果、冲突或领域风险时选择 fallback            |

---

# 五、禁止模型名进入 Core 分支

禁止在 Core 或通用 Workflow Runtime 中出现：

```rust
NodeKind::LocateAnything
NodeKind::RfDetr
NodeKind::Yolo
```

禁止：

```rust
if model_id == "locate-anything" {
    // special logic
}
```

禁止：

```rust
if model_id.starts_with("rfdetr") {
    // special logic
}
```

禁止通用 Canvas、Run Detail 或 Workflow 编辑器硬编码：

```text
LocateAnything
RF-DETR
YOLO
football
RoboCup
```

通用 Runtime 只认识：

```rust
pub enum ModelCapability {
    VisionLanguage,
    OpenVocabularyDetection,
    PhraseGrounding,
    ObjectDetection,
    Classification,
    SemanticSegmentation,
    InstanceSegmentation,
    PromptedSegmentation,
    KeypointDetection,
}
```

模型名、品牌名和许可证信息来自 Model Registry。

---

# 六、Model Registry 数据模型

完善 Model Registry。

建议数据结构：

```rust
pub struct ModelDescriptor {
    pub id: ModelId,
    pub display_name: String,
    pub provider: String,
    pub backend: BackendDescriptor,
    pub capabilities: BTreeSet<ModelCapability>,
    pub version: ModelVersionMetadata,
    pub input_contract: ModelInputContract,
    pub output_contract: ModelOutputContract,
    pub score_semantics: ScoreSemantics,
    pub runtime_requirements: RuntimeRequirements,
    pub license: LicenseMetadata,
    pub status: ModelAvailabilityStatus,
}
```

模型版本：

```rust
pub struct ModelVersionMetadata {
    pub architecture: Option<String>,
    pub model_version: String,
    pub checkpoint_sha256: Option<String>,
    pub training_dataset_version: Option<String>,
    pub backend_protocol_version: String,
}
```

后端类型：

```rust
pub enum BackendKind {
    OpenAiCompatible,
    HttpVision,
    Mock,
    Onnx,
}
```

本轮至少真实实现：

```text
OpenAiCompatible
HttpVision
Mock
```

ONNX 可以保留类型和文档，但如果未实现，不得在 GUI 中显示为可用。

---

# 七、分数语义和可选置信度

不得假设所有检测模型都提供可比较的概率分数。

新增：

```rust
pub enum ScoreSemantics {
    CalibratedProbability,
    RelativeConfidence,
    RankingScore,
    NotProvided,
    Unknown,
}
```

```rust
pub struct DetectionScore {
    pub value: Option<f32>,
    pub semantics: ScoreSemantics,
}
```

要求：

* `value` 必须在有限范围内；
* NaN 和 Infinity 拒绝；
* 不提供分数时保存 `None`；
* 不允许为 LocateAnything 人工伪造 `0.98`；
* UI 对无分数结果显示：

  ```text
  Confidence not provided
  ```
* Gate 不能对 `None` 直接应用普通 confidence threshold；
* 无分数候选必须通过 Evidence Gate、领域 Validator、其他模型证据或 Human Review。

禁止：

```rust
confidence.unwrap_or(1.0)
```

禁止：

```rust
confidence.unwrap_or(0.5)
```

模型没有提供分数时，系统必须保留“不知道”，而不是通过一个默认浮点数把无知变成数学。

---

# 八、Detection Artifact 标准

统一所有检测 Backend 输出。

```rust
pub struct DetectionSetArtifact {
    pub detections: Vec<DetectionArtifactItem>,
}
```

```rust
pub struct DetectionArtifactItem {
    pub detection_id: DetectionId,
    pub query_id: Option<String>,
    pub model_label: Option<String>,
    pub project_label: Option<LabelId>,
    pub bbox: NormalizedRect,
    pub score: DetectionScore,
    pub source_model_id: ModelId,
    pub source_capability: ModelCapability,
    pub evidence: Vec<DetectionEvidence>,
}
```

单模型初始 evidence：

```rust
pub struct DetectionEvidence {
    pub source_model_id: ModelId,
    pub source_artifact_id: ArtifactId,
    pub bbox: NormalizedRect,
    pub score: DetectionScore,
    pub query_id: Option<String>,
    pub model_label: Option<String>,
    pub raw_output_ref: Option<StoredPayloadRef>,
}
```

要求：

* 坐标内部始终使用 `[0,1]`；
* 所有原始模型坐标由 Backend Adapter 转换；
* Core 不理解模型原始坐标格式；
* 每个候选保存来源模型；
* 每个候选保存 query 或模型类别；
* 模型类别与 Project Label 分离；
* 合法空结果使用空 `DetectionSetArtifact`；
* 空 DetectionSet 不等于模型失败；
* raw output 可以持久化为受控引用，但不能在普通日志中直接打印全部内容。

---

# 九、Open-vocabulary Grounding Capability Skill

新增或完善：

```text
annotagent.open_vocabulary_grounding
```

提供两项正式能力：

```text
OpenVocabularyDetection
PhraseGrounding
```

第一版不要扩展到：

```text
OCR
GUI element grounding
document layout
point localization
visual exemplar prompt
```

除非现有模型 Backend 明确报告支持，并且完成测试。

Skill 输入：

```rust
pub struct GroundingRequest {
    pub image: ImageArtifactRef,
    pub queries: Vec<GroundingQuery>,
    pub mode: GroundingMode,
    pub max_objects: Option<u32>,
}
```

```rust
pub struct GroundingQuery {
    pub id: String,
    pub text: String,
    pub target_label: Option<LabelId>,
}
```

```rust
pub enum GroundingMode {
    CategoryDetection,
    PhraseGrounding,
}
```

输出：

```text
DetectionSetArtifact
```

Skill 必须包含：

* Node Definition；
* JSON Schema；
* Capability requirements；
* Mock Backend；
* Workflow Template；
* 空结果处理；
* Query ID 到 Project Label 的映射；
* 测试；
  -文档。

---

# 十、LocateAnything Backend

实现：

```text
locate-anything-http
```

它是 `OpenVocabularyDetection` 和 `PhraseGrounding` 的 Model Backend。

第一阶段采用：

```text
Rust Runtime
→ Versioned HTTP Vision Protocol
→ Python LocateAnything Worker
→ GPU
```

不得把 Python、CUDA 和模型实现直接耦合到 Rust Core。

## 10.1 Worker 能力

Worker 至少提供：

```text
GET  /health
GET  /v1/capabilities
POST /v1/infer
```

`/v1/capabilities` 返回：

```json
{
  "protocol_version": "1",
  "model_id": "locate-anything-local",
  "capabilities": [
    "open_vocabulary_detection",
    "phrase_grounding"
  ],
  "score_semantics": "not_provided",
  "supports_visual_prompt": false,
  "supports_batch": true
}
```

具体能力必须从 Worker 真实实现报告，不得由前端硬编码。

## 10.2 请求协议

建议：

```json
{
  "protocol_version": "1",
  "request_id": "uuid",
  "operation": "open_vocabulary_detection",
  "model_id": "locate-anything-local",
  "queries": [
    {
      "id": "football",
      "text": "football used in a robot soccer match",
      "target_label": "football"
    }
  ],
  "options": {
    "max_objects": 20,
    "generation_mode": "hybrid"
  }
}
```

图片可以通过：

* `multipart/form-data`；
* 或受控 inline data。

不得向 Worker 提供任意本地文件路径。

不得在日志中打印：

* 完整图片；
* base64；
  -敏感 Header。

## 10.3 响应协议

Worker 必须将模型原始输出转换为标准响应：

```json
{
  "protocol_version": "1",
  "request_id": "uuid",
  "model_id": "locate-anything-local",
  "detections": [
    {
      "query_id": "football",
      "target_label": "football",
      "bbox_xyxy_normalized": [0.43, 0.35, 0.48, 0.41],
      "score": null,
      "score_semantics": "not_provided"
    }
  ],
  "usage": {
    "duration_ms": 1240,
    "device": "cuda"
  }
}
```

如果模型原始输出使用其他坐标范围：

* 由 Worker Adapter 转换到 `[0,1]`；
* Rust Runtime 不依赖具体范围；
* 转换逻辑必须有单元测试。

## 10.4 无目标

模型明确没有目标时：

```json
{
  "detections": []
}
```

Runtime 将其转换为合法空 DetectionSet。

不得转成：

```text
ProviderError
max_steps_or_no_submission
Failed
```

## 10.5 Visual Prompt

如果当前 Worker 不支持 visual exemplar prompt：

* Capability 明确为 false；
* GUI 不允许用户选用；
* 按钮 disabled；
* 显示具体原因；
* Workflow Static Validator 阻止使用该端口；
* 不得提供一个点完才报错的装饰性入口。

---

# 十一、Object Detection Capability Skill

新增或重构为通用：

```text
annotagent.object_detection
```

它不应叫：

```text
YOLO Skill
RF-DETR Skill
```

YOLO 和 RF-DETR 是该 Capability 的不同 Backend。

输入：

```rust
pub struct ObjectDetectionRequest {
    pub image: ImageArtifactRef,
    pub model_binding: ModelBindingId,
    pub target_labels: Vec<LabelId>,
    pub options: DetectionOptions,
}
```

配置：

```rust
pub struct DetectionOptions {
    pub confidence_threshold: Option<f32>,
    pub iou_threshold: Option<f32>,
    pub max_detections: Option<u32>,
    pub class_mapping: BTreeMap<String, LabelId>,
}
```

输出：

```text
DetectionSetArtifact
```

Skill 必须支持：

```text
Mock Backend
HTTP Vision Backend
```

YOLO 和 RF-DETR 共用同一 Capability Contract。

---

# 十二、RF-DETR Backend

实现：

```text
rfdetr-http
```

第一阶段采用：

```text
Rust Runtime
→ Versioned HTTP Vision Protocol
→ Python RF-DETR Worker
→ GPU
```

本轮只要求目标检测。

Instance segmentation、keypoint 和训练可以保留后续接口，但不得声称已完成。

## 12.1 Worker 能力

```json
{
  "protocol_version": "1",
  "model_id": "rfdetr-robocup-ball-v1",
  "capabilities": [
    "object_detection"
  ],
  "score_semantics": "relative_confidence",
  "label_space": [
    "football",
    "robot"
  ],
  "supports_batch": true
}
```

## 12.2 请求

```json
{
  "protocol_version": "1",
  "request_id": "uuid",
  "operation": "object_detection",
  "model_id": "rfdetr-robocup-ball-v1",
  "target_labels": [
    "football"
  ],
  "options": {
    "confidence_threshold": 0.25,
    "iou_threshold": 0.70,
    "max_detections": 100
  }
}
```

## 12.3 响应

```json
{
  "protocol_version": "1",
  "request_id": "uuid",
  "model_id": "rfdetr-robocup-ball-v1",
  "detections": [
    {
      "model_label": "football",
      "bbox_xyxy_normalized": [0.43, 0.35, 0.48, 0.41],
      "score": 0.87,
      "score_semantics": "relative_confidence"
    }
  ],
  "usage": {
    "duration_ms": 36,
    "device": "cuda"
  }
}
```

## 12.4 模型元数据

RF-DETR Model Descriptor 必须保存：

```text
architecture
checkpoint version
checkpoint SHA-256
training dataset version
label space
backend protocol version
runtime requirements
license metadata
```

示例：

```yaml
id: rfdetr-robocup-ball-v1
display_name: RF-DETR RoboCup Ball v1

capabilities:
  - object_detection

version:
  architecture: rfdetr-small
  model_version: "1"
  checkpoint_sha256: "..."
  training_dataset_version: robocup-ball-v3
  backend_protocol_version: "1"

label_space:
  - football
  - robot
```

不得仅保存：

```text
model_path: best.pt
```

`best.pt` 是文件名，不是版本管理。

---

# 十三、许可证元数据

Model Registry 必须支持：

```rust
pub struct LicenseMetadata {
    pub code_license: Option<String>,
    pub weight_license: Option<String>,
    pub source_url: Option<String>,
    pub commercial_use: LicensePermission,
    pub redistribution: LicensePermission,
    pub usage_notes: Vec<String>,
    pub verified_from_official_source: bool,
}
```

```rust
pub enum LicensePermission {
    Allowed,
    Restricted,
    Unknown,
}
```

实现时必须：

1. 阅读实际仓库和模型权重的官方 LICENSE；
2. 区分代码许可证与权重许可证；
3. 不依赖博客二手描述；
4. 无法确认时标记 Unknown；
5. 在 Settings → Models 中显示；
6. 在导出 Model Bundle 时包含；
7. 不将许可证提示写成法律结论。

禁止在没有核验时硬编码：

```text
commercial use allowed
```

或：

```text
commercial use forbidden
```

如果 LocateAnything 权重使用受限许可证，UI 应明确显示研究或评估限制。

如果 RF-DETR 不同尺寸或不同包许可证不同，必须按实际 Model Descriptor 保存，不能用一个全局字符串覆盖所有型号。

---

# 十四、多模型候选不能直接平均

新增标准的候选聚合数据模型。

```rust
pub struct CandidateCluster {
    pub id: CandidateClusterId,
    pub target_label: LabelId,
    pub representative_bbox: NormalizedRect,
    pub members: Vec<DetectionEvidence>,
    pub agreement: CandidateAgreement,
}
```

```rust
pub enum CandidateAgreement {
    SingleSource,
    MultiSourceAgreement {
        minimum_iou: f32,
        mean_iou: f32,
    },
    GeometryConflict,
    LabelConflict,
}
```

禁止：

```rust
merged_confidence =
    (rfdetr_confidence + locate_anything_confidence) / 2.0;
```

特别是 LocateAnything 没有 score 时，不能通过默认值参加平均。

应当保存：

```text
RF-DETR 提供 score 0.87
LocateAnything 提供同位置开放词汇证据
两者 IoU 0.76
```

而不是制造一个虚构的：

```text
merged confidence 0.94
```

---

# 十五、Candidate Match 节点

实现通用 Core Node：

```text
Match Detection Sets
```

输入：

```text
DetectionSet A
DetectionSet B
```

输出：

```text
CandidateClusterSet
```

最低支持：

* 相同 Project Label 匹配；
* IoU 匹配；
* 一对一匹配；
* 未匹配候选保留；
* Geometry conflict；
* Label conflict；
* 稳定顺序；
* 测试。

配置：

```yaml
method: iou
minimum_iou: 0.5
preserve_unmatched: true
```

不得把 LocateAnything 和 RF-DETR 逻辑写入该 Node。

---

# 十六、Evidence Gate

实现通用 Core Node：

```text
Evidence Gate
```

它不同于普通 Confidence Gate。

Confidence Gate 处理有可比较分数的单模型结果。

Evidence Gate 处理：

* 多模型一致；
* 单一模型结果；
* 无分数候选；
* 空结果；
  -模型冲突；
* Domain Validator issue；
  -历史纠错风险。

输入：

```rust
pub struct EvidenceGateInput {
    pub candidates: Vec<CandidateCluster>,
    pub validation_issues: Vec<ValidationIssue>,
    pub correction_risk: Option<CorrectionRisk>,
}
```

输出分支：

```text
accept
fallback
review
reject
```

配置例子：

```yaml
accept_when:
  - minimum_sources: 2
    minimum_iou: 0.60

  - source: specialist_detector
    minimum_score: 0.85
    no_domain_issue: true

fallback_when:
  - empty_specialist_result: true
  - specialist_score_below: 0.55
  - domain_issue: true

review_when:
  - geometry_conflict: true
  - open_vocab_only: true
  - score_missing: true
```

Gate 必须生成可解释原因：

```text
RF-DETR score below 0.55
LocateAnything fallback requested
```

或：

```text
Both detectors agree at IoU 0.74
Candidate accepted
```

---

# 十七、Agent 动态模型选择

固定执行：

```text
RF-DETR
→ LocateAnything
→ Qwen
```

仍然只是昂贵的固定 Workflow。

必须让 Workflow Advisor Agent 和 Recovery Agent动态决定是否调用后备模型。

## 17.1 Workflow Advisor Agent

Advisor 检查：

* Project Label；
* 是否已有专用 detector；
* 专用模型 label space；
* 模型 availability；
  -历史 Dry Run；
  -成本和延迟约束；
  -已启用 Domain Skill；
  -样本数量。

推荐规则不应全部硬编码，但可以提供确定性基础建议：

### 无专用模型

```text
LocateAnything
→ Crop verification
→ Review
```

### 有专用模型

```text
RF-DETR
→ Confidence / Evidence Gate
→ fallback LocateAnything
→ Review
```

### 成本优先

```text
RF-DETR first
→ 仅异常时调用 LocateAnything
```

### 准确率优先

```text
RF-DETR + LocateAnything on selected samples
→ Agreement check
→ Domain validation
```

Advisor 只能生成 Draft。

不得自动 Publish。

## 17.2 Recovery Agent

Recovery Agent 只有在以下条件下才能调用 LocateAnything fallback：

* RF-DETR 返回空结果；
* RF-DETR 分数低；
* Domain Validator 认为可能漏检；
* RF-DETR 与历史模式冲突；
  -用户配置允许；
  -剩余预算足够。

只有在以下条件下调用 Crop Classification：

* 两个 detector 冲突；
  -开放词汇结果没有分数；
  -领域 hard-negative 风险；
  -用户配置要求。

预算不足时：

```text
Human Review
```

不得继续无限调用模型。

---

# 十八、Artifact Cache 与共享执行

相同节点输入不得重复执行。

Cache Key 至少包含：

```text
input image SHA-256
model ID
model version
checkpoint SHA-256
node config hash
query text
Project Label mapping
backend protocol version
```

例如：

```text
三个 Label Pipeline 共享同一个 RF-DETR 节点
→ 每张图只执行一次
```

Replay 后：

```text
修改 Evidence Gate
→ 不重新执行 RF-DETR
→ 不重新执行 LocateAnything
```

修改 LocateAnything query 后：

```text
LocateAnything Cache 失效
RF-DETR Cache 继续有效
```

Run Detail 必须显示：

```text
Executed
Cache hit
Replayed
Skipped
```

---

# 十九、推荐 Workflow Templates

至少提供以下四个模板。

## 19.1 Open-vocabulary Cold Start

```text
Image
→ Open-vocabulary Detection
→ Filter target Label
→ Crop
→ Classification Verify
→ Evidence Gate
→ Commit / Review
```

适合：

```text
没有专用训练数据
```

## 19.2 Specialist Detection

```text
Image
→ Object Detection
→ Filter
→ Confidence Gate
→ Commit / Review
```

适合：

```text
已有专用 detector
```

## 19.3 Specialist with Open-vocabulary Fallback

```text
Image
→ RF-DETR
→ Evidence Gate
    ├── high confidence → Domain Validator → Commit
    ├── empty / low confidence → LocateAnything
    └── domain issue → LocateAnything
→ Candidate Match
→ Crop Verify
→ Commit / Review
```

## 19.4 Dual-model Audit

```text
Image
├── RF-DETR
└── LocateAnything
→ Candidate Match
→ Evidence Gate
→ Review disagreements
→ Commit agreements
```

适合：

```text
评估专用模型质量
发现潜在漏检
构建训练数据
```

模板只绑定 Capability，不绑定具体模型 ID。

Project 负责选择：

```text
object_detection → rfdetr-robocup-ball-v1
open_vocabulary_detection → locate-anything-local
classification → qwen3.7-flash
```

---

# 二十、RoboCup Ball Skill 的使用方式

`robocup.ball` 不得依赖具体模型品牌。

其 Capability requirements：

```yaml
requires:
  capabilities:
    - crop
    - human_review

optional_capabilities:
  - object_detection
  - open_vocabulary_detection
  - classification
  - field_segmentation
  - robot_detection
```

`robocup.ball` 只提供：

-足球 hard-negative Validator；

* Field Relation Validator；
* Recovery Policy；
* Correction Memory；
* Review reasons；
* RoboCup Ball Workflow Templates；
  -领域 Prompt Resource。

禁止在 `robocup.ball` 中写：

```rust
call_locate_anything()
call_rfdetr()
```

正确方式：

```rust
request_capability(ModelCapability::OpenVocabularyDetection)
```

由 Project Model Binding 解析到具体 Backend。

---

# 二十一、RoboCup 混合流程示例

创建或更新：

```text
examples/robocup-ball-hybrid/
```

Project 示例：

```yaml
version: 1

project:
  name: B-Human Football Hybrid Detection

skills:
  - annotagent.open_vocabulary_grounding
  - annotagent.object_detection
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
  specialist_detector:
    capability: object_detection
    model: rfdetr-robocup-ball-v1

  open_vocab_detector:
    capability: open_vocabulary_detection
    model: locate-anything-local

  crop_verifier:
    capability: classification
    model: qwen3.7-flash

workflow:
  template: robocup.ball.specialist_with_open_vocab_fallback
```

再提供完全离线：

```text
examples/robocup-ball-hybrid-mock/
```

Mock 必须覆盖：

1. RF-DETR 高分正确球；
2. RF-DETR 空结果，LocateAnything找到球；
3. RF-DETR 低分，LocateAnything一致；
4. RF-DETR 与 LocateAnything 几何冲突；
5. LocateAnything 单独产生白鞋误检；
6. `robocup.ball` 触发 Crop Verify；
7. 预算不足进入 Human Review。

---

# 二十二、Model Worker 管理

Settings → Models 中支持：

* 添加 HTTP Worker；
  -测试连接；
  -获取 capabilities；
  -查看模型版本；
  -查看 license；
  -查看 label space；
  -查看 score semantics；
  -查看设备；
  -查看最近延迟；
  -启用或禁用；
  -选择 Project 默认绑定。

模型状态：

```rust
pub enum ModelAvailabilityStatus {
    Available,
    Unreachable,
    Misconfigured,
    IncompatibleProtocol,
    MissingWeights,
    Disabled,
}
```

提供：

```text
Test connection
Refresh capabilities
View setup instructions
```

不得在 Rust Server 启动时自动下载几 GB 权重。

不得因为 Worker 未运行导致整个 AnnotAgent 无法启动。

---

# 二十三、Guided Experience 集成

现有 Guided Mode 必须保留。

用户不需要先知道 LocateAnything 和 RF-DETR 的技术差异。

## 23.1 New Project 推荐

当用户：

```text
要标 bounding box
没有专用 detector
```

显示：

```text
Recommended for getting started

Find objects by description
Uses an open-vocabulary model
No training data required
```

当用户已经配置 RF-DETR：

```text
Recommended for repeated labeling

Use your trained detector first
Ask an open-vocabulary model only when results are uncertain
```

## 23.2 Expert Mode

Expert Mode 显示真实模型：

```text
Specialist detector
RF-DETR RoboCup Ball v1

Fallback detector
LocateAnything 3B

Crop verifier
qwen3.7-flash
```

## 23.3 不暴露错误术语

默认模式使用：

```text
Find objects by description
Use trained detector
Check uncertain results
Compare detector results
```

Expert Mode 使用：

```text
OpenVocabularyDetection
ObjectDetection
EvidenceGate
CandidateClusterSet
```

---

# 二十四、Run Detail

Run Detail Results 模式显示：

```text
5 images
7 accepted detections
2 need review
1 open-vocabulary fallback
3 cache hits
0 failed
```

每个 bbox 显示：

```text
football
RF-DETR · 0.87
```

或者：

```text
football
LocateAnything · confidence not provided
```

若两个模型一致：

```text
football
2 models agree · IoU 0.74
```

Debug 模式显示：

```text
Source models
Individual bounding boxes
Individual scores
Score semantics
IoU match
Domain issues
Recovery actions
Cache status
Duration
Cost
Artifact lineage
```

不得只显示一个合并后无法追溯的框。

---

# 二十五、Review

Review Detail 必须显示为什么进入人工复核。

例子：

```text
Needs review

RF-DETR did not find a football.
LocateAnything found one candidate.
This model does not provide a confidence score.
```

或：

```text
Needs review

RF-DETR and LocateAnything disagree on location.
Bounding-box IoU: 0.18
```

或：

```text
Needs review

Candidate is close to the lower part of a robot box.
RoboCup Ball Skill marked it as a possible white shoe.
```

操作：

```text
Accept & next
Reject & next
Edit box
Use RF-DETR box
Use LocateAnything box
Merge manually
```

用户决策写入：

* Annotation Revision；
* Correction Memory；
  -模型证据；
* reviewer note。

---

# 二十六、TUI

TUI 主标题保持：

```text
AnnotAgent
Composable Annotation Agent Runtime
```

Models 面板显示：

```text
RF-DETR RoboCup Ball v1
object_detection
Available
36 ms

LocateAnything Local
open_vocabulary_detection
Available
1.24 s
No confidence score
```

Run Trace 明确显示：

```text
RF-DETR completed
Evidence Gate requested fallback
LocateAnything completed
Candidate Match found agreement
RoboCup Validator passed
Annotation committed
```

命令至少支持：

```text
/models
/models test <id>
/run
/pause
/resume
/cancel
/replay
/artifacts
/gui
```

---

# 二十七、HTTP Worker 安全

Worker Endpoint 默认只允许：

```text
http://127.0.0.1
http://localhost
```

远程 Worker 必须由用户显式启用。

必须防止：

* 任意 URL SSRF；
* 任意本地路径读取；
* Header 泄露；
* 图片 base64 日志；
* 无限请求；
* 超大响应；
* 缺少 timeout；
* Worker 返回恶意路径；
* Worker 返回 NaN；
* Worker 返回超界坐标；
* Worker 返回重复 detection ID；
* Worker 协议版本不兼容；
* Worker 假冒 capability。

所有 Worker 响应视为不可信输入。

---

# 二十八、错误模型

所有模型错误必须结构化。

```rust
pub enum VisionBackendError {
    Unreachable,
    Timeout,
    Cancelled,
    InvalidProtocol,
    UnsupportedOperation,
    InvalidCoordinates,
    InvalidScore,
    MissingWeights,
    OutOfMemory,
    ModelFailure,
    ResponseTooLarge,
}
```

错误必须记录：

```text
run id
image id
node id
model id
backend
operation
elapsed time
retry count
error code
recoverability
```

GUI 不得只显示：

```text
Run reached a terminal condition
```

应显示：

```text
LocateAnything worker did not respond within 120 seconds.

The RF-DETR result is still available.
You can retry the fallback step or send this result to Review.
```

---

# 二十九、预算策略

开放词汇模型通常比专用 detector 慢且昂贵。

Project 可以配置：

```toml
[detection_policy]
specialist_first = true
fallback_on_empty = true
fallback_below_score = 0.55
fallback_on_domain_issue = true
maximum_open_vocab_calls_per_image = 1
```

Agent 必须检查：

-剩余模型调用次数；
-剩余时间；
-剩余费用；
-当前风险；
-是否已有足够证据。

预算不足时：

```text
Human Review
```

不得：

```text
Budget exceeded → Failed
```

高风险候选在预算不足时应进入 Review。

---

# 三十、RF-DETR 训练闭环的边界

本轮不实现完整训练平台。

但必须为未来闭环准备明确数据模型：

```text
Accepted Annotations
→ Export COCO
→ External Training Job
→ Register New Model Version
→ Bind to Project
→ Dry Run
→ Activate
```

可以新增：

```rust
pub struct TrainingDatasetReference {
    pub project_id: ProjectId,
    pub export_id: ExportId,
    pub schema_hash: String,
    pub annotation_revision: String,
}
```

可以在文档中定义 Model Registration 流程。

不得在 Annotation Workflow 中自动启动训练。

训练是独立 Job，不是单图片标注节点。

---

# 三十一、Milestone 计划

## Milestone 0：基线与设计账本

完成：

* 仓库核验；
* 现有 Detection Skill 分析；
* 现有 Worker 协议分析；
* 现有 Artifact 分析；
* License 核验计划；
* 状态文档；
  -验收矩阵初稿。

提交：

```text
docs: establish mixed detection backend baseline
```

## Milestone 1：Capability 和 Model Registry 重构

完成：

* `OpenVocabularyDetection`；
* `PhraseGrounding`；
* `ObjectDetection`；
* Model Descriptor；
* Score Semantics；
* License Metadata；
* Availability；
* Capability validation；
* migration；
  -测试。

提交：

```text
feat(core): model open-vocabulary and specialist detection capabilities
```

## Milestone 2：Detection Artifact 和 Evidence

完成：

* Optional score；
* Score semantics；
* Detection Evidence；
* Candidate Cluster；
* Parent lineage；
* Storage migration；
* API DTO；
  -序列化测试。

提交：

```text
feat(core): preserve detection evidence and score semantics
```

## Milestone 3：通用 HTTP Vision Protocol

完成：

* protocol version；
* health；
* capabilities；
* infer；
* timeout；
* cancel；
* malformed response；
* coordinate validation；
* Worker contract tests；
  -安全边界。

提交：

```text
feat(provider): add versioned detection worker protocol
```

## Milestone 4：LocateAnything Backend

完成：

* Python Worker；
* Rust Adapter；
* Open-vocabulary detection；
* Phrase grounding；
* no-object；
* optional score；
* capability discovery；
* Mock；
* contract tests；
  -设置页面；
  -文档。

提交：

```text
feat(models): integrate locateanything grounding backend
```

## Milestone 5：RF-DETR Backend

完成：

* Python Worker；
* Rust Adapter；
* detection；
* class mapping；
* score；
* checkpoint metadata；
* label space；
* Mock；
* contract tests；
  -设置页面；
  -文档。

提交：

```text
feat(models): integrate rfdetr detection backend
```

## Milestone 6：Candidate Match 和 Evidence Gate

完成：

* IoU matching；
* unmatched candidates；
* conflicts；
* Evidence Gate；
  -可解释决策；
  -测试；
* GUI 展示。

提交：

```text
feat(runtime): combine detector evidence without fabricating scores
```

## Milestone 7：Advisor 和 Recovery Agent

完成：

* specialist-first 建议；
* cold-start 建议；
* fallback selection；
  -预算；
* Stop conditions；
* Agent Trace；
* Mock 多轮测试。

提交：

```text
feat(agent): select open-vocabulary fallbacks from detection evidence
```

## Milestone 8：RoboCup Ball Hybrid Template

完成：

* `robocup.ball` capability binding；
* specialist + fallback template；
* hard-negative；
* correction memory；
* field relation；
* Mock 示例；
  -真实项目配置。

提交：

```text
feat(robocup): add specialist and open-vocabulary ball workflow
```

## Milestone 9：Guided UX、Run 和 Review

完成：

* New Project 推荐；
* Model setup；
* Pipeline Recipe；
* Results summary；
* Evidence Inspector；
* Review explanations；
* TUI；
* URL restore；
  -无障碍。

提交：

```text
feat(ui): explain mixed detector evidence and fallbacks
```

## Milestone 10：可靠性和 Release 验收

完成：

* 100 张 Mock batch；
* Pause、Resume、Cancel；
  -缓存；
* Replay；
* Worker crash；
* timeout；
* invalid coordinates；
* License display；
  -真实 smoke test；
  -文档；
  -演示。

提交：

```text
test(release): validate open-vocabulary and specialist detection alpha
```

---

# 三十二、Release Blocking Acceptance Matrix

以下全部满足后，才能声称 Alpha 完成。

## A. 架构

* [ ] LocateAnything 不是 Core Node 类型。
* [ ] RF-DETR 不是 Core Node 类型。
* [ ] YOLO 和 RF-DETR 共用 Object Detection Capability。
* [ ] LocateAnything 实现 Open-vocabulary Capability。
* [ ] `robocup.ball` 不引用具体模型 ID。
* [ ] Generic Project 可以使用 LocateAnything。
* [ ] Generic Project 可以使用 RF-DETR。
* [ ] Core 中不存在模型品牌分支。

## B. Artifact

* [ ] Detection score 支持 `None`。
* [ ] Score semantics 被保存。
* [ ] 不伪造 LocateAnything confidence。
* [ ] 每个候选保存来源模型。
* [ ] 每个候选保存 query 或 model label。
* [ ] 多模型候选保留独立 evidence。
* [ ] Artifact lineage 可追踪。
* [ ] 合法空结果不是失败。

## C. LocateAnything

* [ ] Worker health 可检查。
* [ ] Worker capabilities 可发现。
* [ ] 支持开放词汇检测。
* [ ] 支持短语定位。
* [ ] 支持多个 query。
* [ ] 支持 no-object。
* [ ] 坐标正确归一化。
* [ ] 无 score 时保存 `None`。
* [ ] 不支持的 Visual Prompt 被阻止。
* [ ] timeout 和 cancel 可用。
* [ ] 模型许可证元数据可见。

## D. RF-DETR

* [ ] Worker health 可检查。
* [ ] Worker capabilities 可发现。
* [ ] 支持 object detection。
* [ ] 支持 label space。
* [ ] 支持 class mapping。
* [ ] 支持真实 score。
* [ ] 支持 confidence threshold。
* [ ] 保存 checkpoint hash。
* [ ] 保存训练数据版本。
* [ ] timeout 和 cancel 可用。
* [ ] 模型许可证元数据可见。

## E. 多模型证据

* [ ] 支持 IoU 匹配。
* [ ] 支持未匹配候选。
* [ ] 支持 geometry conflict。
* [ ] 支持 label conflict。
* [ ] 不平均不可比较分数。
* [ ] Evidence Gate 给出可解释原因。
* [ ] RF-DETR 高分可以不调用 LocateAnything。
* [ ] RF-DETR 空结果可以触发 LocateAnything。
* [ ] Domain risk 可以触发 LocateAnything。
* [ ] 预算不足进入 Review。

## F. Agent

* [ ] Advisor 能建议 cold-start Pipeline。
* [ ] Advisor 能建议 specialist-first Pipeline。
* [ ] Advisor 只能生成 Draft。
* [ ] Recovery Agent 根据证据选择 fallback。
* [ ] Recovery Agent 根据 fallback 结果改变决定。
* [ ] Agent 有 Tool、预算和停止条件。
* [ ] Trace 显示为什么调用后备模型。
* [ ] Trace 不展示隐藏思维链。

## G. Cache 和 Replay

* [ ] 相同 RF-DETR 输入只执行一次。
* [ ] 相同 LocateAnything query 只执行一次。
* [ ] 修改 Gate 不重新执行 detector。
* [ ] 修改 query 只使 LocateAnything Cache 失效。
* [ ] Cache Key 包含模型版本和配置。
* [ ] Replay 保留 lineage。
* [ ] 不产生重复 Annotation commit。

## H. 产品

* [ ] 全局品牌仍为 AnnotAgent。
* [ ] Guided Mode 使用用户语言。
* [ ] Expert Mode 显示真实模型和证据。
* [ ] Run Results 显示来源模型。
* [ ] 无分数结果显示“未提供置信度”。
* [ ] 多模型一致显示 agreement。
* [ ] Review 显示进入原因。
* [ ] Review 可以选择某个模型的框。
* [ ] Settings 可以测试 Worker。
* [ ] Worker 不可用不阻止 AnnotAgent 启动。

## I. RoboCup

* [ ] `robocup.ball` 使用 Capability Binding。
* [ ] 白鞋风险可以触发 fallback 或 Crop Verify。
* [ ] 点球点风险不会直接 auto-accept。
* [ ] Correction Memory 影响 Recovery。
* [ ] Generic Project 不加载 RoboCup。
* [ ] RoboCup 只在启用 Skill 的项目中出现。

## J. 作业要求

* [ ] Agent Loop 由 Rust Runtime 实现。
* [ ] TUI 可查看和取消。
* [ ] GUI 可查看和取消。
* [ ] Model、endpoint 和费用可配置。
* [ ] 实时进度可见。
* [ ] Run 历史和 Artifact 可查看。
* [ ] 每次模型调用记录用量和耗时。
* [ ] RoboCup 场景定制真实存在。
* [ ] Mock 演示无需 Key。
* [ ] Live smoke test 真实执行或明确标记 live-conditional。

---

# 三十三、必须完成的 Mock 场景

至少提供以下确定性测试。

## Case 1：专用 detector 高置信度

```text
RF-DETR 发现 football，score 0.92
→ Domain Validator 通过
→ 不调用 LocateAnything
→ Commit
```

## Case 2：专用 detector 空结果

```text
RF-DETR 返回空
→ Evidence Gate 请求 fallback
→ LocateAnything 找到候选
→ Crop Verify
→ Commit 或 Review
```

## Case 3：两个模型一致

```text
RF-DETR bbox
+ LocateAnything bbox
→ IoU 0.76
→ Multi-source agreement
→ Commit
```

## Case 4：两个模型冲突

```text
RF-DETR bbox
+ LocateAnything bbox
→ IoU 0.12
→ Geometry conflict
→ Recovery Agent
→ Crop Verify
→ Review
```

## Case 5：LocateAnything 白鞋误检

```text
LocateAnything 候选
→ 无 confidence
→ 位于 robot 下部
→ RoboCup PossibleWhiteShoe
→ Crop Classification
→ Reject
```

## Case 6：预算不足

```text
RF-DETR 低分
→ 应调用 LocateAnything
→ 预算不足
→ Human Review
→ Run 不失败
```

## Case 7：Worker 崩溃

```text
RF-DETR Worker 中断
→ 结构化错误
→ Agent 判断是否使用 LocateAnything
→ Run Partial 或 Review
→ 不 panic
```

---

# 三十四、真实 Smoke Test

在存在合法环境和权重时执行。

## LocateAnything

至少 5 张图片：

* 有目标；
* 无目标；
* 多目标；
* 小目标；
* hard negative。

记录：

```text
成功率
平均耗时
输出框数量
空结果
错误
显存
Worker 启动方式
```

## RF-DETR

至少 5 张图片：

* 有目标；
* 无目标；
* 多目标；
* 低分目标；
  -小目标。

记录：

```text
模型版本
checkpoint hash
平均耗时
分数
输出框数量
错误
显存
```

## 混合流程

至少执行：

```text
RF-DETR first
→ LocateAnything fallback
→ Candidate Match
→ RoboCup Validator
→ Review / Commit
```

若缺少：

* GPU；
  -模型权重；
  -网络；
  -合法许可证；
  -依赖环境；

则：

* 精确记录 Blocker；
* 标记为 `live-conditional`；
* 不伪造结果；
* 继续完成 Mock、协议、Runtime、UI 和文档；
* 不将 live test 伪装成 Release 通过。

---

# 三十五、自动测试

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

Web：

```bash
npm run typecheck
npm run test
npm run build
```

Python Worker 根据仓库环境执行：

```bash
python -m pytest
```

或等价测试命令。

必须增加：

* Worker contract tests；
* coordinate conversion tests；
* score semantics tests；
* missing score tests；
* no-object tests；
* malformed response tests；
* timeout tests；
* cancel tests；
* oversized response tests；
* Candidate Match tests；
* Evidence Gate tests；
* Agent fallback tests；
* Cache tests；
* Replay tests；
* License metadata tests；
* GUI browser tests。

---

# 三十六、浏览器验收

必须测试：

1. Settings 注册 LocateAnything Worker；
2. 显示 capabilities；
3. 显示无 confidence；
4. Settings 注册 RF-DETR Worker；
5. 显示 label space；
6. 显示 checkpoint version；
7. New Project 推荐 cold-start；
8. New Project 推荐 specialist-first；
9. Pipeline 展示 fallback；
10. Dry Run 显示 fallback 数量；
11. Run Results 显示来源模型；
12. Debug 显示两组 evidence；
13. Review 显示冲突原因；
14. 选择 RF-DETR box；
15. 选择 LocateAnything box；
16. Worker 离线错误；
17. Worker timeout；
18. Generic Project 不出现 RoboCup；
19. 1024px 无横向溢出；
20. 200% Zoom 核心路径可用。

---

# 三十七、文档

新增：

```text
docs/OPEN_VOCABULARY_DETECTION.md
docs/SPECIALIST_DETECTION.md
docs/DETECTION_EVIDENCE.md
docs/HTTP_VISION_PROTOCOL.md
docs/MODEL_LICENSE_METADATA.md
docs/LOCATE_ANYTHING_BACKEND.md
docs/RFDETR_BACKEND.md
docs/HYBRID_DETECTION_WORKFLOWS.md
docs/DEMO_HYBRID_DETECTION.md
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

README 不要把项目描述成：

```text
LocateAnything UI
RF-DETR GUI
```

正确表述：

> AnnotAgent can combine open-vocabulary models, specialist detectors, domain validators, and human review into versioned annotation pipelines.

---

# 三十八、课程演示

创建：

```text
docs/DEMO_HYBRID_DETECTION.md
```

5 分钟演示：

```text
0:00–0:30
问题：冷启动时没有专用训练数据，积累数据后又不应该一直使用昂贵 VLM。

0:30–1:00
架构：AnnotAgent Agent + Capability Skills + Model Backends + RoboCup Domain Skill。

1:00–1:30
LocateAnything 开放词汇冷启动，不需要目标类别训练权重。

1:30–2:00
RF-DETR 专用 detector 低成本运行。

2:00–2:40
RF-DETR 空结果，Recovery Agent 自动调用 LocateAnything fallback。

2:40–3:20
两个模型产生冲突，Candidate Match 和 Evidence Gate 保留独立证据。

3:20–4:00
RoboCup Ball Validator 发现白鞋风险，进入 Crop Verify 和 Review。

4:00–4:30
人工选择正确框并写入 Correction Memory。

4:30–4:50
展示 Artifact lineage、Replay、耗时和费用。

4:50–5:00
说明 Generic Project 不依赖 RoboCup，也可使用同一检测能力。
```

---

# 三十九、不得采用的假实现

禁止：

* 在 Core 中增加 `LocateAnythingNode`；
* 在 Core 中增加 `RfDetrNode`；
* 把模型品牌当成 Skill；
* 把模型名称写死在 `robocup.ball`；
* 为 LocateAnything 伪造 confidence；
* 把两个模型分数简单平均；
* 每张图无条件调用所有模型；
* 把 fallback 写成固定死链；
* 忽略 Agent 预算；
* Worker 未启动时让整个 Server 崩溃；
* 自动下载大模型；
* 提交权重；
* 只在 UI 中展示模型卡片但 Runtime 不可执行；
* 用 Mock 结果冒充真实模型；
* 不核验许可证就声称商业可用；
* 把 Python Worker 错误吞成通用 Failed；
* 让 VLM 重新抄写 detector bbox；
* 为了接模型破坏 Guided Experience；
* 恢复 RoboCup 为全局品牌；
* push；
* 修改 remote；
* 提交 API Key。

---

# 四十、最终报告格式

最终报告必须包含：

## 1. 实际架构

说明：

* Capability Skill；
* Model Backend；
* Domain Skill；
* Core Node；
* Agent 的关系。

## 2. LocateAnything

说明：

* 实际支持的操作；
* Worker；
  -坐标转换；
  -无目标；
* score 语义；
  -不支持的能力；
  -许可证；
  -测试。

## 3. RF-DETR

说明：

* 实际支持的操作；
* Worker；
* label space；
* class mapping；
* checkpoint metadata；
  -分数；
  -许可证；
  -测试。

## 4. Detection Evidence

说明：

* Detection Artifact；
* Candidate Cluster；
* IoU matching；
  -冲突；
  -为什么没有平均分数。

## 5. Agent 行为

说明：

* Advisor 如何推荐 Pipeline；
* Recovery 何时调用 fallback；
  -预算；
  -停止条件；
  -人工边界。

## 6. RoboCup Skill

说明：

* 如何通过 Capability 使用模型；
  -如何处理白鞋、点球点和领域风险；
  -为什么没有写死模型名。

## 7. Guided UX

说明：

-用户如何配置模型；
-用户如何选择冷启动或专用 detector；
-Run 如何展示证据；
-Review 如何展示冲突。

## 8. 测试

列出实际执行命令和真实结果。

不得把未执行测试报告为通过。

## 9. Live-conditional

分别说明：

* LocateAnything；
* RF-DETR；
* GPU；
  -权重；
  -许可证；
  -浏览器人工测试。

## 10. Milestone 提交

列出：

```text
commit hash
commit message
milestone
```

## 11. 未完成内容

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

## 12. Git 状态

说明：

-当前分支；
-工作区是否干净；
-领先远程提交数；
-未 push；
-remote 未修改。

---

# 四十一、启动指令

将本文保存为：

```text
docs/execution/DETECTION_BACKENDS_MASTER_PROMPT.md
```

然后从仓库根目录启动 Codex，并输入：

```text
阅读 docs/execution/DETECTION_BACKENDS_MASTER_PROMPT.md，并将其作为本次长程任务的最高目标。

先核验 Git、当前代码、Model Registry、Skill Registry、Artifact、Workflow、HTTP Backend、Web、TUI 和测试，不要盲信已有完成说明。

从 Milestone 0 开始持续执行。

普通架构和实现决策自行决定，并记录到 DETECTION_BACKENDS_DECISIONS.md。

每完成一个 Milestone：
1. 更新状态和验收证据；
2. 执行对应 Rust、Web、Worker 和浏览器测试；
3. 修复回归；
4. 创建独立本地提交；
5. 继续下一 Milestone。

重点保证：

1. LocateAnything 是 Open-vocabulary Detection Backend；
2. RF-DETR 是 Object Detection Backend；
3. 模型品牌不进入 Core Node 类型；
4. Detection score 可以缺失；
5. 不伪造或平均不可比较的 confidence；
6. 多模型结果保留独立 evidence；
7. Recovery Agent 只在必要时调用 fallback；
8. robocup.ball 依赖 capability，不依赖具体模型；
9. Guided Mode 仍然面向用户任务；
10. Runtime、Artifact、Replay、预算和历史真实可用。

外部模型暂时无法运行时，继续完成：
- Mock；
-协议；
-Runtime；
-Artifact；
-UI；
-测试；
-文档。

将真实模型项精确标记为 live-conditional，不得伪造。

不要 push。
不要修改 Git remote。
不要下载或提交模型权重，除非仓库已有明确受控机制且权重位于 Git 忽略目录。
不要使用、恢复或提交任何对话中出现过的 API Key。
不要执行 reset、rebase、amend 或破坏性 checkout。

只有 Release Blocking Acceptance Matrix 全部满足，或只剩明确记录的外部 live-conditional 项时，才输出最终报告。
```
