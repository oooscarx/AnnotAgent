# AnnotAgent Geometry-Safe Pipeline Builder + Self-Improvement Alpha

你现在是 AnnotAgent 仓库的长期主实现工程师。

本次任务要解决一个已经通过真实 RoboCup 足球标注暴露出来的问题：

```text
VLM 能正确找到足球，
语义置信度达到 0.98～0.99，
但 bbox 可能偏移、偏松或不贴边。

当前 Pipeline 却把该语义置信度直接送入 Confidence Gate，
导致未经几何验证的 bbox 被自动 Commit。
```

本次任务不是简单地：

* 修改 Advisor Prompt；
* 把 SAM 无条件加进所有 VLM Pipeline；
* 提高 Review 阈值；
* 把一个 `confidence` 拆成四个凭空生成的小数；
* 手工替当前 Project 修改 Workflow。

最终必须让 **AnnotAgent 自己**做到：

```text
首次生成 Pipeline
→ 知道 VLM bbox 尚未经过几何校准
→ 生成保守且可执行的安全流程
→ 根据可用模型决定是否加入几何 Refiner
→ Dry Run 收集几何质量证据
→ 人工修框形成结构化反馈
→ 诊断问题属于语义、漏检还是几何误差
→ 生成现有 Pipeline 的修订 Draft
→ Before / After Dry Run
→ 只有证据显示改进时才推荐新版本
→ 人工批准后发布不可变版本
```

核心原则：

```text
Prompt 负责帮助 LLM 选择合理方案；
Model/Operation Metadata 负责表达能力边界；
Rust Static Validator 负责阻止不安全流程；
Dry Run 和人工修正负责产生证据；
人类负责最终发布。
```

产品名称始终是：

```text
AnnotAgent
```

RoboCup 仍然只是 Domain Skill 和应用案例。

---

# 一、开始前核验仓库

首先执行：

```bash
git status --short --branch
git log --oneline -20
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
skills/robocup/
examples/
workspace/
```

重点核验：

* 当前正在使用的 RoboCup Ball Project；
* 当前默认 Published Workflow；
* 当前 Qwen Model Profile；
* 当前 VLM Detection 节点；
* 当前 Score Semantics；
* 当前 Geometry Semantics；
* 当前 Confidence Gate；
* 当前 Hard-negative Validator；
* 当前 SAM Adapter；
* 当前 Prompted Segmentation Capability；
* 当前 Mask-to-BBox 转换；
* 当前 Review Revision；
* 当前 Correction Memory；
* 当前 Dry Run Summary；
* 当前 Advisor Prompt；
* 当前 Pipeline Builder Agent Tools；
* 当前 Static Validator；
* 当前 Workflow Version 兼容和迁移机制。

不得仅因为某个 `.rs` 文件存在就声称能力可用。

必须区分：

```text
类型已定义
Adapter 已实现
节点已注册
Model Profile 已配置
Worker 健康
权重可用
Smoke Test 通过
Advisor 可以选择
Pipeline 可以发布
```

这些是不同状态。

禁止：

```text
git reset
git rebase
git commit --amend
破坏性 git checkout
修改 Git remote
push
使用或恢复对话中出现过的 API Key
提交模型权重
用 Mock 结果冒充真实 SAM、Qwen 或检测器
```

---

# 二、建立长期执行账本

创建并持续维护：

```text
docs/execution/GEOMETRY_SAFETY_MASTER_PLAN.md
docs/execution/GEOMETRY_SAFETY_STATUS.md
docs/execution/GEOMETRY_SAFETY_DECISIONS.md
docs/execution/GEOMETRY_SAFETY_ACCEPTANCE.md
docs/execution/GEOMETRY_SAFETY_BLOCKERS.md
docs/execution/GEOMETRY_SAFETY_KNOWN_LIMITATIONS.md
```

`GEOMETRY_SAFETY_STATUS.md` 必须记录：

```text
当前 Milestone
已完成
正在进行
下一步
最近 Rust 测试
最近 Web 测试
最近 E2E
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

# 三、首先复现当前错误

先建立一个回归 Fixture，稳定复现：

```text
Image
→ VLM Detection
→ Select ball
→ RoboCup Validator
→ Confidence Gate based on 0.98 semantic confidence
→ Commit
```

Fixture 中：

* 语义目标正确；
* 模型输出 `semantic confidence = 0.99`；
* bbox 相比人工框明显偏松；
* 预测框与人工框 IoU 较低；
* 当前 Pipeline 没有几何 Refiner；
* 当前 Confidence Gate 仍允许自动 Commit。

预期旧行为：

```text
Auto accepted
```

本任务完成后的预期行为：

```text
Static validation rejects unsafe auto-commit
```

或者：

```text
Runtime routes the result to mandatory Review
```

必须用测试证明修复前后的差异。

---

# 四、不要简单增加多个虚构 Confidence

不要粗暴实现：

```rust
semantic_confidence: f32,
localization_confidence: f32,
geometry_confidence: f32,
validation_confidence: f32,
```

因为大多数模型并没有提供后面三个可解释的标量。

正确做法是区分：

```text
模型提供的分数
系统测量的几何质量
Validator 状态
校准状态
人工验证状态
```

建议数据模型：

```rust
pub struct DetectionQuality {
    pub semantic_score: Option<QualityScore>,
    pub detector_score: Option<QualityScore>,
    pub geometry: GeometryQualityReference,
    pub validation_state: ValidationState,
}
```

```rust
pub struct QualityScore {
    pub value: f32,
    pub semantics: ScoreSemantics,
    pub source: QualityScoreSource,
}
```

```rust
pub enum ScoreSemantics {
    SemanticConfidence,
    DetectionConfidence,
    CalibratedProbability,
    RelativeConfidence,
    RankingScore,
    NotProvided,
    Unknown,
}
```

```rust
pub enum ValidationState {
    NotEvaluated,
    Passed,
    PassedWithWarnings,
    NeedsReview,
    Failed,
}
```

几何质量不要求一定有一个 `f32`：

```rust
pub struct GeometryQualityReference {
    pub semantics: GeometrySemantics,
    pub calibration_status: GeometryCalibrationStatus,
    pub report_id: Option<GeometryQualityReportId>,
}
```

必须明确：

```text
SemanticConfidence
≠ Detection IoU
≠ Box tightness
≠ Localization accuracy
```

---

# 五、建立每个模型操作的质量契约

不要简单在整个 `qwen3.7-flash` Model Profile 上写：

```text
geometry_quality = bad
```

同一个模型可能用于：

* 文本 Agent；
* 图像分类；
  -属性识别；
* VLM Detection。

质量语义应当绑定到：

```text
Model Profile Revision
+
Capability / Operation
```

新增或整理：

```rust
pub struct ModelCapabilityQualityContract {
    pub model_profile_id: ModelProfileId,
    pub model_profile_revision: u64,
    pub capability: ModelCapability,
    pub output_geometry: GeometrySemantics,
    pub score_semantics: ScoreSemantics,
    pub auto_accept_eligibility: AutoAcceptEligibility,
    pub evidence_source: ContractEvidenceSource,
}
```

```rust
pub enum GeometrySemantics {
    NotApplicable,
    CoarseHypothesis,
    PredictedGeometry,
    RefinedGeometry,
    HumanVerified,
}
```

校准状态必须独立，不要把 `Calibrated` 混进几何来源：

```rust
pub enum GeometryCalibrationStatus {
    Uncalibrated,
    CollectingEvidence,
    Provisional,
    Passed,
    Failed,
    Stale,
}
```

自动接受资格：

```rust
pub enum AutoAcceptEligibility {
    NeverFromScoreAlone,
    RequiresProjectCalibration,
    EligibleWithCalibration,
}
```

默认策略：

### VLM Detection

```text
output_geometry = CoarseHypothesis
score_semantics = SemanticConfidence 或 RelativeConfidence
auto_accept_eligibility = NeverFromScoreAlone
```

### 通用专用检测器

例如 YOLO、RF-DETR：

```text
output_geometry = PredictedGeometry
score_semantics = DetectionConfidence 或 RelativeConfidence
auto_accept_eligibility = RequiresProjectCalibration
```

不要因为它叫专业检测器就直接宣布几何已校准。

### SAM / Prompted Segmentation

```text
output_geometry = RefinedGeometry
score_semantics = RelativeConfidence 或 NotProvided
auto_accept_eligibility = RequiresProjectCalibration
```

SAM 输出边界更细并不意味着永远正确。

### 人工修改

```text
output_geometry = HumanVerified
```

---

# 六、Model Registry 中的默认 VLM 契约

OpenAI-compatible VLM 用于 bbox Detection 时，默认必须获得保守契约：

```yaml
capability: vision_language_detection
output_geometry: coarse_hypothesis
score_semantics: semantic_confidence
auto_accept_eligibility: never_from_score_alone
small_object_localization: unknown
requires_geometry_verification: true
```

用户可以覆盖自定义元数据，但：

* 用户声明必须标记为 `user_declared`；
* 用户声明不等于系统校准；
* 用户声明不能绕过 Project Static Validator；
* 要启用自动几何接受，仍需真实校准证据或人工批准的风险策略。

不得把“支持输出 bbox”自动解释成“输出 bbox 足够精确”。

---

# 七、建立 Project Geometry Policy

不同 Project 对 bbox 精度要求不同。

新增：

```rust
pub struct ProjectGeometryPolicy {
    pub project_id: ProjectId,
    pub task_kind: TaskKind,
    pub required_quality: RequiredGeometryQuality,
    pub auto_accept_policy: GeometryAutoAcceptPolicy,
    pub calibration_thresholds: GeometryCalibrationThresholds,
}
```

```rust
pub enum RequiredGeometryQuality {
    CoarseLocalization,
    TrainingBoundingBox,
    TightBoundingBox,
    PixelAccurateMask,
}
```

足球训练数据默认应使用：

```text
TrainingBoundingBox
```

而不是：

```text
CoarseLocalization
```

自动接受策略：

```rust
pub enum GeometryAutoAcceptPolicy {
    HumanReviewRequired,
    RefinerOrReview,
    CalibrationRequired,
    ExplicitRiskAcceptance,
}
```

默认规则：

```text
VLM Detection + TrainingBoundingBox
→ RefinerOrReview
```

---

# 八、建立几何质量报告

新增或完善：

```rust
pub struct GeometryQualityReport {
    pub id: GeometryQualityReportId,
    pub project_id: ProjectId,
    pub image_id: ImageId,
    pub candidate_artifact_id: ArtifactId,
    pub reference_artifact_id: Option<ArtifactId>,
    pub source: GeometryEvidenceSource,

    pub iou: Option<f32>,
    pub normalized_center_shift: Option<f32>,
    pub pixel_center_shift: Option<f32>,
    pub predicted_area: Option<f32>,
    pub reference_area: Option<f32>,
    pub area_ratio: Option<f32>,
    pub width_ratio: Option<f32>,
    pub height_ratio: Option<f32>,

    pub foreground_occupancy: Option<f32>,
    pub mask_support: Option<f32>,
    pub edge_support: Option<f32>,

    pub issue_codes: Vec<GeometryIssueCode>,
    pub created_at: DateTime<Utc>,
}
```

问题类型：

```rust
pub enum GeometryIssueCode {
    TooLoose,
    TooTight,
    CenterShift,
    WidthError,
    HeightError,
    AspectRatioError,
    PartialObject,
    IncludesBackground,
    RefinerConflict,
    InsufficientEvidence,
}
```

注意：

* 小目标 IoU 对一两个像素非常敏感；
* 不得只依赖平均 IoU；
* 必须同时观察中心偏移、面积比例和人工调整率；
* 应按目标像素面积分桶统计：

  * small；
  * medium；
  * large。

尤其 RoboCup 足球通常是小目标，不能让几个较大的球框把整体平均值装饰得过于体面。

---

# 九、人工 Review 必须产生结构化几何反馈

Review 中增加或整理原因：

```text
Too loose
Too tight
Shifted
Wrong object
Missed object
Duplicate
Wrong label
Other
```

RoboCup Ball Skill 还可以提供：

```text
White shoe
White sock
Penalty mark
Field-line intersection
```

当用户修改 bbox 时，系统自动计算：

```text
原框与人工框 IoU
中心移动
面积缩放
宽度变化
高度变化
```

记录：

```rust
pub struct GeometryCorrectionEvidence {
    pub project_id: ProjectId,
    pub run_id: RunId,
    pub image_id: ImageId,
    pub annotation_id: AnnotationId,
    pub source_node_id: NodeId,
    pub source_model_profile_id: ModelProfileId,
    pub source_model_revision: u64,
    pub original_geometry: GeometrySnapshot,
    pub corrected_geometry: GeometrySnapshot,
    pub reason: GeometryCorrectionReason,
    pub quality_report_id: GeometryQualityReportId,
    pub created_at: DateTime<Utc>,
}
```

这些数据必须可被：

* Dry Run Summary；
* Calibration；
* Pipeline Builder Agent；
* Improve Pipeline；

读取。

不能只把人工修改保存成一个新的 bbox，然后假装系统从中获得了经验。

---

# 十、建立 Project/Model 几何校准

校准必须绑定具体上下文，不能全局宣布：

```text
qwen3.7-flash 已校准
```

建议 Key：

```rust
pub struct GeometryCalibrationKey {
    pub project_id: ProjectId,
    pub task_id: TaskId,
    pub label_id: Option<LabelId>,
    pub model_profile_id: ModelProfileId,
    pub model_profile_revision: u64,
    pub node_definition_id: NodeDefinitionId,
    pub node_config_hash: String,
    pub prompt_version: Option<String>,
    pub preprocessing_hash: String,
    pub dataset_profile_revision: String,
}
```

报告：

```rust
pub struct GeometryCalibrationReport {
    pub id: GeometryCalibrationId,
    pub key: GeometryCalibrationKey,
    pub status: GeometryCalibrationStatus,
    pub sample_count: u32,
    pub small_object_sample_count: u32,

    pub median_iou: Option<f32>,
    pub p10_iou: Option<f32>,
    pub median_center_shift: Option<f32>,
    pub p90_center_shift: Option<f32>,
    pub median_area_ratio_error: Option<f32>,
    pub manual_adjustment_rate: Option<f32>,
    pub too_loose_rate: Option<f32>,
    pub too_tight_rate: Option<f32>,

    pub thresholds: GeometryCalibrationThresholds,
    pub evidence_run_ids: Vec<RunId>,
    pub created_at: DateTime<Utc>,
}
```

状态要求：

### `Uncalibrated`

无充分证据。

### `CollectingEvidence`

已有少量结果，但不够支持自动接受。

### `Provisional`

样本较少，只能用于 Dry Run 或受限项目。

### `Passed`

达到 Project 阈值，可允许特定 Geometry Gate 自动接受。

### `Failed`

证据显示不满足要求。

### `Stale`

以下任一发生变化：

* Model Profile Revision；
* Prompt Version；
* Resize/Grid/Tile 配置；
* Node 配置；
* Label Schema；
* Refiner；
* 数据分布版本。

校准必须失效或重新评估。

API Key 轮换不导致校准失效。

---

# 十一、校准样本不能和优化样本完全相同

Pipeline 自我改进必须防止在用户刚修过的几张图上自我陶醉。

至少区分：

```text
Diagnosis evidence
Evaluation holdout
```

规则：

1. 人工修正样本可以用于诊断；
2. 改进后的 Pipeline 必须尽量在独立 Holdout 上评估；
3. 样本不足时明确显示：

   ```text
   Insufficient evaluation evidence
   ```
4. 不得把相同三张图片既用于决定加 SAM，又用于证明 SAM 完美；
5. 只有 5 张图片的 Demo 可以标记为 `provisional comparison`；
6. 不得基于 4 张图片生成“模型已经校准”的生产级结论。

---

# 十二、强制 Static Validation 规则

在 Rust Workflow Static Validator 中增加发布阻断规则。

核心规则：

```text
如果任务输出是 bounding_box，
候选来自 CoarseHypothesis 或未校准 PredictedGeometry，
则不能只通过 SemanticConfidence 或 RelativeConfidence
直接进入 Commit。
```

至少需要满足以下之一：

1. 经过 `HumanReview`；
2. 经过可用 Geometry Refiner；
3. 经过 `Geometry Quality Evaluation`；
4. 存在当前 Project 和当前 Model/Node Revision 的有效 Calibration；
5. 用户对该 Published Draft 做过明确、可审计的风险批准。

建议错误码：

```text
uncalibrated_geometry_auto_commit
semantic_score_used_as_geometry_evidence
geometry_acceptance_path_missing
geometry_calibration_missing
geometry_calibration_stale
geometry_refiner_unavailable
unsafe_legacy_workflow
```

示例：

```text
VLM Detection
→ Confidence Gate(semantic confidence >= 0.92)
→ Commit
```

必须失败：

```text
uncalibrated_geometry_auto_commit
```

示例：

```text
VLM Detection
→ Mandatory Human Review
→ Commit
```

合法。

示例：

```text
VLM Detection
→ Box Prompt
→ Prompted Segmentation
→ Mask to BBox
→ Geometry Evaluation
→ Geometry Decision
→ Commit / Review
```

在模型可用、Contract 合法时可通过。

示例：

```text
Calibrated Specialist Detection
→ Geometry Decision
→ Commit / Review
```

在校准有效时可通过。

---

# 十三、不要破坏不可变历史版本

已有 Published Workflow 和历史 Run 必须保持原样可审计。

不得静默修改历史版本语义。

增加：

```rust
pub enum WorkflowSafetyCompatibility {
    Safe,
    RequiresMigration,
    LegacyRiskAccepted,
    UnsafeForNewRuns,
}
```

对于历史上存在：

```text
Uncalibrated VLM bbox
→ Semantic Confidence Gate
→ Commit
```

的 Published Version：

1. 历史 Run 继续可查看；
2. 历史 Version 不修改；
3. Replay 可以在 Sandbox 中运行；
4. 默认阻止用该旧版本启动新的正式 Run；
5. 提供：

   ```text
   Create geometry-safe draft
   ```
6. 创建新 Draft 时保留原结构并增加安全路径；
7. 用户可以显式选择 Legacy Risk Acceptance，但必须记录审计事件；
8. 课程 Demo 默认不得使用 Legacy Risk Acceptance 绕过规则。

---

# 十四、首次 Pipeline 生成策略

Pipeline Builder Agent 第一次构造 bbox Pipeline 时，就必须读取：

* Model capability quality contract；
* Geometry semantics；
* Score semantics；
* Project Geometry Policy；
* 当前 Calibration；
* 当前健康 Refiner；
* 当前 Specialist Detector；
* 当前 Domain Skill。

## 情况 A：只有 Qwen VLM

不得生成：

```text
Image
→ Qwen Detection
→ Semantic Confidence Gate
→ Commit
```

应生成：

```text
Image
→ Optional grid/resize support
→ Qwen VLM coarse detection
→ Select target label
→ Domain validation
→ Mandatory Human Review
→ Commit
```

同时给出：

```text
Geometry status:
Uncalibrated coarse proposal

Automatic bbox acceptance:
Disabled

Optional improvement:
Configure a prompted-segmentation or calibrated specialist detector.
```

Grid 可以提高粗定位，但不能把 Geometry Semantics 从：

```text
CoarseHypothesis
```

变成：

```text
Calibrated
```

## 情况 B：健康 SAM 可用

建议：

```text
Image
→ VLM coarse detection
→ Select target
→ Detections to Box Prompts
→ Prompted Segmentation
→ Mask Quality Validation
→ Mask to BBox
→ Compare coarse and refined geometry
→ Geometry Decision
    ├── stable → Commit
    └── large change / weak mask → Review
```

## 情况 C：SAM 不可用

不得把 SAM 加进 Runnable Draft。

可以显示：

```text
Suggested optional improvement
Prompted segmentation backend is not configured.
```

当前 Draft 必须使用：

```text
Mandatory Review
```

## 情况 D：健康 Specialist Detector 可用

可以优先建议：

```text
Specialist Detection
→ Geometry Validation
→ Domain Validation
→ Decision
→ Commit / Review
```

但若 Specialist 尚未针对当前 Project 校准，不得仅凭 score 绕过几何政策。

## 情况 E：Provider Failure

不得建议 SAM。

应建议：

```text
Provider repair
Configured provider fallback
Alternative available detector
Human review
```

## 情况 F：No Candidate

不得建议依赖 bbox Prompt 的 SAM。

可建议：

```text
Tile
Resize
Open-vocabulary detector
Specialist detector
Alternative VLM
Human search/review
```

## 情况 G：Wrong Object

例如白鞋被识别为球。

优先建议：

```text
Crop Classification
Domain Validator
Second detector
Correction Memory
Review
```

SAM 只会更精确地框住白鞋，依然没有让它变成足球。

---

# 十五、更新 Pipeline Builder Agent 系统规则

更新 Advisor System Prompt，但不能只靠 Prompt。

加入以下规则：

```text
1. A VLM detection score usually reflects semantic confidence,
   not bounding-box IoU, tightness, or center accuracy.

2. Treat VLM-produced bounding boxes as uncalibrated coarse
   hypotheses unless project-specific calibration evidence proves
   otherwise.

3. Never route an uncalibrated VLM bounding box directly to Commit
   using only semantic or relative confidence.

4. For training-quality bounding boxes, require at least one:
   - human review;
   - prompted-segmentation refinement plus geometry evaluation;
   - project-specific calibration;
   - another compatible source of localization evidence.

5. Grid overlays, coordinate instructions, and larger prompts can
   improve coarse localization but do not constitute geometry
   calibration.

6. Do not add SAM when:
   - the provider call failed;
   - no candidate exists;
   - the candidate is semantically the wrong object.

7. Consider prompted segmentation only when:
   - a candidate box exists;
   - the target is semantically plausible;
   - the issue is geometric;
   - a healthy compatible backend exists;
   - a valid Detection → Prompt → Mask → Geometry path exists.

8. Do not add unavailable, disabled, missing-weight, mock-only, or
   incompatible models to a runnable draft.

9. Use human correction and Dry Run evidence to decide whether
   refinement is needed.

10. Do not claim a model is geometrically calibrated without a valid
    calibration report for the exact model revision, prompt,
    preprocessing, node configuration, task, and project scope.

11. All proposals remain Drafts.
    You cannot publish or start a full dataset run.

12. When evidence is insufficient, prefer conservative Review rather
    than fabricated confidence.
```

---

# 十六、为 Pipeline Builder 增加几何工具

新增或整理以下受控 Agent Tools。

## 16.1 检查模型质量契约

```text
inspect_model_quality_contract
```

返回：

* Operation；
* Score Semantics；
* Geometry Semantics；
* Auto-accept eligibility；
* Calibration status；
* Model availability。

## 16.2 检查 Project Geometry Policy

```text
inspect_project_geometry_policy
```

## 16.3 检查历史人工修正

```text
inspect_geometry_correction_summary
```

返回有限聚合信息：

* 样本数；
* IoU；
* 中心偏移；
* 面积变化；
* too loose；
* too tight；
* wrong object；
* size bucket。

## 16.4 检查校准

```text
inspect_geometry_calibration
```

## 16.5 查找合法 Refiner 路径

```text
find_geometry_refinement_path
```

例如：

```text
DetectionSet
→ BoxPromptSet
→ PromptedSegmentation
→ MaskSet
→ MaskToBBox
→ DetectionSet
```

## 16.6 比较 Pipeline 几何质量

```text
compare_pipeline_geometry
```

输入：

* baseline Dry Run；
* candidate Dry Run。

输出：

* recall；
* semantic errors；
* geometry metrics；
* review rate；
* cost；
* latency；
* regressions；
* evidence sufficiency。

## 16.7 创建安全迁移 Draft

```text
create_geometry_safe_draft
```

只能基于现有 Published Version 创建 Draft，不修改原版本。

---

# 十七、Pipeline 自我改进闭环

新增产品操作：

```text
Improve Automation
```

入口至少出现在：

* Project Overview；
* Run Results；
* Review Summary；
* Pipeline Version 页面。

启动后执行：

```text
选择 Evidence Runs
→ 汇总人工修正和 Validator 问题
→ Failure Classification
→ 检查可用 Models 和 Refinement Paths
→ 创建现有 Pipeline 的 Patch Draft
→ Static Validation
→ Before / After Dry Run
→ Compare
→ 提交人工审批
```

Agent 不应默认重写整个 Pipeline。

优先生成：

```text
Pipeline Patch
```

而不是：

```text
Brand-new unrelated workflow
```

---

# 十八、错误诊断必须分类

实现或完善：

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
    InsufficientEvidence,
}
```

当前足球案例应当能够诊断为：

```text
GeometryError
```

而不是：

```text
SemanticError
```

结构化诊断示例：

```text
Classification:
LocalizationGeometryError

Evidence:
- The semantic target was correct in 4/4 reviewed detections.
- Semantic scores were 0.98–0.99.
- Human edits repeatedly shrank or shifted the boxes.
- The source model operation emits CoarseHypothesis geometry.
- The active workflow used semantic confidence for auto-acceptance.
- No valid geometry calibration existed.
- No healthy prompted-segmentation backend was available.
```

不得保存或展示模型隐藏思维链。

允许显示的是结构化证据和结论。

---

# 十九、生成 Pipeline Patch

若 SAM 可用，Patch 示例：

```diff
 Image Input
 Qwen VLM Detection
 Select football
+Detections to Box Prompts
+Prompted Segmentation
+Mask Quality Validation
+Mask to Bounding Box
+Compare Geometry
-Raw Semantic Confidence Decision
+Geometry-aware Decision
 Commit / Review
```

若 SAM 不可用：

```diff
 Image Input
 Qwen VLM Detection
 Select football
 RoboCup Ball Validator
-Semantic Confidence → Commit
+Mandatory Geometry Review
 Commit
```

同时生成未解析改进建议：

```text
Optional setup requirement:
Configure a Prompted Segmentation model to enable automatic
geometry refinement.
```

---

# 二十、Before / After 评估

候选 v2 必须和当前 v1 比较。

至少展示：

```text
Semantic precision
Semantic recall
Mean / median IoU
P10 IoU
Median center shift
P90 center shift
Manual resize rate
Too-loose rate
Too-tight rate
No-candidate rate
Review rate
Cost per image
Latency per image
Failure count
```

按小目标分桶：

```text
Small objects
Medium objects
Large objects
```

推荐 v2 的默认规则：

1. Recall 不显著下降；
2. Geometry 指标有真实改善；
3. 人工调整率下降；
4. Review rate 不产生不可接受增长；
5. Cost 和 latency 未超过 Project 硬约束；
6. Evaluation evidence 足够；
7. 无新的严重错误类型。

如果改进证据不足，显示：

```text
Candidate pipeline created,
but there is not enough independent evaluation evidence to recommend it.
```

不得用四张图宣布新 Pipeline 已经普遍更优。

---

# 二十一、SAM 不能被无条件信任

SAM 后处理必须保留：

* 原始 VLM bbox；
* Box Prompt；
* Mask；
* Mask-to-BBox 结果；
* coarse/refined IoU；
* 中心移动；
* 面积变化；
* Mask quality；
* 最终 Decision。

规则示例：

```text
SAM refinement stable
+ mask quality high
+ area change within range
→ eligible for geometry decision
```

```text
SAM moves center dramatically
or shrinks area excessively
or mask quality is weak
→ Review
```

不得：

```text
SAM returned a mask
→ automatically trust
```

SAM 也会犯错，只是它犯错时边缘往往画得更精致。

---

# 二十二、GUI 改造

## 22.1 Run Results

不要只显示：

```text
Confidence: 0.99
```

改为：

```text
Semantic confidence
0.99

Box quality
Uncalibrated coarse proposal

Geometry verification
Not performed
```

如果经过 SAM：

```text
Semantic confidence
0.99

Box quality
Refined by prompted segmentation

Geometry evidence
Coarse/refined IoU: 0.63
Area reduced by 34%
Status: needs review
```

## 22.2 Automation 页面

对危险流程显示发布阻断：

```text
Automatic acceptance is unsafe

The selected VLM provides semantic confidence,
but its bounding boxes are not geometrically calibrated.

Choose one:
[Require human review]
[Add compatible refiner]
[Run geometry calibration]
```

## 22.3 Model Profile

显示：

```text
VLM Detection

Geometry output:
Coarse proposal

Score meaning:
Semantic confidence

Project calibration:
Not calibrated
```

## 22.4 Review

增加：

```text
Too loose
Too tight
Shifted
Wrong object
Object missed
```

修改后显示：

```text
This correction will be used as geometry-quality evidence
for future pipeline improvements.
```

## 22.5 Improve Automation

展示：

* 诊断；
* 使用的 Evidence；
* Workflow Diff；
* 新增 Model Binding；
* Before / After；
* Cost；
* Latency；
* Evidence sufficiency；
* Save as Draft；
* Apply selected changes；
* Publish 仍需人工操作。

---

# 二十三、Guided Mode 与 Expert Mode

Guided Mode 使用：

```text
Semantic certainty
Box quality
Needs geometry check
Refine box
Review uncertain box
Improve automation
```

Expert Mode 使用：

```text
ScoreSemantics
GeometrySemantics
GeometryCalibrationStatus
GeometryQualityReport
Artifact lineage
Model Profile Revision
Node Config Hash
```

默认模式不得用一排内部 enum 欢迎用户。

---

# 二十四、API 和 Storage

复用现有 API 风格，增加或整理：

```text
GET  /api/model-profiles/:modelId/quality-contracts
GET  /api/projects/:projectId/geometry-policy
PUT  /api/projects/:projectId/geometry-policy

GET  /api/projects/:projectId/geometry-calibrations
POST /api/projects/:projectId/geometry-calibrations
GET  /api/geometry-calibrations/:calibrationId

GET  /api/runs/:runId/geometry-summary
GET  /api/projects/:projectId/geometry-corrections

POST /api/projects/:projectId/pipeline-improvements
GET  /api/pipeline-improvements/:improvementId
POST /api/pipeline-improvements/:improvementId/compare
POST /api/pipeline-improvements/:improvementId/apply-to-draft

POST /api/workflow-versions/:versionId/create-safe-draft
```

Storage 至少保存：

```text
model capability quality contracts
project geometry policies
geometry quality reports
geometry correction evidence
geometry calibration reports
pipeline improvement sessions
baseline/candidate comparisons
workflow safety compatibility
```

不得保存 API Key。

---

# 二十五、Migration

现有数据可能只有：

```text
confidence
geometry_semantics = predicted_geometry
```

迁移规则必须保守：

1. 旧 VLM Detection 的 score 默认解释为：

   ```text
   RelativeConfidence 或 SemanticConfidence
   ```

   不得迁移为 calibrated geometry score。

2. 旧 VLM Geometry 默认迁移为：

   ```text
   CoarseHypothesis
   ```

3. 旧 Specialist Detector 默认迁移为：

   ```text
   PredictedGeometry + Uncalibrated
   ```

4. 历史人工修改可生成 Geometry Correction Evidence；

5. 历史版本不修改；

6. 新正式 Run 对 unsafe legacy workflow 默认阻断；

7. 提供 Create Safe Draft；

8. Migration 可重复执行；

9. 使用数据库事务；

10. 有完整回滚测试。

---

# 二十六、必须完成的测试

## Case 1：高语义分数不能通过 Geometry Gate

```text
VLM bbox
semantic confidence = 0.99
geometry = CoarseHypothesis
calibration = Uncalibrated
→ Semantic Confidence Gate
→ Commit
```

预期：

```text
Static validation fails:
semantic_score_used_as_geometry_evidence
```

## Case 2：VLM + Mandatory Review 合法

```text
VLM coarse bbox
→ Review
→ Commit
```

预期通过。

## Case 3：健康 SAM 路径

```text
VLM coarse bbox
→ Box Prompt
→ SAM
→ Mask
→ Mask to BBox
→ Geometry Evaluation
→ Decision
→ Commit / Review
```

预期通过。

## Case 4：SAM unavailable

预期：

* SAM 不进入 Runnable Draft；
* Agent生成 Review fallback；
* SAM 作为 setup alternative；
* Draft 可以保存；
* 不伪造 Worker。

## Case 5：Provider Failure 不建议 SAM

预期：

```text
failure class = ProviderFailure
```

Agent 不加入 SAM。

## Case 6：No Candidate 不建议 SAM

Agent优先建议：

```text
Tile / alternative detector / open vocabulary / review
```

## Case 7：Wrong Object 不用 SAM 作为主要修复

白鞋误检时优先：

```text
Crop classification
RoboCup validator
Review
```

## Case 8：人工修框产生几何证据

修改 bbox 后验证：

* IoU；
* center shift；
* area ratio；
* reason；
* lineage；
* model revision。

## Case 9：校准通过

使用足够样本和阈值：

* 生成 Passed Calibration；
* 精确绑定 Model Revision 和 Node Config；
* Geometry Decision 可以读取。

## Case 10：校准失效

修改：

* Prompt；
* Grid；
* Resize；
* Model Revision；
* Node Config。

预期 Calibration 变为 `Stale`。

## Case 11：首次只配置 Qwen

Advisor 必须生成：

```text
VLM coarse detection
→ Domain validation
→ Mandatory Review
→ Commit
```

不得直接自动接受。

## Case 12：注册 SAM 后重新改进

```text
Improve Automation
→ 发现 GeometryError
→ 查找到 SAM conversion path
→ 创建 v2 Draft
→ Before / After Dry Run
```

## Case 13：Before / After 没改善

预期：

* Candidate Draft 保存；
* 不显示 Recommend v2；
* 人工仍可查看；
* 不自动发布。

## Case 14：Legacy Workflow

旧 Published Version：

* 历史可查看；
* 新 Run 默认阻断；
* Create Safe Draft 可用；
* 原版本 hash 不变。

## Case 15：小目标分桶

确保大目标良好结果不能掩盖小目标框误差。

## Case 16：Generic Project

不启用 RoboCup 时：

* Geometry safety 仍生效；
* 页面无 RoboCup；
* Domain reason 不出现。

---

# 二十七、真实足球回归验证

若当前 B-Human 四张或五张图片仍在工作区中：

1. 不修改原历史 Run；
2. 将现有预测和人工参考导出为回归 Fixture；
3. 比较：

   * 当前 VLM-only；
   * VLM + mandatory review；
   * VLM + local foreground fallback；
   * VLM + real SAM，若 Worker 可用；
4. 记录：

   * 每图 bbox；
   * IoU；
   * center shift；
   * area ratio；
   * review outcome；
   * duration；
   * cost。

没有人工 Ground Truth 时：

* 允许用户在 Review 中创建；
* 不得用模型自己的输出作为 Ground Truth；
* 不得声称几何改进已经得到客观证明。

---

# 二十八、Milestone 计划

## Milestone 0：复现与基线

完成：

* 当前危险 Pipeline Fixture；
* 当前 0.99 semantic score 自动 Commit 回归；
* 当前数据模型盘点；
* 状态文档；
* 验收矩阵初稿。

提交：

```text
test(geometry): reproduce unsafe vlm bbox auto-acceptance
```

## Milestone 1：质量语义和 Model Contract

完成：

* Score Semantics；
* Geometry Semantics；
* Capability Quality Contract；
* Auto-accept Eligibility；
* migrations；
  -测试。

提交：

```text
feat(models): separate semantic confidence from geometry quality
```

## Milestone 2：Static Geometry Safety

完成：

* Project Geometry Policy；
* Static Validator；
  -错误码；
* Legacy compatibility；
* Create Safe Draft；
  -测试。

提交：

```text
feat(workflow): block uncalibrated geometry from score-only commit
```

## Milestone 3：Review 几何反馈

完成：

-结构化 Review reasons；

* Geometry Quality Report；
* Correction Evidence；
  -大小分桶；
  -API；
  -测试。

提交：

```text
feat(review): capture structured bbox correction evidence
```

## Milestone 4：Calibration

完成：

* Calibration Key；
* Calibration Report；
  -状态；
  -失效规则；
  -Project thresholds；
  -API；
  -测试。

提交：

```text
feat(evaluation): calibrate geometry quality by model and project
```

## Milestone 5：Advisor 首次安全生成

完成：

-质量契约 Tools；

* Geometry Policy Tool；
* Calibration Tool；
* system prompt；
  -只有 Qwen 时的保守 Draft；
  -SAM unavailable alternative；
  -测试。

提交：

```text
feat(agent): build geometry-safe pipelines from the first draft
```

## Milestone 6：Refinement Path

完成：

* Detection → Prompt → Mask → BBox；
* Geometry Comparison；
  -SAM 可用/不可用路径；
  -Artifact lineage；
  -测试。

提交：

```text
feat(workflow): add auditable prompted geometry refinement
```

## Milestone 7：Improve Automation Agent

完成：

-入口；
-诊断；
-Pipeline Patch；
-Before / After；
-Holdout；
-推荐规则；
-人工审批；
-测试。

提交：

```text
feat(agent): improve pipelines from review and geometry evidence
```

## Milestone 8：GUI、TUI 和 Release

完成：

* semantic confidence / box quality 分离显示；
* calibration UI；
* static blocker 修复入口；
* improve automation；
  -TUI；
  -E2E；
  -文档；
  -真实 smoke test，若环境允许。

提交：

```text
test(release): validate geometry-safe self-improving annotation alpha
```

---

# 二十九、Release Blocking Acceptance Matrix

## A. 质量语义

* [ ] Semantic Confidence 与 Geometry Quality 分离。
* [ ] 不为 Geometry Quality 伪造默认数值。
* [ ] VLM Detection 默认是 CoarseHypothesis。
* [ ] Specialist Detection 默认是 PredictedGeometry，不自动视为已校准。
* [ ] SAM 输出是 RefinedGeometry，不自动视为已校准。
* [ ] 人工修改是 HumanVerified。
* [ ] Score Semantics 被保存和展示。

## B. 静态安全

* [ ] VLM semantic score 不能单独允许 bbox Commit。
* [ ] TrainingBoundingBox Project 必须有几何接受路径。
* [ ] Missing calibration 会阻止危险发布。
* [ ] Stale calibration 会阻止危险发布。
* [ ] Human Review 是合法保守路径。
* [ ] Prompted refinement 是合法候选路径。
* [ ] Legacy Published Version 保持不可变。
* [ ] Unsafe legacy version 默认不能启动新正式 Run。

## C. 首次 Pipeline

* [ ] 只有 Qwen 时默认 Mandatory Review。
* [ ] Grid 不被视为 Calibration。
* [ ] 无 SAM 时不生成假的 SAM Runnable Draft。
* [ ] 有 SAM 时可以生成完整转换链。
* [ ] Provider Failure 不建议 SAM。
* [ ] No Candidate 不建议 SAM。
* [ ] Wrong Object 不用 SAM 作为主要修复。
* [ ] Advisor 最终只提交 Draft。

## D. Review 和 Calibration

* [ ] Review 支持 Too Loose。
* [ ] Review 支持 Too Tight。
* [ ] Review 支持 Shifted。
* [ ] Review 支持 Wrong Object。
* [ ] 自动计算 IoU、center shift 和 area ratio。
* [ ] Correction 绑定来源节点和模型版本。
* [ ] Calibration 绑定精确 Model/Prompt/Config。
* [ ] 配置改变会使 Calibration Stale。
* [ ] 小目标单独统计。
* [ ] 样本不足时不声称校准通过。

## E. Pipeline 自我改进

* [ ] Run 和 Review 有 Improve Automation。
* [ ] Agent 能诊断 GeometryError。
* [ ] Agent 生成 Patch，而非默认重写全部 Pipeline。
* [ ] Agent 检查真实可用 Model。
* [ ] Agent 执行 Static Validation。
* [ ] Agent 执行 Before / After Dry Run。
* [ ] 使用独立 Holdout，条件允许时。
* [ ] 无改进时不推荐 v2。
* [ ] Agent 不自动 Publish。
* [ ] Published v1 不被修改。

## F. 产品

* [ ] Run 同时显示 Semantic Confidence 和 Box Quality。
* [ ] 未校准状态用户可见。
* [ ] 危险自动接受有明确 Blocker。
* [ ] Blocker 有 Require Review、Add Refiner、Run Calibration 等修复动作。
* [ ] Guided Mode 不暴露无必要内部 ID。
* [ ] Expert Mode 可查看完整 Quality Report。
* [ ] Generic Project 不出现 RoboCup 内容。
* [ ] 全局品牌仍为 AnnotAgent。

## G. 回归

* [ ] Batch 不回归。
* [ ] Pause、Resume、Cancel 不回归。
* [ ] Artifact lineage 不回归。
* [ ] Replay 不回归。
* [ ] Review 不回归。
* [ ] Export 不回归。
* [ ] Provider 管理不回归。
* [ ] Token 和费用统计不回归。
* [ ] Tool Call 历史不回归。

---

# 三十、最终测试命令

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

E2E 至少覆盖：

1. unsafe VLM auto-commit 被拦截；
2. safe mandatory review；
3. SAM available refinement；
4. SAM unavailable fallback；
5. Provider Failure；
6. No Candidate；
7. Wrong Object；
8. Review geometry feedback；
9. Calibration pass；
10. Calibration stale；
11. first-draft Qwen safety；
12. Improve Automation；
13. Before / After；
14. Legacy safe migration；
15. Generic Project no RoboCup；
16. URL 和状态恢复；
    17.原有 Run、Review、Replay 和 Export。

真实 SAM 或 Qwen 不可用时：

* 完成 Mock；
  -完成 Contract；
  -完成 Runtime；
  -完成 UI；
  -完成测试；
  -标记为 live-conditional；
  -不得用 Mock 冒充真实结果。

---

# 三十一、不得采用的假修复

禁止：

* 只修改 Prompt；
* 只把 Confidence Gate 阈值从 0.92 调高到 0.99；
* 把一个 Confidence 拆成四个随意生成的浮点数；
* 把所有 VLM 框无条件交给 SAM；
* 把所有 SAM Mask 无条件当成正确；
* Provider 失败时建议 SAM；
* 没候选时建议 SAM；
* 用 SAM 修语义误检；
* 把 Grid 视为几何校准；
* 把 Specialist 模型天然视为已校准；
* 用模型输出自己验证自己；
* 用相同样本同时优化和证明改进；
* 静默修改 Published Version；
* 为了通过校验让 Agent 自动 Publish；
* 没有真实 Worker 时使用 Mock 节点进入正式 Draft；
* 删除历史 Run；
* 修改 remote；
* push；
  -提交 API Key；
  -提交模型权重。

---

# 三十二、文档

新增：

```text
docs/VLM_GEOMETRY_SAFETY.md
docs/GEOMETRY_QUALITY_MODEL.md
docs/GEOMETRY_CALIBRATION.md
docs/PIPELINE_SELF_IMPROVEMENT.md
docs/SAFE_VLM_DETECTION_PIPELINES.md
docs/LEGACY_WORKFLOW_MIGRATION.md
docs/DEMO_GEOMETRY_SAFETY.md
```

更新：

```text
README.md
docs/DESIGN.md
docs/AGENT_LOOP.md
docs/CORE_AND_SKILLS.md
docs/GUIDED_EXPERIENCE.md
docs/ROBOCUP_SKILL.md
docs/KNOWN_LIMITATIONS.md
docs/COURSE_REQUIREMENTS.md
```

---

# 三十三、最终报告

最终报告必须包含：

## 1. 当前错误如何复现

说明：

-原 Pipeline；
-语义 score；
-几何问题；
-为什么旧 Gate 错误接受。

## 2. 数据模型

说明：

* Score Semantics；
* Geometry Semantics；
* Calibration Status；
* Quality Report；
  -为什么没有伪造多个 Confidence。

## 3. Static Validator

说明：

-新增规则；
-错误码；
-合法几何接受路径；
-Legacy 版本处理。

## 4. 首次 Pipeline 生成

说明：

-只有 VLM；
-有 SAM；
-有 Specialist；
-SAM unavailable；
-Provider Failure；
-No Candidate；
-Wrong Object。

## 5. Review 与 Calibration

说明：

-结构化反馈；
-几何指标；
-校准 Scope；
-失效条件；
-小目标处理。

## 6. Improve Automation

说明：

-诊断；
-Patch；
-Before / After；
-Holdout；
-推荐标准；
-人工发布边界。

## 7. SAM 流程

说明：

-粗框；
-Box Prompt；
-Mask；
-Mask-to-BBox；
-质量比较；
-回退。

## 8. 测试结果

列出真实执行命令和结果。

不得把未执行测试写成通过。

## 9. Milestone 提交

列出：

```text
commit hash
commit message
milestone
```

## 10. Live-conditional

分别说明：

* Qwen；
* SAM；
* Specialist；
  -权重；
  -GPU；
  -真实人工 Ground Truth。

## 11. 未完成内容

明确区分：

```text
未实现
已实现但未验证
外部环境阻塞
不属于本轮
```

禁止使用：

```text
基本完成
理论上支持
应该已经修好
大概率更准确
```

## 12. Git 状态

说明：

-当前分支；
-工作区；
-领先远程提交数；
-未 push；
-remote 未修改。

---

# 三十四、启动指令

将本文保存为：

```text
docs/execution/GEOMETRY_SAFETY_MASTER_PROMPT.md
```

然后从仓库根目录启动 Codex，输入：

```text
阅读 docs/execution/GEOMETRY_SAFETY_MASTER_PROMPT.md，并将其作为本次长期任务的最高目标。

先核验当前 Model Quality Metadata、VLM Detection、Confidence Gate、Static Validator、Review Revision、SAM Adapter、Calibration 和 Pipeline Builder，不要盲信文档中的完成声明。

本次任务重点不是手工替当前 RoboCup Project 改 Workflow，而是让 AnnotAgent：

1. 从首次生成 Pipeline 时就将 VLM bbox 视为未经校准的粗几何假设；
2. 将语义置信度和几何质量彻底分离；
3. 在 Rust Static Validator 中阻止 semantic-confidence-only bbox Commit；
4. 在无 Refiner 时生成 Mandatory Review 的安全流程；
5. 在存在健康 SAM 时生成可审计的 Detection → Prompt → Mask → BBox 链；
6. 从人工 bbox 修正中提取结构化几何证据；
7. 对具体 Project、Model Revision、Prompt 和配置进行校准；
8. 让 Improve Automation Agent 诊断问题并生成 Pipeline Patch；
9. 通过 Before / After Dry Run 验证改进；
10. 保持人工批准和 Published Version 不可变边界；
11. 保持 Batch、Artifact、Replay、Review、Provider 和 Export 不回归。

从 Milestone 0 开始持续执行。

普通技术决策自行决定，并记录到 GEOMETRY_SAFETY_DECISIONS.md。

每完成一个 Milestone：
1. 更新状态和验收证据；
2. 执行对应 Rust、Web 和 E2E 测试；
3. 修复回归；
4. 创建独立本地提交；
5. 继续下一 Milestone。

真实 SAM、Qwen 或 Specialist 不可用时：
-完成 Mock；
-完成 Contract；
-完成 Static Validator；
-完成 Calibration；
-完成 Agent Loop；
-完成 UI；
-完成测试；
-将真实推理标记为 live-conditional。

不得用 Mock 冒充真实模型结果。

不要 push。
不要修改 Git remote。
不要使用、恢复或提交任何对话中出现过的 API Key。
不要提交模型权重。
不要执行 reset、rebase、amend 或破坏性 checkout。

只有 Release Blocking Acceptance Matrix 全部满足，或只剩明确记录的 live-conditional 项时，才输出最终报告。
```
