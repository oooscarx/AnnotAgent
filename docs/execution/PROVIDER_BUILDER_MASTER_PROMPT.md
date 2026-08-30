# AnnotAgent Provider Registry + LLM Pipeline Builder Alpha

你现在是 AnnotAgent 仓库的长期主实现工程师。

本次任务有两个核心目标：

1. 将 LLM/VLM Provider 管理提升为一等产品能力，让用户配置一次 Provider、凭证和模型，之后在不同 Project、Agent 和 Workflow 节点中复用；
2. 为 Pipeline Builder Agent 提供一套受 Registry 约束的工具与节点目录，使 LLM 可以构造、校验、试跑和修订标注调用链，但不能生成任意代码或绕过 Rust Runtime。

本任务不是简单增加一个 Provider 下拉框，也不是把 API Key 从一个文本框移动到另一个文本框。

最终产品关系必须是：

```text
Provider
    └── 提供连接、认证、协议和账户边界

Model Profile
    └── 表示 Provider 下可被选择的具体模型及能力

Project Model Binding
    └── 为 Project 或 Workflow capability 选择 Model Profile

Pipeline Builder Agent
    └── 从已配置 Model Profiles、Skills 和 Node Catalog 中构造 Draft

Workflow Runtime
    └── 校验并执行已发布的不可变调用链
```

产品名称始终是：

```text
AnnotAgent
```

RoboCup 仍然只是 Domain Skill 和示例 Project，不得重新成为全局品牌。

---

# 一、先核验当前仓库

开始前必须执行：

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
examples/
workspace/
```

重点核验：

* 当前 Provider 配置；
* 当前 OpenAI-compatible Provider；
* 当前 Model Binding；
* 当前 Model Registry；
* 当前 API Key 保存方式；
* 当前 Provider 预设；
* 当前模型健康状态；
* 当前 Pipeline Advisor；
* 当前 Skill Registry；
* 当前 Node Registry；
* 当前 Workflow Draft；
* 当前 Dry Run；
* 当前 Artifact；
* 当前 Token 和费用记录；
* 当前 Web Settings；
* 当前 TUI 模型管理；
* 当前数据库 migration；
* 当前测试数量和结果。

不得盲信文档中的“已完成”描述。

如果现有能力已经正确实现，应迁移和复用，不要重新创建同义类型。

禁止：

```text
git reset
git rebase
git commit --amend
破坏性 git checkout
修改 Git remote
push
提交 API Key
在日志中打印 API Key
将 API Key 返回给前端
提交模型权重
用 Mock 冒充真实调用
```

---

# 二、长期执行状态文件

创建并持续维护：

```text
docs/execution/PROVIDER_BUILDER_MASTER_PLAN.md
docs/execution/PROVIDER_BUILDER_STATUS.md
docs/execution/PROVIDER_BUILDER_DECISIONS.md
docs/execution/PROVIDER_BUILDER_ACCEPTANCE.md
docs/execution/PROVIDER_BUILDER_BLOCKERS.md
docs/execution/PROVIDER_BUILDER_KNOWN_LIMITATIONS.md
```

`PROVIDER_BUILDER_STATUS.md` 必须包含：

```text
当前 Milestone
已完成内容
正在进行内容
下一步
最近 Rust 测试
最近 Web 测试
最近 E2E 测试
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

# 三、Provider、Model、Skill、Node 和 Tool 的严格边界

必须在代码和文档中固定以下定义。

## 3.1 Provider

Provider 只表示 LLM/VLM API 服务连接。

它负责：

* 服务名称；
* API 协议；
* Base URL；
  -账户或 Workspace；
  -凭证引用；
  -安全 Header；
  -连接策略；
  -超时和限流；
  -健康状态；
  -该 Provider 下的模型发现。

示例：

```text
Alibaba DashScope
OpenAI
OpenRouter
Gemini compatible endpoint
Custom OpenAI-compatible endpoint
Local vLLM compatible endpoint
Mock
```

Provider 不表示某个具体模型。

## 3.2 Model Profile

Model Profile 表示一个可实际绑定和调用的模型。

例如：

```text
Provider: Alibaba DashScope
Model Profile: qwen3.7-flash
```

Model Profile 负责：

-远程模型 ID；
-显示名称；
-输入模态；
-协议能力；
-任务能力；
-上下文限制；
-输出限制；
-工具调用支持；
-结构化输出支持；
-图像输入支持；
-价格；
-默认推理参数；
-健康状态；
-版本；
-模型身份。

用户在 Project 或 Workflow 中实际选择的是 Model Profile。

## 3.3 Skill

Skill 提供一类任务知识、模板、Validator 或领域策略。

例如：

```text
Classification Skill
Detection Skill
Segmentation Skill
robocup.ball Domain Skill
```

Skill 不保存 API Key。

Skill 不等于 Provider。

Skill 不等于模型品牌。

## 3.4 Node

Node 是 Workflow 中可执行的一步。

例如：

```text
Detect
Crop
Classify
Validate
Decision
Review
Commit
```

Node 可以要求某种模型 capability，并通过 Model Binding 解析具体 Model Profile。

## 3.5 Agent Tool

Agent Tool 是 Pipeline Builder Agent 用来检查系统、修改 Draft、执行校验和试跑的受控动作。

例如：

```text
list_compatible_models
add_pipeline_node
validate_pipeline
dry_run_pipeline
```

Agent Tool 不等于 Runtime Node。

必须避免把同一个概念同时叫 Provider、Model、Skill 和 Tool。命名混乱不会创造扩展性，只会创造一套需要考古才能配置的系统。

---

# 四、Provider 数据模型

在 Core 中新增或整理：

```rust
pub struct ProviderProfile {
    pub id: ProviderId,
    pub display_name: String,
    pub preset_id: Option<String>,
    pub adapter: ProviderAdapterKind,
    pub base_url: Url,
    pub organization: Option<String>,
    pub workspace: Option<String>,
    pub credential_ref: Option<CredentialReference>,
    pub safe_headers: BTreeMap<String, String>,
    pub connection_policy: ProviderConnectionPolicy,
    pub enabled: bool,
    pub health: ProviderHealthSnapshot,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

适配器类型：

```rust
pub enum ProviderAdapterKind {
    OpenAiCompatible,
    Mock,
}
```

第一阶段不要为每个厂商创建独立 Runtime 分支。

以下供应商可以作为 UI Preset：

```text
Alibaba DashScope / Qwen
OpenAI
OpenRouter
Google Gemini OpenAI-compatible
Custom OpenAI-compatible
Local OpenAI-compatible
Mock
```

Preset 只负责预填：

* Adapter；
* Base URL；
  -推荐 Header；
  -文档提示；
  -默认模型名称候选。

禁止：

```rust
if provider_name == "qwen" { ... }
```

运行时必须根据 Adapter 和 Model Profile 工作。

---

# 五、Provider 连接策略

```rust
pub struct ProviderConnectionPolicy {
    pub request_timeout_seconds: u64,
    pub maximum_retries: u32,
    pub maximum_concurrency: u32,
    pub minimum_retry_delay_ms: u64,
    pub maximum_retry_delay_ms: u64,
    pub allow_remote_http: bool,
    pub allowed_redirects: u32,
}
```

安全规则：

* 外部 Provider 默认必须使用 HTTPS；
* `http://127.0.0.1` 和 `http://localhost` 可以用于本地兼容服务；
* URL 内不得包含用户名和密码；
  -禁止危险重定向；
* Header 名称必须经过白名单校验；
* `Authorization` 由后端根据 CredentialReference 注入；
  -前端不能提交任意 Authorization Header；
  -请求和响应有大小上限；
  -超时、重试和取消必须可用；
* Provider 错误必须结构化。

---

# 六、Credential Store

当前新实现不应继续把新 API Key 默认保存到普通工作区文件。

设计统一接口：

```rust
#[async_trait]
pub trait SecretStore: Send + Sync {
    async fn put(
        &self,
        scope: SecretScope,
        secret: SecretValue,
    ) -> Result<CredentialReference, SecretStoreError>;

    async fn resolve(
        &self,
        reference: &CredentialReference,
    ) -> Result<SecretValue, SecretStoreError>;

    async fn delete(
        &self,
        reference: &CredentialReference,
    ) -> Result<(), SecretStoreError>;

    async fn exists(
        &self,
        reference: &CredentialReference,
    ) -> Result<bool, SecretStoreError>;
}
```

支持：

```rust
pub enum CredentialSource {
    SystemKeyring,
    EnvironmentVariable,
    SessionOnly,
    LegacyWorkspaceFile,
}
```

要求：

1. 新 GUI 默认使用系统 Keyring；
2. 使用跨平台 `keyring` 或现有可靠实现；
3. macOS 对应 Keychain；
4. CI 和单元测试使用 InMemory Secret Store；
5. 环境变量引用继续支持；
6. Session-only Key 进程退出即失效；
7. Legacy Workspace File 只用于读取旧配置和显式迁移；
8. 不自动迁移旧 Secret；
9. 用户必须明确点击迁移；
10. 数据库只保存 CredentialReference；
11. API 只返回：

    ```text
    credential_configured: true/false
    ```
12. API 不返回 Secret；
    13.日志不显示 Secret；
    14.错误信息不包含 Secret；
    15.前端不写 localStorage；
    16.前端不把 Secret 放进 URL；
    17.历史导出不包含 Secret；
13. Workflow Version 不包含 Secret。

提供：

```text
Migrate credential to system keychain
Rotate credential
Remove credential
Use environment variable instead
Use for this session only
```

---

# 七、Model Profile 数据模型

```rust
pub struct ModelProfile {
    pub id: ModelProfileId,
    pub revision: u64,
    pub provider_id: ProviderId,
    pub display_name: String,
    pub remote_model_id: String,
    pub input_modalities: BTreeSet<InputModality>,
    pub protocol_features: ProtocolFeatures,
    pub task_capabilities: BTreeSet<ModelCapability>,
    pub limits: ModelLimits,
    pub generation_defaults: GenerationDefaults,
    pub pricing: ModelPricing,
    pub status: ModelProfileStatus,
    pub enabled: bool,
    pub locked: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

输入模态：

```rust
pub enum InputModality {
    Text,
    Image,
    Video,
}
```

协议能力：

```rust
pub struct ProtocolFeatures {
    pub tool_calls: bool,
    pub parallel_tool_calls: bool,
    pub structured_output: bool,
    pub json_schema: bool,
    pub usage_reporting: bool,
    pub streaming: bool,
    pub reasoning_controls: bool,
}
```

任务能力：

```rust
pub enum ModelCapability {
    TextGeneration,
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

限制：

```rust
pub struct ModelLimits {
    pub context_tokens: Option<u64>,
    pub maximum_output_tokens: Option<u64>,
    pub maximum_images_per_request: Option<u32>,
    pub maximum_image_pixels: Option<u64>,
}
```

价格：

```rust
pub struct ModelPricing {
    pub currency: String,
    pub input_per_million_tokens: Option<Decimal>,
    pub output_per_million_tokens: Option<Decimal>,
    pub cached_input_per_million_tokens: Option<Decimal>,
    pub per_image: Option<Decimal>,
    pub per_request: Option<Decimal>,
    pub source: PricingSource,
    pub updated_at: Option<DateTime<Utc>>,
}
```

价格来源：

```rust
pub enum PricingSource {
    UserConfigured,
    ProviderDiscovered,
    Preset,
    Unknown,
}
```

不要声称 Preset 价格永远有效。

---

# 八、Provider 与 Model 生命周期

## 8.1 Provider 创建

流程：

```text
Choose preset
→ Name provider
→ Configure endpoint
→ Configure credential
→ Validate connection settings
→ Discover or add models
→ Save
```

一个用户可以配置多个同厂商 Provider：

```text
Qwen Personal
Qwen Lab Account
OpenRouter Test
Local vLLM
```

不得用厂商名作为唯一 ID。

## 8.2 Model Discovery

支持：

```text
Discover models
Add model manually
```

若 Provider 支持兼容的 `/models`：

* 获取模型 ID；
  -用户选择导入；
  -不自动假设能力；
  -根据 Preset 提供建议；
  -标记为未验证。

若不支持：

-允许手动填写 remote model ID；
-手动配置 capabilities；
-所有人工填写能力标记：

```text
user_declared
```

## 8.3 Provider 健康状态

```rust
pub enum ProviderHealthStatus {
    Unknown,
    Configured,
    Available,
    Unreachable,
    InvalidCredential,
    RateLimited,
    IncompatibleProtocol,
    Disabled,
}
```

区分两类测试。

### Passive Check

尽量不产生收费请求：

* URL 解析；
* DNS/连接；
* Credential 是否存在；
* `/models`，若可用；
  -协议响应格式；
  -证书；
  -超时。

### Active Probe

执行一次最小模型调用：

-可能计费；
-必须明确提示；
-必须由用户主动触发；
-记录 Token 和费用；
-不能在页面加载时自动执行；
-不能由 Pipeline Builder Agent 未经批准触发。

按钮：

```text
Check connection
Run billable model test
```

不得把两者都写成含义模糊的 `Test`。

---

# 九、Provider 和 Model 删除规则

删除 Provider 前检查：

* 是否有 Model Profile；
  -是否被 Project Model Binding 引用；
  -是否被 Draft 引用；
  -是否被 Published Workflow Version 引用。

若仍被引用：

-阻止删除；
-显示引用位置；
-允许先 Disable；
-提供 Rebind 操作。

Published Workflow 的历史快照不能因 Provider 删除而消失。

删除 Provider 不得删除历史 Run、Artifact 或 Usage。

---

# 十、Model Profile 版本与可复现性

以下字段变化会影响推理语义：

* remote model ID；
* Base URL；
* generation defaults；
* structured output 模式；
* reasoning mode；
  -图像参数；
* capability；
  -系统 Prompt 默认值。

这些变化必须创建新的 Model Profile revision，或者要求显式确认迁移。

API Key 轮换不改变 Model Profile revision。

价格变化不改变模型语义，但每次 Run 必须保存实际使用的价格快照。

Published Workflow Version 至少固定：

```text
ModelProfileId
ModelProfile revision
Provider adapter
Remote model ID
Generation defaults
Prompt version
Skill version
Node configuration
```

不得保存 Secret。

---

# 十一、Project Model Binding

Project 不直接保存裸 Provider 名称或裸模型字符串。

实现：

```rust
pub struct ProjectModelBinding {
    pub id: ModelBindingId,
    pub project_id: ProjectId,
    pub capability: ModelCapability,
    pub role: ModelBindingRole,
    pub model_profile_id: ModelProfileId,
    pub locked: bool,
    pub created_at: DateTime<Utc>,
}
```

角色：

```rust
pub enum ModelBindingRole {
    PipelineBuilder,
    PrimaryInference,
    Detection,
    Classification,
    Segmentation,
    Verification,
    Fallback,
}
```

优先级：

```text
Workflow Node explicit binding
> Project capability binding
> Project role binding
> Global default
```

不得根据 Provider 名称猜测模型。

如果没有兼容 Model Profile：

```text
Unresolved model binding
```

而不是由 Agent 编造一个模型 ID。

---

# 十二、Provider Fallback 与 Workflow Fallback 必须分开

## Provider Fallback

只处理基础设施问题：

```text
timeout
rate limit
provider unavailable
temporary server failure
```

例如：

```text
Qwen primary
→ timeout
→ OpenRouter fallback
```

## Workflow Fallback

处理语义问题：

```text
no detection
low confidence
conflicting evidence
domain validation issue
```

例如：

```text
RF-DETR empty
→ LocateAnything
```

禁止把二者混在同一个 `fallback` 字段中。

第一阶段 Provider Route 可以支持：

```rust
pub struct ProviderRoute {
    pub primary: ModelProfileId,
    pub fallbacks: Vec<ModelProfileId>,
    pub fallback_on: BTreeSet<InfrastructureFailureKind>,
    pub maximum_fallbacks: u32,
}
```

语义 fallback 仍由 Workflow 的 Decision 节点处理。

---

# 十三、Settings 信息架构

Settings 调整为：

```text
Providers
Models
Vision Workers
Storage
Usage
```

## 13.1 Providers

显示 Provider 卡片：

```text
Display name
Preset/vendor
Endpoint host
Credential configured
Model count
Health
Last checked
Recent usage
Enabled / disabled
```

操作：

```text
Add provider
Edit
Check connection
Run billable test
Rotate credential
Disable
Delete
```

## 13.2 Provider Detail

页面分区：

```text
Connection
Credential
Models
Limits
Usage
Advanced
```

默认隐藏：

-完整 Header；
-原始 JSON；
-协议调试；
-历史错误。

## 13.3 Models

汇总所有 Model Profiles。

支持筛选：

```text
Provider
Capability
Input modality
Health
Enabled
Cost status
```

## 13.4 Vision Workers

YOLO、SAM、RF-DETR、LocateAnything 等本地或远程 CV Worker 继续作为 Vision Worker / Model Backend 管理。

它们不属于本次 LLM/VLM Provider 管理。

不得把 API Provider 和 GPU Worker 混成一种凭证模型。

---

# 十四、Project 中选择已配置模型

在：

```text
Project → Build → Automation
```

用户只能选择已经配置和启用的 Model Profile。

下拉框按 Provider 分组：

```text
Alibaba DashScope
  qwen3.7-flash
  qwen-max

OpenRouter
  model-x
  model-y
```

每个模型显示：

```text
Capabilities
Health
Vision support
Tool calls
Structured output
Pricing status
```

只显示与当前 Node 兼容的模型。

例如 Pipeline Builder Agent 模型必须至少支持：

```text
TextGeneration
ToolCalls
StructuredOutput
```

VLM Detection 节点必须至少支持：

```text
Image input
VisionLanguage
StructuredOutput 或 ToolCalls
```

用户可以锁定绑定：

```text
Lock this model choice
```

Pipeline Builder Agent 不得修改锁定绑定。

没有兼容模型时，显示：

```text
No compatible model configured

[Connect a provider]
[Add a model]
```

---

# 十五、Pipeline Builder Agent 只能选择已配置模型

Agent 可以读取：

* Provider 显示名称；
* Model Profile；
* capabilities；
  -健康状态；
  -价格；
  -延迟统计；
  -限制；
  -是否锁定。

Agent不能读取：

* API Key；
* Secret 值；
* Authorization Header；
  -系统 Keychain 内容；
* Provider 管理操作权限。

Agent不能：

-创建 Provider；
-修改 Endpoint；
-保存 API Key；
-删除 Provider；
-启用远程 URL；
-执行收费 Active Probe；
-绕过用户锁定模型；
-绑定 Registry 中不存在的模型。

如果缺少模型，Agent 返回：

```text
Provider setup required
```

并给出 capability 要求：

```text
需要一个支持图像输入和结构化输出的 Vision Language Model
```

---

# 十六、LLM 可拼接的节点目录

不要把所有底层函数都暴露为 Workflow Node。

当前 Alpha 的用户可组合节点目录收敛为以下内容。

## 16.1 输入

### `core.image_input`

输入图片，输出：

```text
ImageArtifact
```

### `core.existing_annotations`

可选读取已有标注，用于：

-增量标注；
-复核；
-精修。

输出：

```text
AnnotationCandidateSet
```

## 16.2 图像准备

### `core.resize`

按最大边、目标尺寸或像素预算缩放。

### `core.tile`

将大图分块，用于小目标检测。

配置：

```text
tile size
overlap
maximum tiles
merge policy
```

### `core.crop`

根据 Detection 或 Region 生成 CropSet。

必须保存 parent reference 和坐标映射。

## 16.3 模型推理

### `capability.detect`

根据绑定模型执行：

```text
Object Detection
Open-vocabulary Detection
Phrase Grounding
```

输出：

```text
DetectionSetArtifact
```

### `capability.classify`

支持：

```text
Whole-image classification
Crop classification
Candidate verification
Attribute classification
```

输出：

```text
ClassificationSetArtifact
```

### `capability.segment`

支持：

```text
Semantic segmentation
Prompted segmentation
Instance segmentation
```

输出：

```text
Mask / Polygon Artifact
```

节点只有在存在兼容模型时可用。

## 16.4 结果转换

### `core.select_and_map`

Guided UI 中统一表示：

```text
Select results
```

内部支持：

-按模型 Label 筛选；
-映射为 Project Label；
-按 score 筛选；
-按 Geometry 筛选；
-按 query 筛选；
-去除未知 Label。

### `core.project_coordinates`

将 Crop、Tile 或局部模型输出重新投影到原图坐标。

### `core.attach_result`

把分类或属性结果关联回父 Detection、Mask 或 Annotation Candidate。

不得依靠数组位置关联。

## 16.5 证据和验证

### `core.combine_evidence`

内部可以复用：

```text
Candidate Match
Candidate Merge
IoU matching
NMS
Deduplication
```

Guided UI 只显示：

```text
Combine model evidence
```

### `core.validate`

运行：

-通用 Geometry Validator；
-Schema Validator；
-Domain Skill Validator；
-模型输出完整性检查。

### `core.decision`

统一表示：

```text
Auto-accept rule
```

内部支持：

```rust
pub enum DecisionMode {
    Confidence,
    Evidence,
    DomainPolicy,
}
```

输出：

```text
accept
review
reject
fallback
```

## 16.6 人工和输出

### `core.human_review`

将结果送入 Review Queue。

### `core.commit`

唯一允许写入正式 Annotation 的节点。

Commit 前必须经过：

* Rust Validator；
* Decision；
  -或 Human Approval。

---

# 十七、不应作为可拼接节点的能力

以下属于 Runtime 横切能力，不应让 LLM 当作方框插入：

```text
Cache
Replay
Retry
Timeout
Budget
Usage tracking
Checkpoint
Pause
Resume
Cancel
History
```

它们应作为：

* Node Policy；
* Workflow Policy；
* Runtime Policy。

以下属于 Project 操作，不属于 Annotation Workflow：

```text
Export
Provider setup
Model registration
Credential rotation
Training
```

以下属于 UI，不属于 Workflow：

```text
Artifact Inspector
Run Results
Review Canvas
```

这样可以避免 Node Catalog 再次膨胀成人类无法阅读的积木仓库。

---

# 十八、Node Definition 契约

每个可组合节点必须注册：

```rust
pub struct NodeDefinition {
    pub id: NodeDefinitionId,
    pub display_name: String,
    pub category: NodeCategory,
    pub input_ports: Vec<PortDefinition>,
    pub output_ports: Vec<PortDefinition>,
    pub config_schema: JsonSchema,
    pub required_model_capability: Option<ModelCapability>,
    pub cardinality: NodeCardinality,
    pub side_effect: NodeSideEffect,
    pub dry_run_supported: bool,
    pub expert_only: bool,
}
```

端口：

```rust
pub struct PortDefinition {
    pub name: String,
    pub artifact_type: ArtifactType,
    pub required: bool,
    pub cardinality: PortCardinality,
}
```

副作用：

```rust
pub enum NodeSideEffect {
    None,
    HumanSuspension,
    AnnotationCommit,
}
```

LLM 只能从 Node Registry 读取并实例化这些节点。

---

# 十九、Pipeline Builder Agent 工具

Pipeline Builder Agent 的 Tool Catalog 分为四组。

## 19.1 检查 Project

```text
inspect_project
inspect_label_schema
inspect_label
sample_dataset
inspect_sample_image
inspect_existing_automations
```

## 19.2 检查能力和模型

```text
list_enabled_skills
load_skill_resource

list_node_definitions
inspect_node_definition
list_pipeline_templates

list_provider_profiles
list_compatible_models
inspect_model_profile
check_provider_availability
estimate_model_cost
```

其中：

* `list_provider_profiles` 不返回 Secret；
* `inspect_model_profile` 不返回 Secret；
* `check_provider_availability` 只执行 Passive Check；
* Agent 不得调用 Billable Active Probe。

## 19.3 修改 Draft

```text
create_pipeline_draft
create_draft_from_template

add_pipeline_node
remove_pipeline_node
connect_pipeline_nodes
disconnect_pipeline_nodes

set_node_configuration
bind_model_profile
set_label_mapping
set_decision_policy
set_runtime_policy

compare_pipeline_drafts
undo_last_draft_change
```

所有修改必须作用于真实持久化 Draft。

禁止提供一个不受约束的：

```text
replace_entire_workflow_json
```

禁止允许 LLM 直接写数据库。

## 19.4 校验、试跑和结束

```text
validate_pipeline
estimate_pipeline_cost

dry_run_pipeline
inspect_dry_run_summary
inspect_failed_samples
inspect_review_samples
inspect_node_statistics
inspect_node_artifacts

submit_draft_for_human_approval
finish_agent_session
```

Agent 不得拥有：

```text
publish_pipeline
start_full_dataset_run
set_api_key
create_provider
delete_provider
execute_shell
execute_python
download_model
open_arbitrary_url
```

---

# 二十、Pipeline Builder Agent 标准循环

必须实现真实多轮循环：

```text
1. inspect_project
2. inspect_label_schema
3. list_enabled_skills
4. list_compatible_models
5. list_pipeline_templates
6. 创建 Draft
7. validate_pipeline
8. 根据错误修改 Draft
9. 再次 validate_pipeline
10. dry_run_pipeline
11. inspect_dry_run_summary
12. 查看有限数量的失败或 Review 样本
13. 修改模型绑定、阈值或节点
14. 再次 Dry Run
15. submit_draft_for_human_approval
```

至少一个集成测试必须稳定复现：

```text
第一次 Draft：
VLM Detection 节点绑定了一个不支持 Image Input 的模型

Rust Validator：
返回 incompatible_model_capability

Agent：
查询兼容模型
重新绑定 VLM Model

第二次校验：
通过

Dry Run：
Review rate 过高

Agent：
增加 Crop Classification 或调整 Decision

第二次 Dry Run：
结果改善

Agent：
提交人工审批
```

如果 Agent 只调用一次 LLM 并返回 JSON，不得称为 Agent Loop。

---

# 二十一、Workflow Grammar 限制

当前 Alpha 不允许任意无限 DAG。

允许的高层语法：

```text
Input
→ Optional image preparation
→ One or more model inference stages
→ Select / Map / Attach
→ Optional evidence combination
→ Validation
→ Decision
    ├── Commit
    ├── Review
    ├── Reject
    └── Bounded fallback
```

静态规则：

1. 必须是 DAG；
2. 不允许循环；
3. Commit 前必须经过 Validate 和 Decision，或人工批准；
4. 不确定分支必须到 Review、Reject 或受限 fallback；
5. 模型节点必须绑定兼容 Model Profile；
6. Model Profile 必须启用；
7. Provider Credential 必须存在；
8. Skill 必须启用；
9. 输入输出 Artifact 类型必须兼容；
10. Crop 结果必须能投影回原图；
11. Published Workflow 不允许 unresolved binding；
12. Fallback 最大两层；
13. 单图最大模型调用数必须可估算；
14. 费用超过 Project 硬约束时不得发布；
15. Side-effect Node 只能是 Review 或 Commit；
16. Dry Run 中 Commit 必须使用 Sandbox Commit。

---

# 二十二、模型选择策略

Agent 为节点选择模型时，应按以下条件过滤：

```text
Required capability
Input modality
Protocol feature
Provider enabled
Credential configured
Model enabled
Health
Project privacy constraint
Project cost constraint
Project latency constraint
User-locked binding
```

然后才可以排序。

建议排序信息：

```text
Compatibility
Availability
User preference
Historical latency
Estimated cost
Context limit
Image support
Usage reporting
```

Agent 必须说明结构化理由：

```text
Selected qwen3.7-flash because:
- supports image input
- supports structured output
- configured credential exists
- estimated cost fits the project limit
```

不得声称某模型“更准确”，除非项目中存在真实评测数据。

---

# 二十三、Provider 与模型默认设置

支持：

## 全局默认

```text
Default Pipeline Builder Model
Default Vision Language Model
Default Text Model
```

## Project 默认

```text
Detection model
Classification model
Verification model
Pipeline Builder model
```

## Node override

具体节点绑定指定 Model Profile。

默认只是一种选择优先级，不改变 Workflow 的不可变性。

发布时最终绑定必须明确冻结。

---

# 二十四、Guided Experience

Provider 管理不能迫使用户先学习 Model Registry。

## 24.1 首次需要模型时

如果 Project 没有兼容模型：

```text
A vision model is required to test this automation.

[Connect a provider]
```

点击后打开内嵌向导：

```text
Choose provider preset
→ Enter API key
→ Select or add model
→ Check connection
→ Return to automation
```

返回后保留 Draft 和页面状态。

## 24.2 已配置 Provider

用户在 Automation 节点中看到：

```text
Model choice

qwen3.7-flash
Alibaba DashScope
Vision · Structured output · Tool calls
```

而不是：

```text
Endpoint
API Key
Header
Remote Model ID
```

高级配置进入 Expert Mode。

## 24.3 Agent 模型选择

启动 Pipeline Builder Agent 前显示：

```text
Agent model

qwen3.7-flash via Alibaba DashScope
```

允许用户更换为其他兼容 Model Profile。

Agent 不得静默更换锁定模型。

---

# 二十五、Provider Usage 与费用

每次调用必须保存：

```text
Provider Profile ID
Model Profile ID
Model Profile revision
Project
Workflow Version
Node
Agent Session
Input tokens
Output tokens
Cached tokens
Image count
Request count
Duration
Cost
Usage source
Request ID
Status
Retry count
```

Settings → Usage 支持按以下维度查看：

```text
Provider
Model
Project
Agent
Workflow
Date
```

Provider Detail 显示：

```text
Today
This month
Recent errors
Rate-limit events
Average latency
```

不得用估算数据伪装 Provider 返回的实际用量。

明确区分：

```text
Actual
Estimated
Unknown
```

---

# 二十六、API

实现或整理：

```text
GET    /api/provider-presets

GET    /api/providers
POST   /api/providers
GET    /api/providers/:providerId
PATCH  /api/providers/:providerId
DELETE /api/providers/:providerId

POST   /api/providers/:providerId/credential
DELETE /api/providers/:providerId/credential
POST   /api/providers/:providerId/migrate-credential

POST   /api/providers/:providerId/check
POST   /api/providers/:providerId/active-probe
POST   /api/providers/:providerId/discover-models

GET    /api/model-profiles
POST   /api/model-profiles
GET    /api/model-profiles/:modelId
PATCH  /api/model-profiles/:modelId
DELETE /api/model-profiles/:modelId

GET    /api/model-profiles/compatible
GET    /api/model-profiles/:modelId/usage

GET    /api/projects/:projectId/model-bindings
PUT    /api/projects/:projectId/model-bindings

GET    /api/agent-model-bindings
PUT    /api/agent-model-bindings
```

要求：

-所有输出 DTO 都不包含 Secret；

* Credential API 返回配置状态；
* Active Probe 明确标记可能计费；
  -删除有引用的 Provider 返回 `409 Conflict`；
  -不兼容模型绑定返回结构化错误；
* API 错误包含修复建议。

---

# 二十七、错误模型

```rust
pub enum ProviderErrorCode {
    InvalidEndpoint,
    MissingCredential,
    InvalidCredential,
    Unreachable,
    Timeout,
    RateLimited,
    IncompatibleProtocol,
    ModelNotFound,
    UnsupportedCapability,
    ResponseTooLarge,
    InvalidResponse,
    Cancelled,
}
```

```rust
pub struct ProviderErrorDetails {
    pub code: ProviderErrorCode,
    pub provider_id: ProviderId,
    pub model_profile_id: Option<ModelProfileId>,
    pub operation: String,
    pub recoverable: bool,
    pub retry_after_ms: Option<u64>,
    pub safe_message: String,
}
```

错误消息不得包含：

* API Key；
* Authorization Header；
  -完整请求 body；
  -完整图片 base64。

GUI 不得只显示：

```text
Provider failed
```

应显示：

```text
Alibaba DashScope rejected the configured credential.

[Update credential]
```

或者：

```text
qwen3.7-flash does not support the tool-calling features required by Pipeline Builder.

[Choose another model]
```

---

# 二十八、TUI

TUI 保留现有 Run 和 Artifact 能力，并增加：

```text
/providers
/providers show <id>
/providers check <id>

/models
/models show <id>
/models compatible <capability>

/bindings
/bind <role> <model-profile-id>

/advisor
/advisor cancel
```

要求：

* TUI 不显示 Secret；
* TUI 不打印 Authorization；
* Provider 状态使用文字和颜色；
* Agent Trace 显示使用了哪个 Provider 和 Model；
  -模型选择支持键盘；
  -如果 TUI 不实现 Secret 输入，应明确引导用户使用 GUI 或环境变量；
  -不得创建一个会在终端回显 API Key 的普通输入框。

---

# 二十九、迁移现有 Provider 配置

当前已有 Provider Selector 和 `default-vision` Model Binding。

必须实现迁移：

```text
旧 Provider 配置
→ ProviderProfile

旧 model 字符串
→ ModelProfile

旧 default-vision
→ ProjectModelBinding

旧 Secret 文件引用
→ Legacy CredentialReference
```

要求：

1. 现有 Project 仍可打开；
2. 现有 Published Workflow 仍可查看；
3. 如果旧模型信息足够，可以自动创建 Model Profile；
4. 如果能力无法确定，标记 `user_declared` 或 `unknown`；
5. 不自动移动 Secret；
6. 显式提供迁移到 Keychain；
7. 历史 Run 不修改；
8. migration 可重复运行；
9. migration 有 rollback-safe transaction；
10. migration 有测试。

---

# 三十、Agent 安全边界

Pipeline Builder Agent 必须遵守：

1. 只能调用注册 Tool；
2. 只能读取脱敏 Provider 和 Model metadata；
3. 不能读取 Secret；
4. 不能创建 Provider；
5. 不能修改 Credential；
6. 不能执行 Active Probe；
7. 不能生成任意代码；
8. 不能执行 Shell；
9. 不能访问任意 URL；
10. 不能绑定未知模型；
11. 不能绑定未启用模型；
12. 不能修改锁定 Binding；
13. 不能修改 Published Version；
14. 不能自动 Publish；
15. 不能自动启动完整 Batch；
16. 不能静默放宽预算；
17. 不能把图片中的文字当成系统指令；
18. 不能跨 Project 访问 Provider 使用限制之外的数据；
19. 不能展示隐藏思维链；
20. 所有 Tool Call 和结果必须可审计。

---

# 三十一、测试要求

## 31.1 Provider 单元测试

覆盖：

* Provider ID；
  -多个同厂商 Provider；
* Base URL；
* HTTPS 限制；
* loopback HTTP；
  -安全 Header；
* CredentialReference；
* Secret masking；
* Keyring Store Mock；
* Environment Variable Store；
* Session Store；
* Legacy migration；
* Provider health；
* Provider disable；
* Provider deletion conflict。

## 31.2 Model Profile 测试

覆盖：

* Model revision；
* capabilities；
* protocol features；
* input modalities；
* pricing；
* unknown pricing；
* manual capability；
* incompatible binding；
* locked binding；
* provider disabled；
* missing credential；
* model deletion conflict。

## 31.3 Agent Tool 测试

覆盖：

-列出兼容模型；
-无兼容模型；
-检查 Provider availability；
-未知 Provider；
-未知 Model；
-绑定不兼容模型；
-绑定锁定模型；
-创建 Draft；
-添加节点；
-非法端口；
-校验；
-Dry Run；
-根据结果修订；
-提交人工审批。

## 31.4 Agent Loop 集成测试

完整测试：

```text
Project: football bounding box

Configured:
- text-only model
- vision-language model

Agent first binds text-only model to detection
→ Rust Validator rejects

Agent calls list_compatible_models
→ binds vision-language model

Dry Run
→ review count too high

Agent adds Crop + Classification
→ validates
→ runs second Dry Run
→ submits draft for human approval

Agent never publishes
```

## 31.5 Secret 安全测试

检查：

* API 响应；
  -日志；
* SQLite；
  -历史导出；
* Workflow Version；
  -Run Event；
  -Agent Tool Result；
  -错误消息；
  -Web localStorage；
  -URL。

任何位置都不得出现 Secret 明文。

## 31.6 Web E2E

覆盖：

1. 添加 Qwen Provider；
2. 保存 Key 到测试 Secret Store；
3. 添加 Model Profile；
4. 检查 Provider；
5. Project 选择配置好的模型；
6. 兼容模型筛选；
7. 缺少兼容模型时内嵌设置；
8. Agent 使用选中的模型；
9. Agent不能看到 Secret；
10. Provider Disable 后阻止新 Run；
11. Provider 删除引用冲突；
12. Key rotation；
    13.刷新后配置恢复；
13. Generic Project 无 RoboCup；
14. 1024px 无溢出。

---

# 三十二、Milestone 计划

## Milestone 0：基线和迁移设计

完成：

-核验现有 Provider；
-核验 Secret；
-核验 Model Binding；
-列出当前 API；
-列出迁移方案；
-建立状态文档；
-建立测试基线。

提交：

```text
docs: establish provider registry and builder baseline
```

## Milestone 1：Provider 与 Secret Store

完成：

* ProviderProfile；
* Adapter；
* Connection Policy；
* SecretStore；
* Keyring；
* Environment；
* Session；
* Legacy reference；
* migrations；
  -单元测试。

提交：

```text
feat(provider): add reusable provider profiles and secure credentials
```

## Milestone 2：Model Profile 与 Binding

完成：

* ModelProfile；
* capabilities；
* protocol features；
* pricing；
* revision；
* Project bindings；
* Agent binding；
  -兼容性查询；
* migrations；
  -测试。

提交：

```text
feat(models): add reusable model profiles and capability bindings
```

## Milestone 3：Provider API 和 GUI

完成：

* Provider CRUD；
* Credential；
* Passive Check；
* Active Probe；
* Model Discovery；
* Settings UI；
  -引用保护；
* Usage；
  -E2E。

提交：

```text
feat(settings): manage llm and vlm providers from one registry
```

## Milestone 4：Node Catalog 收敛

完成：

-整理 Node Definition；
-添加 Resize、Tile、Coordinate Projection；
-合并 Guided Select & Map；
-合并 Guided Decision；
-合并 Guided Evidence；
-区分 Runtime Policy；
-更新文档和测试。

提交：

```text
refactor(workflow): expose a constrained annotation node catalog
```

## Milestone 5：Pipeline Builder Agent Tools

完成：

* Provider/Model discovery tools；
* Node discovery tools；
* Draft mutation tools；
* Validation tools；
* Dry Run tools；
  -人工审批工具；
  -权限；
  -审计；
  -安全测试。

提交：

```text
feat(agent): let the builder inspect providers and edit real drafts
```

## Milestone 6：真实 LLM Tool Loop

完成：

* OpenAI-compatible Agent Provider；
  -正确 Tool Call 历史；
  -上下文管理；
  -模型选择；
  -校验修订；
  -Dry Run 修订；
  -预算；
  -取消；
  -停止条件；
  -Scripted Mock；
  -真实 Provider smoke test，若可用。

提交：

```text
feat(agent): build and revise pipelines through constrained llm tools
```

## Milestone 7：Project Guided UX 与 TUI

完成：

* Project 模型选择；
  -内嵌 Provider 配置；
  -Agent model selector；
  -Agent progress；
  -Draft Diff；
  -TUI Provider/Model 命令；
  -刷新恢复；
  -无障碍。

提交：

```text
feat(ui): guide provider selection and agent-built automations
```

## Milestone 8：迁移、回归与 Release

完成：

-旧 Provider 迁移；
-旧 Model Binding 迁移；
-旧 Secret 显式迁移；
-所有现有 Project 回归；
-Run、Review、Replay、Batch、Export 回归；
-文档；
-课程演示；
-Release Matrix。

提交：

```text
test(release): validate provider registry and pipeline builder alpha
```

---

# 三十三、Release Blocking Acceptance Matrix

以下全部满足后才能声称 Alpha 完成。

## A. Provider

* [ ] Provider 与 Model 是独立实体。
* [ ] 一个 Provider 可以包含多个 Model Profiles。
* [ ] 可以配置多个同厂商 Provider。
* [ ] Provider Preset 不产生 Runtime 厂商分支。
* [ ] Provider 可以启用、禁用和检查。
* [ ] Passive Check 不自动产生收费调用。
* [ ] Active Probe 需要用户明确确认。
* [ ] Provider 删除会检查引用。
* [ ] Provider 不可用不会使 AnnotAgent 无法启动。

## B. Secret

* [ ] 新 Secret 默认不写普通工作区文件。
* [ ] 支持系统 Keyring。
* [ ] 支持环境变量引用。
* [ ] 支持 Session-only。
* [ ] 旧 Secret 需要显式迁移。
* [ ] 数据库没有 Secret。
* [ ] API 不返回 Secret。
* [ ] 日志不包含 Secret。
* [ ] 历史导出不包含 Secret。
* [ ] 前端 localStorage 不包含 Secret。

## C. Model Profile

* [ ] 用户实际选择 Model Profile。
* [ ] Model Profile 记录 Provider。
* [ ] 记录输入模态。
* [ ] 记录协议能力。
* [ ] 记录任务能力。
* [ ] 记录价格。
* [ ] 记录限制。
* [ ] 记录 revision。
* [ ] Published Workflow 固定 Model Profile revision。
* [ ] Key rotation 不改变 Workflow Version。

## D. Project Binding

* [ ] Project 可以选择配置好的模型。
* [ ] 只显示兼容模型。
* [ ] 支持全局默认。
* [ ] 支持 Project 默认。
* [ ] 支持 Node override。
* [ ] 支持锁定 Binding。
* [ ] Agent不能修改锁定 Binding。
* [ ] 无兼容模型时返回 unresolved binding。

## E. Node Catalog

* [ ] LLM 只能使用注册节点。
* [ ] 支持 Image Input。
* [ ] 支持 Resize。
* [ ] 支持 Tile。
* [ ] 支持 Crop。
* [ ] 支持 Detect。
* [ ] 支持 Classify。
* [ ] 支持 Segment。
* [ ] 支持 Select & Map。
* [ ] 支持 Coordinate Projection。
* [ ] 支持 Attach Result。
* [ ] 支持 Combine Evidence。
* [ ] 支持 Validate。
* [ ] 支持 Decision。
* [ ] 支持 Human Review。
* [ ] 支持 Commit。
* [ ] Cache、Replay、Retry 和 Budget 不作为普通节点。

## F. Agent

* [ ] Agent 使用真实 Tool Calls。
* [ ] Agent 检查 Project。
* [ ] Agent 检查 Provider。
* [ ] Agent 检查兼容 Models。
* [ ] Agent 检查 Nodes 和 Skills。
* [ ] Agent 修改真实 Draft。
* [ ] Agent 调用 Static Validation。
* [ ] Agent 根据校验错误修订。
* [ ] Agent调用 Dry Run。
* [ ] Agent根据 Dry Run 结果修订。
* [ ] Agent最终只提交人工审批。
* [ ] Agent不能 Publish。
* [ ] Agent不能启动完整 Batch。
* [ ] Agent不能读取 Secret。
* [ ] Agent不能创建 Provider。
* [ ] Agent有预算、取消和停止条件。
* [ ] Trace 不展示隐藏思维链。

## G. Workflow 安全

* [ ] 不允许任意代码节点。
* [ ] 不允许 Shell。
* [ ] 不允许任意 URL。
* [ ] 不允许环。
* [ ] 输入输出必须兼容。
* [ ] Commit 前必须经过 Validate 和 Decision。
* [ ] 不确定分支必须 Review、Reject 或受限 fallback。
* [ ] Published Version 不可变。
* [ ] Dry Run 不写正式 Annotation。

## H. 产品

* [ ] Provider 在 Settings 中集中管理。
* [ ] API Key 只填写一次即可复用。
* [ ] Project 中可选择已配置 Model Profile。
* [ ] Agent 模型可选择。
* [ ] 选择器显示 Provider、能力和健康状态。
* [ ] 无模型时可内嵌连接 Provider。
* [ ] 默认界面不暴露 Secret 或内部 ID。
* [ ] Generic Project 不出现 RoboCup。
* [ ] 全局产品仍为 AnnotAgent。

## I. 回归

* [ ] Project 可打开。
* [ ] Published Workflow 可运行。
* [ ] Batch 可运行。
* [ ] Pause、Resume 和 Cancel 可用。
* [ ] Artifact 可查看。
* [ ] Replay 可用。
* [ ] Review 可用。
* [ ] Export 可用。
* [ ] Token 和费用记录可用。
* [ ] HTTP Vision Workers 不受破坏。

---

# 三十四、课程演示脚本

创建：

```text
docs/DEMO_PROVIDER_BUILDER.md
```

5 分钟演示：

```text
0:00–0:30
问题：用户有多个 LLM/VLM Provider，不应在每个 Project 重复填写 Key 和模型名称。

0:30–1:00
在 Settings 中展示 Provider、CredentialReference 和 Model Profiles。

1:00–1:30
创建 Project，选择已配置的 qwen3.7-flash。

1:30–2:00
启动 Pipeline Builder Agent。

2:00–2:30
Agent 检查 Label、Skills、Node Catalog 和兼容 Models。

2:30–3:00
Agent 首次绑定了不兼容模型，Rust Validator 拒绝。

3:00–3:25
Agent 查询兼容模型并修订 Draft。

3:25–3:50
Agent 执行 Dry Run，查看结果和成本。

3:50–4:15
Agent 根据 Review 数量增加 Crop Classification。

4:15–4:35
用户查看 Diff 并批准 Draft。

4:35–4:50
发布不可变 Workflow Version。

4:50–5:00
展示 Secret 不进入历史、Token 和费用可追踪。
```

---

# 三十五、明确不做

本轮不做：

* Provider Marketplace；
  -自动注册未知 Provider；
  -为每家厂商实现独立 Adapter；
  -云端 Secret 同步；
  -团队共享 Credential；
  -自动充值；
  -代用户转售 Token；
  -任意代码节点；
  -Shell Tool；
  -LLM 自动 Publish；
  -LLM 自动启动完整 Batch；
  -运行时自由修改 Workflow；
  -自动下载视觉模型权重；
  -完整模型训练平台；
  -动态插件商店；
  -多租户权限系统。

---

# 三十六、不得采用的假实现

禁止：

* 把 Provider 和 Model 放进同一个字符串；
* 每个 Project 单独保存同一份 API Key；
* 在前端 localStorage 保存 Key；
* 把 Masked Key 当作真实 Secret 返回；
* 页面加载时自动发收费请求；
* 让 Agent创建 Provider；
* 让 Agent修改 API Key；
* 让 Agent直接写完整 Workflow JSON；
* 提供 `execute_python`；
* 提供 `run_shell`；
* 把 Cache、Replay、Budget 都做成用户必须理解的节点；
* 允许无 Validate 的 Commit；
* 无兼容模型时由 Agent编造模型；
* 把 Provider infrastructure fallback 与语义 fallback 混在一起；
* 修改 Draft 后不进行 Dry Run；
* Agent直接 Publish；
* 用前端假 Trace 冒充 Tool Calls；
* 用 Mock 冒充真实 Provider；
* push；
  -修改 remote；
  -提交 API Key。

---

# 三十七、最终测试

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

1. 添加 Provider；
2. 保存 CredentialReference；
3. 添加 Model Profile；
4. Project 选择模型；
5. Pipeline Builder 使用该模型；
6. 不兼容模型被拒绝；
7. Agent重新选择兼容模型；
8. Dry Run 修订；
9. Agent不自动 Publish；
10. Secret 不泄露；
11. Provider Disable；
12. Provider Delete Conflict；
13. Key Rotation；
14. Legacy Migration；
15. Generic Project 无 RoboCup；
16. 原有 Run、Review、Replay、Export 回归。

---

# 三十八、最终报告格式

最终报告必须包含：

## 1. Provider 与 Model 边界

说明：

* Provider 管理什么；
* Model Profile 管理什么；
* Project 选择什么；
* Workflow 固定什么。

## 2. Secret 管理

说明：

* Keyring；
  -环境变量；
  -Session；
  -旧 Secret 迁移；
  -防泄露验证。

## 3. Provider UI 和 API

说明实际完成的：

* Provider CRUD；
  -模型发现；
  -健康检查；
  -Active Probe；
* Usage；
  -引用保护。

## 4. Node Catalog

列出最终允许 LLM 组合的节点，以及被移出节点目录的横切能力。

## 5. Pipeline Builder Agent

说明：

* Agent Tools；
  -多轮过程；
  -模型选择；
* Validation；
  -Dry Run；
* Revision；
  -人工审批边界；
  -停止条件。

## 6. Migration

说明：

-旧 Provider；
-旧 Model Binding；
-旧 Secret；

* Published Workflow；
  -历史 Run。

## 7. 自动测试

列出实际执行的命令和真实结果。

不得把未执行测试写成通过。

## 8. Milestone 提交

按顺序列出：

```text
commit hash
commit message
milestone
```

## 9. Live-conditional

明确说明：

-真实 Qwen；

* OpenAI；
* OpenRouter；
* Gemini compatible；
  -外部网络；
  -系统 Keychain；
  -人工浏览器测试。

## 10. 未完成内容

明确区分：

```text
未实现
已实现但未验证
外部环境限制
明确不属于本轮
```

不得使用：

```text
基本完成
理论上支持
应该可用
大概率正常
```

## 11. Git 状态

说明：

-当前分支；
-工作区是否干净；
-领先远程提交数；
-未 push；
-remote 未修改。

---

# 三十九、启动指令

将本文保存为：

```text
docs/execution/PROVIDER_BUILDER_MASTER_PROMPT.md
```

然后从 AnnotAgent 仓库根目录启动 Codex，输入：

```text
阅读 docs/execution/PROVIDER_BUILDER_MASTER_PROMPT.md，并将其作为本次长期任务的最高目标。

先核验 Git、当前 Provider、Model Binding、Secret、Workflow、Agent、API、GUI、TUI 和测试，不要盲信已有完成说明。

从 Milestone 0 开始持续执行。

本次重点是：

1. Provider 统一管理 LLM/VLM 服务连接和凭证；
2. Model Profile 表示用户真正选择的模型；
3. Provider 配置一次后可在多个 Project 中复用；
4. API Key 默认进入安全 Secret Store，而不是普通配置文件；
5. Project 和 Workflow 只能绑定已配置且能力兼容的 Model Profile；
6. Pipeline Builder Agent 只能从受 Registry 约束的节点目录构造调用链；
7. Agent 通过 Tool Calls 修改真实 Draft；
8. Agent 必须执行 Static Validation 和 Dry Run；
9. Agent 根据错误和 Dry Run 结果修订 Draft；
10. Agent 最终只提交人工审批，不能自动 Publish；
11. 保持现有 Runtime、Artifact、Batch、Replay、Review 和 Export 不回归。

普通技术决策自行完成，并记录到 PROVIDER_BUILDER_DECISIONS.md。

每完成一个 Milestone：
1. 更新状态和验收证据；
2. 执行对应 Rust、Web 和 E2E 测试；
3. 修复回归；
4. 创建独立本地提交；
5. 继续下一 Milestone。

真实 Provider 暂时不可用时，继续完成：
- Mock Provider；
- InMemory Secret Store；
- Provider Registry；
- Model Profile；
-Agent Tools；
- Scripted Mock Agent Loop；
- GUI；
-TUI；
-测试；
-迁移；
-文档。

将真实 Provider 项精确标记为 live-conditional，不得用 Mock 冒充。

不要 push。
不要修改 Git remote。
不要使用、恢复或提交任何对话中出现过的 API Key。
不要执行 reset、rebase、amend 或破坏性 checkout。

只有 Release Blocking Acceptance Matrix 全部满足，或只剩明确记录的 live-conditional 项时，才输出最终报告。
```
