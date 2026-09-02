# AnnotAgent Rust Expert Model Plugin Alpha

你现在是 AnnotAgent 仓库的长期主实现工程师。

本次任务的目标是：

> 建立一个完全由 Rust 实现的专家视觉模型插件系统，使开发者能够安装、配置、运行、更新和卸载 SAM、YOLO、RF-DETR、PIDNet、LocateAnything 及未来专家模型，而不需要修改 AnnotAgent Core，也不需要安装 Python。

本任务中的“Rust-only”严格表示：

* AnnotAgent Plugin Host 使用 Rust；
* Plugin SDK 使用 Rust；
* Plugin Protocol 使用 Rust；
* 官方模型插件使用 Rust；
* 插件可执行文件使用 Rust 编译；
* 安装流程不调用 Python；
* 运行流程不调用 Python；
* 测试不依赖 Python；
* 不使用 pip、uv、conda、venv、FastAPI、Pydantic；
* 不通过 Rust 进程启动 Python Worker；
* 不把 Python 服务伪装成 Rust 插件。

允许 Rust 插件通过维护良好的 Rust Binding 调用：

* ONNX Runtime；
* TensorRT；
* CUDA；
* Metal；
* CoreML；
* OpenVINO；
* 其他必要的原生推理库。

这些原生库必须被插件封装，不能进入 AnnotAgent Core 的业务逻辑。

不要把“Rust-only”错误解释为必须用 Rust 重新实现 CUDA、矩阵乘法、Transformer 和所有模型算子。那不是插件架构，是一项足以吞掉整个项目的文明重建活动。

---

# 一、版本目标

本次版本名称：

```text
AnnotAgent Rust Expert Model Plugin Alpha
```

最终产品关系：

```text
AnnotAgent Core
    ↓ launches and controls
Rust Model Plugin Process
    ↓ loads
ONNX / TensorRT / Candle / native model runtime
    ↓ returns
Typed AnnotAgent Artifacts
```

插件示例：

```text
SAM Rust Plugin
→ PromptedSegmentation
→ MaskSetArtifact

YOLO Rust Plugin
→ ObjectDetection
→ DetectionSetArtifact

RF-DETR Rust Plugin
→ ObjectDetection
→ DetectionSetArtifact

PIDNet Rust Plugin
→ SemanticSegmentation
→ SemanticMaskArtifact

LocateAnything Rust Plugin
→ OpenVocabularyDetection
→ DetectionSetArtifact
```

核心原则：

```text
Plugin 提供模型能力；
Skill 提供任务知识和策略；
Workflow 组合能力；
Agent 根据 Registry 选择能力；
Core 不认识具体模型品牌。
```

---

# 二、开始前核验仓库

首先执行：

```bash
git status --short --branch
git log --oneline -20
```

随后检查：

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
skills/
```

重点核验：

* 当前 HTTP Vision Protocol；
* 当前 Vision Worker Registry；
* 当前 Model Profile；
* 当前 Capability；
* 当前 Artifact Contracts；
* 当前 Plugin 或 Skill Registry；
* 当前 SAM Adapter；
* 当前 YOLO Adapter；
* 当前 RF-DETR Adapter；
* 当前 LocateAnything Adapter；
* 当前 PIDNet 或语义分割接口；
* 当前 Model Availability；
* 当前 Geometry Safety；
* 当前 Workflow Static Validator；
* 当前 Pipeline Builder Agent；
* 当前 Published Workflow Version 引用；
* 当前 Cache 和 Replay；
* 当前数据库 Migration；
* 当前测试结果；
* 当前 Python Worker 文件和所有引用。

开始前运行：

```bash
cargo test --workspace --all-features
```

记录真实基线。

不要盲信现有文档中的“已经接入”声明。必须区分：

```text
协议存在
适配器存在
Rust 插件存在
权重存在
插件进程能启动
健康检查通过
Contract 通过
真实推理通过
Workflow 可选择
```

禁止：

```text
git reset
git rebase
git commit --amend
破坏性 git checkout
修改 Git remote
push
使用或恢复任何对话中出现过的 API Key
提交大型模型权重
用 Mock 推理冒充真实模型
```

---

# 三、长期执行状态文件

创建并持续维护：

```text
docs/execution/RUST_PLUGIN_MASTER_PLAN.md
docs/execution/RUST_PLUGIN_STATUS.md
docs/execution/RUST_PLUGIN_DECISIONS.md
docs/execution/RUST_PLUGIN_ACCEPTANCE.md
docs/execution/RUST_PLUGIN_BLOCKERS.md
docs/execution/RUST_PLUGIN_KNOWN_LIMITATIONS.md
```

`RUST_PLUGIN_STATUS.md` 必须记录：

```text
当前 Milestone
已完成
正在进行
下一步
最近 Rust 测试
最近插件 Conformance Test
最近真实模型测试
最近 Web 测试
最近 E2E
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
6. 继续下一个 Milestone；
7. 不等待用户确认。

---

# 四、插件必须是独立 Rust 进程

第一版插件形态固定为：

```rust
pub enum PluginRuntimeKind {
    NativeRustProcess,
}
```

不要实现：

```text
Rust dylib / cdylib 热加载
C ABI 插件
Python Worker
WASM GPU 插件
任意 Shell 插件
```

原因：

* Rust 没有稳定语言级 ABI；
* 第三方模型崩溃不能拖死 AnnotAgent；
* CUDA、ONNX Runtime 和模型依赖需要隔离；
* 插件版本需要并存；
* 插件更新不能改变旧 Workflow；
* Core 不能加载任意第三方代码到同一地址空间。

正确结构：

```text
AnnotAgent Plugin Host
→ 启动经过验证的 Rust Binary
→ Binary 仅监听 loopback 临时端口
→ 使用版本化协议通信
→ 返回强类型 Artifact
→ 插件崩溃时 Core 保持运行
```

---

# 五、Workspace 结构

新增或整理：

```text
crates/
├── annotagent-plugin-api/
├── annotagent-plugin-sdk/
├── annotagent-plugin-host/
├── annotagent-plugin-registry/
├── annotagent-model-runtime-onnx/
└── annotagent-model-runtime-common/

plugins/
├── dummy-detector/
├── yolo-onnx/
├── sam-onnx/
├── rfdetr-onnx/
├── pidnet-onnx/
└── locate-anything-rust/

plugin-fixtures/
└── tiny-models/
```

职责如下。

## `annotagent-plugin-api`

只包含稳定的数据类型：

* Plugin ID；
* Plugin Version；
* Manifest；
* Capability；
* Artifact Contract；
  -运行协议；
  -错误类型；
  -安装状态；
  -权重身份；
  -许可证；
  -资源要求；
  -Plugin Model Reference。

不得依赖具体模型 crate。

## `annotagent-plugin-sdk`

供插件开发者使用：

* Rust Plugin Server；
* health endpoint；
* capability endpoint；
* model endpoint；
* contract endpoint；
* infer endpoint；
* cancel endpoint；
* warmup endpoint；
* shutdown endpoint；
  -请求校验；
  -响应序列化；
  -图片解码；
  -坐标归一化；
* Artifact Builder；
* tracing；
* cancellation；
  -错误映射；
* conformance helper。

## `annotagent-plugin-host`

负责：

* 插件进程启动；
  -握手；
  -健康检查；
  -生命周期；
  -取消；
  -超时；
  -崩溃恢复；
  -标准输出和错误输出的安全采集；
  -资源限制；
  -临时文件；
  -请求认证；
  -插件禁用；
  -版本并存。

## `annotagent-plugin-registry`

负责：

* 安装记录；
  -版本；
  -插件状态；
  -引用关系；
  -权重配置；
* Model Profile 注册；
  -更新；
  -禁用；
  -卸载；
  -插件目录；
  -许可证确认；
  -审计事件。

## `annotagent-model-runtime-onnx`

提供通用 Rust ONNX 推理基础：

* Session 创建；
  -CPU Execution Provider；
  -可选 CUDA Execution Provider；
  -可选 TensorRT Execution Provider；
  -Tensor shape 校验；
  -模型输入输出查询；
  -动态维度处理；
  -线程配置；
  -显存或内存错误映射；
  -Session cache；
  -Warmup；
  -Cancel 边界；
  -模型文件 hash。

优先采用维护良好的 Rust ONNX Runtime Binding。

不要把模型专用预处理和后处理全部塞进通用 Runtime。

## `annotagent-model-runtime-common`

提供：

-图像 resize；
-letterbox；
-normalization；
-NCHW/NHWC 转换；
-bbox 转换；
-NMS；
-mask resize；
-mask threshold；
-connected components；
-contour extraction；
-polygon simplification；
-tensor utilities；
-数值校验。

---

# 六、彻底禁止 Python 依赖

本次实现完成后，官方插件安装与运行路径中不得出现：

```text
python
python3
pip
uv
conda
venv
requirements.txt
FastAPI
Pydantic
torch Python package
transformers Python package
```

新增 CI 检查：

```bash
rg -n \
  "python3?|pip|uv |conda|venv|requirements\.txt|FastAPI|Pydantic" \
  crates/annotagent-plugin-* \
  crates/annotagent-model-runtime-* \
  plugins/
```

允许这些词只出现在：

```text
docs/legacy/
migration documentation
historical compatibility notes
```

禁止 Rust 插件调用：

```rust
Command::new("python")
Command::new("python3")
Command::new("uv")
Command::new("pip")
```

增加静态测试或仓库检查。

---

# 七、现有 Python Worker 的迁移策略

不要立刻删除现有 Worker 文件而破坏回归。

按以下流程迁移：

1. 列出所有 Python Worker；
2. 列出其 Capability；
3. 列出输入输出协议；
4. 列出对应测试；
5. 创建 Rust 等价插件；
6. 完成 Contract parity test；
7. 完成 Artifact parity test；
8. 更新 Registry 和示例；
9. 更新文档；
10. 删除运行时引用；
11. 将旧 Worker 移到：

```text
docs/legacy/python-workers/
```

或在确认没有价值后删除；
12. 官方 Release 不包含旧 Python Worker；
13. CI 不运行旧 Python Worker；
14. UI 不再给出 Python Worker 启动说明。

新增：

```text
docs/RUST_PLUGIN_MIGRATION.md
```

记录每个旧 Worker 的迁移状态。

---

# 八、插件包格式

插件包后缀：

```text
.annotplugin
```

使用确定性归档格式。可以采用 ZIP 或 tar.zst，但同一版本必须具有稳定内容摘要。

包结构：

```text
plugin.annotplugin/
├── annotagent-plugin.toml
├── checksums.json
├── signatures/
│   └── publisher.ed25519.sig
├── bin/
│   ├── linux-x86_64/
│   │   └── plugin
│   ├── linux-aarch64/
│   │   └── plugin
│   ├── macos-aarch64/
│   │   └── plugin
│   └── windows-x86_64/
│       └── plugin.exe
├── models/
│   └── models.toml
├── weights/
│   └── recipes.toml
├── licenses/
│   ├── CODE-LICENSE
│   └── WEIGHTS-LICENSE
├── schemas/
├── fixtures/
├── icons/
└── README.md
```

插件包默认不包含大权重。

允许打包小型测试权重，但必须：

* 有明确许可证；
* 有 SHA-256；
* 用于 conformance test；
* 体积受限；
* 不冒充生产模型。

---

# 九、Plugin Manifest

示例：

```toml
schema_version = "1"

id = "org.annotagent.sam-onnx"
version = "1.0.0"
display_name = "SAM Prompted Segmentation"
description = "Rust-native prompted segmentation plugin"
publisher = "AnnotAgent"
plugin_api = "1"

[runtime]
kind = "native_rust_process"
entrypoint = "bin/{target}/annotagent-plugin-sam"
protocol = "http-vision-v1"
startup_timeout_seconds = 30
shutdown_timeout_seconds = 10

[compatibility]
annotagent = ">=0.3.0,<0.4.0"
targets = ["linux-x86_64", "linux-aarch64"]
accelerators = ["cpu", "cuda"]

[permissions]
network = "loopback_only"
provider_secrets = false
project_files = false
temporary_images = true
plugin_cache = true
subprocesses = false

[resources]
minimum_memory_mb = 4096
recommended_memory_mb = 8192
minimum_vram_mb = 0
recommended_vram_mb = 8192
maximum_response_mb = 256

[[models]]
id = "sam-vit-b-onnx"
display_name = "SAM ViT-B ONNX"
capabilities = ["prompted_segmentation"]
input_contracts = ["image+box_prompts", "image+point_prompts"]
output_contracts = ["mask_set"]
score_semantics = "relative_confidence"
geometry_semantics = "refined_geometry"

[weights]
bundled = false
required = true
provisioning = "local_path_or_fixed_recipe"
checkpoint_sha256_required = true

[license]
code = "Apache-2.0"
weights = "user_confirmation_required"
commercial_use = "unknown"
```

Manifest 必须声明：

* Plugin ID；
  -版本；
  -Plugin API；
  -支持平台；
  -可执行文件；
  -协议；
* Capability；
  -输入输出 Contract；
  -分数语义；
  -几何语义；
  -资源要求；
  -权限；
  -权重配置；
  -许可证；
  -兼容的 AnnotAgent 版本。

---

# 十、模型品牌不能进入 Core

禁止：

```rust
NodeKind::Sam
NodeKind::Yolo
NodeKind::RfDetr
NodeKind::LocateAnything
NodeKind::PidNet
```

禁止：

```rust
if plugin_id == "sam" { ... }
if model_id.starts_with("yolo") { ... }
```

Core 只认识：

```rust
pub enum ModelCapability {
    ObjectDetection,
    OpenVocabularyDetection,
    PhraseGrounding,
    ImageClassification,
    SemanticSegmentation,
    PromptedSegmentation,
    InstanceSegmentation,
    KeypointDetection,
}
```

正确表达：

```text
Node:
Prompted Segmentation

Model Binding:
sam-vit-b-onnx

Plugin:
org.annotagent.sam-onnx@1.0.0
```

`robocup.ball` 只能请求 Capability，不得请求具体插件 ID。

---

# 十一、Plugin Process Handshake

Plugin Host 启动插件时：

1. 创建随机 Session Token；
2. 分配插件私有状态目录；
3. 分配插件权重只读目录；
4. 生成临时 loopback 监听要求；
5. 启动 Rust 插件进程；
6. 不传递 Provider Secret；
7. 通过 stdin 或受控 IPC 传递一次性启动配置；
8. 插件打印单行结构化 Ready Handshake；
9. Host 校验；
10. 之后才允许请求。

Handshake 示例：

```json
{
  "status": "ready",
  "plugin_api": "1",
  "protocol_version": "1",
  "listen": "127.0.0.1:43127",
  "worker_id": "org.annotagent.sam-onnx",
  "session_nonce": "..."
}
```

插件必须：

* 仅监听 loopback；
* 不使用固定公共端口；
* 不把 Token 写日志；
* 不接受未授权请求；
* 进程结束后端口释放。

如果现有 HTTP Vision Protocol 已有认证机制，应扩展并复用，不创建重复协议。

---

# 十二、Rust Plugin Protocol

复用并扩展当前 HTTP Vision Protocol。

至少支持：

```text
GET  /health
GET  /v1/capabilities
GET  /v1/models
GET  /v1/contracts
POST /v1/infer
POST /v1/cancel
POST /v1/warmup
POST /v1/shutdown
```

所有请求必须包含 Host 生成的 Session Token。

## Health

返回：

```json
{
  "status": "ready",
  "plugin_id": "org.annotagent.sam-onnx",
  "plugin_version": "1.0.0",
  "protocol_version": "1",
  "loaded_models": ["sam-vit-b-onnx"],
  "device": "cuda",
  "uptime_ms": 4000
}
```

## Capabilities

返回插件实际能力，而不是简单复述 Manifest。

Host 必须交叉校验：

```text
Manifest declaration
vs
Runtime declaration
```

不一致时：

```text
InvalidContract
```

## Infer

输入和输出继续使用 AnnotAgent Artifact Contract。

不能传任意宿主机文件路径。

---

# 十三、Plugin SDK

开发者使用：

```rust
use annotagent_plugin_sdk::{
    Plugin,
    PluginServer,
    InferenceContext,
    ArtifactInput,
    ArtifactOutput,
};
```

接口建议：

```rust
#[async_trait]
pub trait ExpertModelPlugin: Send + Sync + 'static {
    fn descriptor(&self) -> PluginRuntimeDescriptor;

    fn models(&self) -> Vec<ModelRuntimeDescriptor>;

    async fn warmup(
        &self,
        model_id: &str,
        context: WarmupContext,
    ) -> Result<(), PluginError>;

    async fn infer(
        &self,
        request: InferenceRequest,
        context: InferenceContext,
    ) -> Result<InferenceResponse, PluginError>;

    async fn cancel(
        &self,
        request_id: RequestId,
    ) -> Result<(), PluginError>;
}
```

SDK 自动处理：

* HTTP Server；
  -认证；
  -协议版本；
* tracing；
* panic 隔离；
* JSON 大小限制；
  -请求校验；
  -取消；
  -超时；
  -错误响应；
  -health；
  -capabilities；
  -models；
  -contracts；
  -shutdown。

插件开发者只实现模型加载、预处理、推理和后处理。

---

# 十四、Plugin Scaffold

CLI：

```bash
annotagent plugin scaffold \
  --id com.example.my-detector \
  --capability object_detection \
  --runtime rust-onnx
```

生成：

```text
my-detector/
├── Cargo.toml
├── annotagent-plugin.toml
├── src/
│   ├── main.rs
│   ├── model.rs
│   ├── preprocess.rs
│   └── postprocess.rs
├── models/
│   └── model.toml
├── tests/
│   ├── conformance.rs
│   └── fixtures.rs
├── fixtures/
├── licenses/
└── README.md
```

Preset：

```bash
annotagent plugin scaffold --preset yolo-onnx
annotagent plugin scaffold --preset sam-onnx
annotagent plugin scaffold --preset rfdetr-onnx
annotagent plugin scaffold --preset pidnet-onnx
annotagent plugin scaffold --preset locate-anything-rust
```

Preset 生成可编译工程和 TODO，不得声称没有权重和模型实现时已经可推理。

---

# 十五、Plugin CLI

实现：

```bash
annotagent plugin inspect <package>

annotagent plugin pack <plugin-directory>
annotagent plugin verify <package>

annotagent plugin install <package>
annotagent plugin list
annotagent plugin show <plugin-id>
annotagent plugin versions <plugin-id>

annotagent plugin provision \
  <plugin-id> \
  --model <model-id> \
  --weights <path>

annotagent plugin start <plugin-id>
annotagent plugin stop <plugin-id>
annotagent plugin restart <plugin-id>
annotagent plugin test <plugin-id>
annotagent plugin doctor <plugin-id>

annotagent plugin enable <plugin-id>
annotagent plugin disable <plugin-id>

annotagent plugin update <package>

annotagent plugin uninstall \
  <plugin-id>@<version>

annotagent plugin gc
```

`plugin gc` 只能删除：

* 无引用插件版本；
* 无引用权重；
  -过期临时文件；
  -废弃测试缓存。

不得删除 Published Workflow 所引用的版本。

---

# 十六、安装生命周期

执行：

```bash
annotagent plugin install model.annotplugin
```

必须完成：

```text
1. 解析归档
2. 防止 Zip Slip / Path Traversal
3. 验证 Manifest
4. 验证 AnnotAgent 版本兼容
5. 验证平台可执行文件
6. 验证每个文件的 SHA-256
7. 验证可选 Publisher Signature
8. 显示权限
9. 显示许可证
10. 用户确认
11. 原子安装到版本目录
12. 不覆盖已有版本
13. 启动插件
14. 执行 Handshake
15. 健康检查
16. Capability Discovery
17. Contract Discovery
18. 与 Manifest 交叉校验
19. 执行 Conformance Test
20. 注册 Plugin Models
21. 根据权重状态标记 Ready 或 NeedsWeights
```

复制文件成功不等于插件可用。

---

# 十七、安装目录

使用操作系统标准应用数据目录，不硬编码用户名。

逻辑结构：

```text
<annotagent-data>/
├── plugins/
│   └── <plugin-id>/
│       └── <version>/
│           ├── manifest/
│           ├── bin/
│           ├── licenses/
│           └── runtime/
├── model-cache/
│   └── <plugin-id>/
│       └── <model-id>/
│           └── <checkpoint-sha256>/
├── plugin-state/
└── plugin-logs/
```

插件不能读取整个 AnnotAgent Workspace。

插件只得到：

* 本次请求的图片字节；
* 本次请求的 Artifact；
  -插件私有缓存；
  -插件权重只读目录；
  -临时目录；
  -取消状态。

---

# 十八、权重管理

权重不默认打包进插件。

支持：

## 本地路径导入

```bash
annotagent plugin provision \
  org.annotagent.sam-onnx \
  --model sam-vit-b-onnx \
  --weights ~/Models/sam-vit-b.onnx
```

Host 必须：

-复制到受控缓存；
-计算 SHA-256；
-保存原始文件名；
-保存模型身份；
-验证大小限制；
-验证 Plugin 能加载；
-不继续依赖原始用户路径。

## 固定下载 Recipe

Manifest 可声明：

```toml
[[weight_recipes]]
id = "official-sam-vit-b"
url = "https://..."
sha256 = "..."
license_url = "https://..."
filename = "sam-vit-b.onnx"
```

Host 只能执行：

```text
HTTPS GET
→ 大小限制
→ SHA-256 校验
→ 原子写入
```

不得执行：

```text
Shell
Git clone
Python script
Install script
任意命令
```

下载和许可证确认必须由用户明确触发。

Pipeline Builder Agent 不得下载权重或接受许可证。

---

# 十九、插件状态

```rust
pub enum PluginStatus {
    Discovered,
    Installing,
    Installed,
    NeedsWeights,
    UnsupportedPlatform,
    Disabled,
    Starting,
    Ready,
    Unhealthy,
    Crashed,
    IncompatibleApi,
    InvalidManifest,
    InvalidContract,
    FailedSmokeTest,
    UpdateAvailable,
}
```

Model Profile 只有在以下条件满足时才能进入可运行选择器：

```text
PluginStatus == Ready
Weight identity complete
Contract valid
Smoke Test passed
Model enabled
```

其他状态只允许作为 Setup Alternative。

---

# 二十、Model Profile 自动注册

插件安装后，Model Registry 自动创建：

```rust
pub struct PluginBackedModelProfile {
    pub plugin_id: PluginId,
    pub plugin_version: Version,
    pub plugin_manifest_sha256: String,
    pub plugin_api_version: String,

    pub model_id: String,
    pub model_profile_revision: u64,
    pub capability_contract_hash: String,

    pub checkpoint_sha256: Option<String>,
    pub score_semantics: ScoreSemantics,
    pub geometry_semantics: GeometrySemantics,
    pub capabilities: BTreeSet<ModelCapability>,
    pub availability: ModelAvailability,
}
```

用户在 Project 中选择 Model Profile，不直接选择插件进程。

---

# 二十一、Published Workflow 固定插件身份

发布 Workflow 时固定：

```text
Plugin ID
Plugin Version
Plugin Package Digest
Plugin API Version
Worker Protocol Version
Model ID
Model Profile Revision
Checkpoint SHA-256
Capability Contract Hash
Node Config
```

因此：

* 插件更新不改变旧 Workflow；
  -权重替换不改变旧 Workflow；
  -旧 Run 可追溯；
* Replay 可以要求精确插件版本；
  -插件缺失时显示明确安装要求。

禁止只保存：

```text
model = sam
```

---

# 二十二、版本并存和更新

插件版本目录必须旁路安装：

```text
org.annotagent.sam-onnx/
├── 1.0.0/
└── 1.1.0/
```

更新流程：

```text
安装新版本
→ 完成校验
→ 完成 Smoke Test
→ 注册新 Model Revision
→ 旧版本保持不变
→ 用户从 Published Version 创建新 Draft
→ 人工迁移绑定
→ Dry Run
→ 发布新 Workflow
```

不得自动把所有 Project 换成新插件。

---

# 二十三、卸载引用保护

卸载前检查：

* Active Run；
* Published Workflow；
* Draft；
* Project Binding；
* Replay；
* Calibration；
* Artifact；
  -历史 Run；
* Cache；
* Model Profile。

存在引用时返回：

```text
Cannot uninstall plugin version.

Referenced by:
- Workflow robocup-ball@v2
- Calibration geometry-17
- 4 historical runs
```

允许：

```text
Disable plugin
Remove executable but retain metadata
Force remove after explicit warning
```

即使 Runtime 被删除，历史记录仍保留。

---

# 二十四、安全边界

Rust 子进程并不自动等于完整沙箱，因此必须诚实实现和描述边界。

Alpha 至少保证：

1. 插件不在 Core 进程中执行；
2. 插件不获得 Provider Secret；
3. 插件不获得 API Key；
4. 插件不获得数据库连接；
5. 插件不获得任意 Workspace 路径；
6. 图片通过字节传输；
7. 权重目录只读；
8. 插件缓存单独隔离；
   9.环境变量白名单；
9. 当前目录固定；
10. stdout/stderr 经过大小限制和脱敏；
    12.响应大小限制；
    13.超时；
    14.取消；
    15.进程退出不会拖死 Server；
    16.并发限制；
    17.可配置最大内存；
    18.可配置最大请求数；
    19.远程网络默认禁止；
    20.权限在安装前展示。

不要声称进程隔离等于强操作系统级沙箱。

若实现 Linux sandbox：

* 作为 feature；
  -使用明确的 namespace/cgroup/seccomp 机制；
  -无此能力时显示 `process-isolated only`；
  -不要静默声称完全沙箱化。

---

# 二十五、插件认证

Host 启动插件时生成一次性 Session Token。

要求：

* Token 不持久化；
* Token 不写日志；
* Token 不进入历史；
* Token 只传给当前子进程；
  -插件只接受携带当前 Token 的请求；
  -进程重启后 Token 变化；
  -其他本地进程不能随意调用插件。

---

# 二十六、Rust ONNX 模型运行时

建立统一 `annotagent-model-runtime-onnx`。

最低支持：

```text
CPU Execution Provider
Optional CUDA Execution Provider
Optional TensorRT Execution Provider
```

每个 Plugin Manifest 声明构建 feature 和目标平台。

通用 Runtime 负责：

-模型文件加载；
-Session；
-输入 tensor；
-输出 tensor；
-shape；
-dtype；
-设备；
-线程；
-warmup；
-session cache；
-错误。

模型插件负责：

-预处理；
-输入名；
-输出名；
-后处理；
-label mapping；
-NMS；
-mask decode；
-模型专用参数。

不要在通用 Runtime 中出现：

```text
YOLO anchor
SAM prompt encoder
RF-DETR class decoder
PIDNet palette
```

---

# 二十七、YOLO Rust 插件

插件：

```text
org.annotagent.yolo-onnx
```

Capability：

```text
ObjectDetection
Optional InstanceSegmentation
```

实现：

* letterbox；
  -normalization；
  -ONNX Session；
  -输出 decode；
  -NMS；
  -class mapping；
  -normalized bbox；
  -真实 score；
  -label space；
  -Artifact；
  -cancel；
  -batch；
  -tests。

输入：

```text
ImageArtifact
```

输出：

```text
DetectionSetArtifact
```

不能直接 Commit Annotation。

首个真实 Release Blocking 模型建议优先选择 YOLO ONNX，因为它最适合作为 Rust 插件体系的完整样板。

---

# 二十八、SAM Rust 插件

插件：

```text
org.annotagent.sam-onnx
```

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

实现：

* image encoder；
  -prompt encoder/decoder；
  -embedding cache；
  -box prompt；
  -point prompt；
  -multi-mask；
  -mask score；
  -mask resize；
  -Artifact；
  -cancel；
  -tests。

Workflow 保持显式：

```text
DetectionSet
→ DetectionsToBoxPrompts
→ PromptedSegmentation
→ MaskSet
→ MaskToBBox
→ GeometryEvaluation
```

不要把 Mask-to-BBox 隐藏进 SAM 插件。

如果实际 SAM ONNX 导出由多个模型文件组成，Manifest 必须记录每个文件及 SHA-256。

---

# 二十九、RF-DETR Rust 插件

插件：

```text
org.annotagent.rfdetr-onnx
```

Capability：

```text
ObjectDetection
```

要求：

* 使用合法导出的 ONNX 或其他 Rust 可调用格式；
  -输入预处理；
  -输出 decode；
  -class mapping；
  -score；
  -label space；
  -NMS 或模型指定后处理；
  -checkpoint identity；
  -training dataset version；
  -tests。

若当前 RF-DETR 导出产物不能被选定 Rust Runtime正确执行：

* 标记 `live-conditional`；
  -完成 Manifest；
  -完成 Contract；
  -完成 Mock Fixture；
  -不得退回 Python；
  -不得声称真实插件已完成。

---

# 三十、PIDNet Rust 插件

插件：

```text
org.annotagent.pidnet-onnx
```

Capability：

```text
SemanticSegmentation
```

输入：

```text
ImageArtifact
```

输出：

```text
SemanticMaskArtifact
```

实现：

* resize；
  -normalization；
  -ONNX；
  -logit decode；
  -class mapping；
  -mask；
  -coordinate restore；
  -tests。

---

# 三十一、LocateAnything Rust 插件

插件：

```text
org.annotagent.locate-anything-rust
```

Capability：

```text
OpenVocabularyDetection
PhraseGrounding
```

这一插件不得通过 Python 启动原模型。

必须先进行可行性审计，检查是否存在以下可行路径之一：

```text
A. 可由 Rust ONNX Runtime 执行的完整导出
B. Candle / Burn 等 Rust 推理实现
C. 可靠的 Rust 原生模型格式和 tokenizer
```

如果没有可执行路径：

1. 实现 Manifest；
2. 实现 Protocol Contract；
3. 实现 Scripted Fixture Plugin；
4. 实现 Registry 和 UI；
5. 状态标记：

   ```text
   Unsupported until a Rust model runtime is available
   ```
6. 不下载 Python 项目；
7. 不启动 Python；
8. 不把旧 Python Worker 当成新插件；
9. 不声称真实推理完成。

LocateAnything 不是本次 Alpha 的首个真实推理 Release Blocker。

插件架构不能因为一个复杂模型尚未 Rust 化而被整个拖死。

---

# 三十二、参考实现优先级

真实模型实现顺序：

```text
1. Dummy Detector Rust Plugin
2. Generic ONNX Fixture Plugin
3. YOLO ONNX Plugin
4. PIDNet ONNX Plugin
5. SAM ONNX Plugin
6. RF-DETR ONNX Plugin
7. LocateAnything Rust feasibility / implementation
```

理由：

* 先验证插件系统；
* 再验证通用 ONNX；
* 再验证检测；
  -再验证分割；
  -再验证多文件复杂模型；
  -最后处理复杂多模态模型。

不要同时调试六个模型，然后让每个模型都成为其他五个失败的嫌疑人。

---

# 三十三、Plugin Conformance Test

SDK 提供统一测试：

```bash
annotagent plugin test <plugin-id>
```

至少检查：

1. 进程启动；
2. Handshake；
3. Token 认证；
4. Health；
5. Capability；
6. Models；
7. Contracts；
8. Manifest/runtime 一致；
9. 无效请求；
10. 超大请求；
11. 非法图片；
12. 非法坐标；
13. NaN；
14. Infinity；
    15.取消；
    16.超时；
    17.并发；
    18.崩溃；
    19.重启；
    20.响应大小；
15. Artifact 类型；
16. Model identity；
    23.权重 hash；
    24.许可证 metadata。

只有通过 Conformance Test 的插件才能标记 Ready。

---

# 三十四、插件开发模式

支持：

```bash
annotagent plugin dev ./plugins/my-plugin
```

开发模式：

* 不安装；
  -从本地 Rust binary 启动；
  -监控进程；
  -显示协议日志；
  -运行 Conformance；
  -不允许用于 Published Workflow；
  -标记：

  ```text
  Development plugin
  ```

发布前必须 `pack` 并安装正式版本。

---

# 三十五、Pipeline Builder Agent 的权限

Pipeline Builder Agent 可以：

```text
list_installed_plugins
list_ready_models
list_compatible_models
inspect_plugin_capabilities
inspect_model_contracts
inspect_plugin_health
```

Pipeline Builder Agent 不可以：

```text
install_plugin
update_plugin
uninstall_plugin
download_weights
accept_license
start_arbitrary_binary
change_plugin_permissions
```

缺少模型时，Agent 只能生成：

```text
Blocked Draft
+
Required Capability
+
Compatible Plugin Suggestions
```

用户点击：

```text
Install compatible model
```

后进入人工安装流程。

---

# 三十六、Geometry Safety 集成

现有几何安全规则必须继续生效。

例如：

```text
VLM coarse bbox
→ semantic score
→ Commit
```

仍然必须被阻止。

安装 SAM 插件后，Pipeline Builder 可以建议：

```text
VLM Detection
→ DetectionsToBoxPrompts
→ SAM Plugin
→ MaskToBBox
→ GeometryEvaluation
→ GeometryDecision
→ Commit / Review
```

条件：

* SAM Plugin Ready；
  -权重 identity 完整；
* Contract 匹配；
* Smoke Test 通过；
* Model Profile enabled；
* Project 允许；
* Workflow 校验通过。

SAM 插件存在但未配置权重时：

```text
NeedsWeights
```

不得进入 Runnable Draft。

---

# 三十七、GUI

新增：

```text
Settings → Expert Model Plugins
```

页面分组：

```text
Ready
Needs setup
Disabled
Unhealthy
Development
Updates
```

插件卡：

```text
YOLO ONNX
v1.0.0

Status: Ready
Capability: Object detection
Runtime: Rust native process
Model: yolo-model
Device: CUDA
Checkpoint: 4f92…
Used by: 2 projects

[Test]
[Models]
[Disable]
[Details]
```

安装向导：

```text
Select package
→ Verify package
→ Review publisher
→ Review permissions
→ Review licenses
→ Check platform
→ Install
→ Add weights
→ Test
→ Register models
```

不得显示任何 Python 环境步骤。

---

# 三十八、TUI

增加：

```text
/plugins
/plugins show <id>
/plugins install <path>
/plugins test <id>
/plugins start <id>
/plugins stop <id>
/plugins doctor <id>
/plugins disable <id>
/plugins enable <id>

/models compatible <capability>
```

TUI 不显示 Secret。

插件日志默认只显示摘要，完整日志需要显式打开。

---

# 三十九、API

实现或整理：

```text
GET    /api/plugins
POST   /api/plugins/install
GET    /api/plugins/:pluginId
POST   /api/plugins/:pluginId/start
POST   /api/plugins/:pluginId/stop
POST   /api/plugins/:pluginId/restart
POST   /api/plugins/:pluginId/test
POST   /api/plugins/:pluginId/enable
POST   /api/plugins/:pluginId/disable
DELETE /api/plugins/:pluginId/:version

GET    /api/plugins/:pluginId/models
POST   /api/plugins/:pluginId/models/:modelId/provision

GET    /api/plugins/:pluginId/references
GET    /api/plugins/:pluginId/logs

GET    /api/plugins/compatible/:capability
```

API 不接受任意可执行文件路径。安装文件必须：

* 来自用户选择的本地归档；
  -被复制到服务端受控临时目录；
  -经过验证；
  -再安装。

---

# 四十、数据库

至少保存：

```text
plugins
plugin_versions
plugin_installations
plugin_permissions
plugin_models
plugin_weight_sets
plugin_health_checks
plugin_test_runs
plugin_references
plugin_license_acceptances
plugin_events
```

不要保存：

* Session Token；
* Provider API Key；
  -图片原始字节；
  -任意插件明文 Secret。

---

# 四十一、旧工作流兼容

现有 Workflow 可能引用手工配置的 HTTP Worker。

迁移方式：

```text
Legacy HTTP Worker Binding
→ External Legacy Model Profile
```

不要把它自动伪装成已安装 Rust Plugin。

提供迁移操作：

```text
Create Rust plugin binding
```

只有用户安装了等价 Rust 插件并通过 Dry Run 后，才创建新 Draft。

历史 Published Workflow 不修改。

---

# 四十二、Release Blocking Acceptance Matrix

## A. Rust-only

* [ ] 官方 Plugin Host 是 Rust。
* [ ] 官方 Plugin SDK 是 Rust。
* [ ] 官方 Reference Plugins 是 Rust。
* [ ] 官方安装不需要 Python。
* [ ] 官方运行不需要 Python。
* [ ] 官方测试不需要 Python。
* [ ] 不启动 Python 子进程。
* [ ] CI 检查活动插件路径无 Python 依赖。
* [ ] Release 包不包含活动 Python Worker。

## B. 插件边界

* [ ] 插件是独立 Rust 进程。
* [ ] 不使用 Rust dylib ABI。
* [ ] 插件崩溃不拖死 Core。
* [ ] Plugin API 有版本。
* [ ] Protocol 有版本。
* [ ] Session Token 认证有效。
* [ ] 插件只监听 loopback。
* [ ] Provider Secret 不传给插件。
* [ ] 插件不获得数据库连接。
* [ ] 插件不获得任意 Workspace 路径。

## C. 安装

* [ ] `.annotplugin` 可以校验。
* [ ] 防止归档路径穿越。
* [ ] 文件 checksum 校验。
* [ ] 平台兼容校验。
* [ ] 权限在安装前展示。
* [ ] 许可证在安装前展示。
* [ ] 原子安装。
* [ ] 不覆盖旧版本。
* [ ] 缺少权重时显示 NeedsWeights。
* [ ] Conformance 通过后才显示 Ready。

## D. 版本和复现

* [ ] 新旧插件版本可以并存。
* [ ] Published Workflow 固定插件版本。
* [ ] Published Workflow 固定包 digest。
* [ ] Published Workflow 固定模型 revision。
* [ ] Published Workflow 固定 checkpoint hash。
* [ ] 更新不改变旧 Workflow。
* [ ] 有引用版本不能直接卸载。
* [ ] 历史 Run 在插件缺失时仍可查看。

## E. Model Registry

* [ ] 插件自动注册 Model Profile。
* [ ] Model Profile 使用 Capability。
* [ ] 模型品牌不进入 Core Node。
* [ ] `robocup.ball` 不引用插件 ID。
* [ ] 只有 Ready Model 可以进入 Runnable Draft。
* [ ] NeedsWeights Model 只作为 Setup Alternative。
* [ ] Worker Contract 与 Manifest 会交叉校验。

## F. 参考插件

* [ ] Dummy Detector Rust Plugin 完整通过。
* [ ] Generic ONNX Fixture Plugin 完整通过。
* [ ] 至少一个真实 ONNX Model Rust Plugin 完整推理通过。
* [ ] YOLO Rust Plugin 能输出 DetectionSet。
* [ ] SAM Rust Plugin Contract 可表达 PromptedSegmentation。
* [ ] RF-DETR Rust Plugin Contract 可表达 ObjectDetection。
* [ ] PIDNet Rust Plugin Contract 可表达 SemanticSegmentation。
* [ ] LocateAnything 无 Rust 推理时不会假装可用。

## G. Workflow

* [ ] Plugin Artifact 可以进入现有 DAG。
* [ ] Static Validator 校验 Capability 和 Contract。
* [ ] Plugin 输出有 Artifact lineage。
* [ ] Cache Key 包含插件和权重版本。
* [ ] Replay 固定插件版本。
* [ ] Cancel 可传到插件。
* [ ] Timeout 可终止请求。
* [ ] 不产生重复 Commit。
* [ ] Geometry Safety 不回归。

## H. 产品

* [ ] GUI 可以安装本地插件包。
* [ ] GUI 可以查看安装状态。
* [ ] GUI 可以配置权重。
* [ ] GUI 可以运行测试。
* [ ] GUI 可以查看模型和 Capability。
* [ ] TUI 可以管理基本生命周期。
* [ ] Pipeline Builder 能发现插件模型。
* [ ] Agent 不能自行安装插件。
* [ ] Generic Project 可使用插件。
* [ ] RoboCup 只作为 Skill 上下文出现。

---

# 四十三、Milestone 计划

## Milestone 0：基线与迁移盘点

完成：

* 核验 Python Workers；
  -核验 HTTP Protocol；
  -核验 Model Registry；
  -列出迁移矩阵；
  -建立状态文档；
  -建立 Rust-only CI 规则；
  -建立测试基线。

提交：

```text
docs: establish rust plugin architecture baseline
```

## Milestone 1：Plugin API 和 Manifest

完成：

* Plugin ID；
  -版本；
  -Manifest；
  -Capability；
  -Contract；
  -权限；
  -权重；
  -许可证；
  -包 digest；
  -序列化和测试。

提交：

```text
feat(plugin): define a versioned rust model plugin api
```

## Milestone 2：Plugin SDK 和 Dummy Plugin

完成：

* Rust SDK；
  -Plugin Server；
  -Handshake；
  -认证；
  -Health；
  -Models；
  -Contracts；
  -Infer；
  -Cancel；
  -Dummy Detector；
  -Conformance Test。

提交：

```text
feat(plugin): add rust sdk and isolated reference worker
```

## Milestone 3：Plugin Host 和 Registry

完成：

-安装；
-验证；
-启动；
-停止；
-重启；
-状态；
-崩溃恢复；
-版本并存；
-数据库；
-CLI；
-测试。

提交：

```text
feat(plugin): manage rust model plugin lifecycles
```

## Milestone 4：Rust ONNX Runtime

完成：

-通用 ONNX Session；
-CPU；
-可选 CUDA；
-输入输出；
-shape；
-dtype；
-cache；
-warmup；
-错误；
-tiny fixture。

提交：

```text
feat(inference): add reusable rust onnx model runtime
```

## Milestone 5：YOLO Rust Plugin

完成：

-ONNX；
-预处理；
-后处理；
-NMS；
-DetectionSet；
-Model Profile；
-真实或合法小模型 Smoke Test；
-Workflow E2E。

提交：

```text
feat(plugin): add rust-native yolo detection plugin
```

## Milestone 6：SAM 和 PIDNet Rust Plugins

完成：

* SAM Contract；
  -SAM ONNX 路径；
  -embedding cache；
  -MaskSet；
  -PIDNet ONNX；
  -SemanticMask；
  -测试；
  -Geometry Safety 集成。

提交：

```text
feat(plugin): add rust segmentation model plugins
```

## Milestone 7：RF-DETR 与 LocateAnything

完成：

* RF-DETR Rust 可行路径；
  -ONNX Contract；
  -真实或 live-conditional 状态；
  -LocateAnything Rust 可行性审计；
  -禁止 Python fallback；
  -Registry；
  -文档。

提交：

```text
feat(plugin): model advanced detectors through rust plugin contracts
```

## Milestone 8：GUI、TUI 和 Agent 集成

完成：

* Settings Plugins；
  -安装向导；
  -权重配置；
  -测试；
  -模型选择；
  -Agent 发现；
  -Unresolved Binding；
  -引用保护；
  -无障碍；
  -E2E。

提交：

```text
feat(ui): manage installable rust expert model plugins
```

## Milestone 9：迁移和 Release

完成：

-旧 Python Worker 迁移；
-旧 HTTP Binding 兼容；
-Rust-only Release；
-全量测试；
-文档；
-课程演示；
-验收矩阵。

提交：

```text
test(release): validate rust expert model plugin alpha
```

---

# 四十四、必须完成的测试

## Case 1：Dummy Plugin 安装

```text
pack
→ verify
→ install
→ start
→ health
→ infer
→ Artifact
→ stop
→ uninstall
```

不得修改 Core。

## Case 2：插件崩溃

```text
Plugin panic
→ Host 记录 Crashed
→ Run 获得结构化错误
→ AnnotAgent Server 继续运行
```

## Case 3：缺少权重

```text
Install SAM plugin
→ NeedsWeights
→ 不进入 Model Selector
→ Provision weights
→ hash check
→ Smoke Test
→ Ready
```

## Case 4：版本并存

```text
v1 已被 Published Workflow 使用
→ 安装 v2
→ v1 继续可运行
→ v2 只供新 Draft 选择
```

## Case 5：卸载保护

引用中的插件版本不能删除。

## Case 6：安全

验证插件：

* 看不到 Provider Secret；
  -看不到数据库路径；
  -看不到任意 Project 路径；
  -只能收到请求图片；
  -无法绑定非 loopback；
  -无 Token 请求被拒绝。

## Case 7：YOLO

```text
Image
→ YOLO Rust Plugin
→ DetectionSet
→ Filter
→ Geometry Decision
→ Review / Commit
```

## Case 8：SAM

```text
Coarse Detection
→ Box Prompt
→ SAM Rust Plugin
→ MaskSet
→ MaskToBBox
→ Geometry Evaluation
```

## Case 9：Agent

缺少 PromptedSegmentation 时：

```text
Blocked Draft
→ 建议安装兼容插件
```

安装后：

```text
Retry from saved draft
→ 发现 Ready Model
→ 加入合法 SAM 链
```

Agent 不自行安装。

## Case 10：无 Python

CI、运行日志和进程树中不出现 Python。

---

# 四十五、最终测试命令

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

插件：

```bash
cargo test -p annotagent-plugin-api
cargo test -p annotagent-plugin-sdk
cargo test -p annotagent-plugin-host
cargo test -p annotagent-model-runtime-onnx
```

E2E：

1. 安装 Dummy Plugin；
2. 启动；
3. 推理；
   4.取消；
   5.崩溃；
   6.重启；
4. NeedsWeights；
5. Provision；
   9.更新；
   10.版本并存；
   11.卸载保护；
6. Agent 模型发现；
7. YOLO Workflow；
8. SAM Geometry Workflow；
9. Generic Project；
10. RoboCup Project；
    17.历史 Replay；
    18.无 Python 依赖。

---

# 四十六、文档

新增：

```text
docs/RUST_MODEL_PLUGINS.md
docs/RUST_PLUGIN_MANIFEST.md
docs/RUST_PLUGIN_SDK.md
docs/RUST_PLUGIN_SECURITY.md
docs/RUST_PLUGIN_PACKAGING.md
docs/RUST_ONNX_RUNTIME.md
docs/RUST_PLUGIN_VERSIONING.md
docs/RUST_PLUGIN_MIGRATION.md
docs/WRITING_A_RUST_MODEL_PLUGIN.md
docs/YOLO_RUST_PLUGIN.md
docs/SAM_RUST_PLUGIN.md
docs/RFDETR_RUST_PLUGIN.md
docs/PIDNET_RUST_PLUGIN.md
docs/LOCATE_ANYTHING_RUST_PLUGIN.md
docs/DEMO_RUST_PLUGIN_ALPHA.md
```

更新：

```text
README.md
docs/DESIGN.md
docs/CORE_AND_SKILLS.md
docs/AGENT_LOOP.md
docs/GUIDED_EXPERIENCE.md
docs/VLM_GEOMETRY_SAFETY.md
docs/KNOWN_LIMITATIONS.md
docs/COURSE_REQUIREMENTS.md
```

---

# 四十七、明确不做

本轮不做：

* Rust dylib 插件；
* Python Worker；
* Python SDK；
* pip 安装；
  -任意 Shell 安装脚本；
  -插件前端 JavaScript 注入；
  -自动接受许可证；
* Agent 自动安装插件；
* Agent 自动下载权重；
  -公共开放插件市场；
  -付费插件；
  -多用户插件共享；
  -跨机器 GPU 调度；
  -完整训练平台；
  -自动模型转换；
  -保证所有 Hugging Face 模型都能直接 Rust 运行；
  -重写 CUDA 或深度学习内核。

---

# 四十八、不得采用的假实现

禁止：

* Rust Host 实际启动 Python Worker；
* 给 Python Worker 套一个 Rust launcher；
* 使用 `Command::new("python")`；
* 把 Python HTTP 服务称为 Rust 插件；
* 仅新增 Manifest，没有可执行生命周期；
* 复制插件文件后就显示 Ready；
* 缺少权重时显示 Available；
* 自动下载未知权重；
* 不校验 checksum；
* 更新时覆盖旧版本；
* Published Workflow 只保存模型名；
* 插件崩溃拖死 AnnotAgent；
* 插件读取 Provider Key；
* Agent 自动安装插件；
* 为每个模型修改 Core enum；
* LocateAnything 无 Rust 实现时偷偷调用旧 Python Worker；
* 用 Mock 冒充真实 ONNX 推理；
* 为了赶进度删除 Geometry Safety；
* push；
  -修改 remote；
  -提交 API Key；
  -提交未经许可的大权重。

---

# 四十九、最终报告格式

最终报告必须包含：

## 1. Rust-only 边界

说明：

* 哪些部分使用 Rust；
  -使用了哪些原生推理库；
  -为什么没有 Python；
  -如何证明运行时没有 Python。

## 2. Plugin 架构

说明：

* Plugin API；
  -SDK；
  -Host；
  -Registry；
  -独立进程；
  -协议；
  -认证；
  -生命周期。

## 3. 安装和版本

说明：

* Package；
  -checksum；
  -signature；
  -安装；
  -权重；
  -版本并存；
  -更新；
  -卸载保护。

## 4. Model Registry

说明：

-插件如何产生 Model Profile；

* Workflow 固定哪些身份；
* Agent 如何发现模型；
* Skill 如何依赖 Capability。

## 5. 官方插件

分别说明：

```text
Dummy
Generic ONNX
YOLO
SAM
PIDNet
RF-DETR
LocateAnything
```

必须区分：

```text
Manifest
Rust implementation
Weights
Smoke Test
Ready status
Live-conditional
```

## 6. 安全

说明：

-进程隔离；
-权限；
-Secret；
-文件；
-网络；
-认证；
-崩溃；
-限制；
-未实现的强沙箱边界。

## 7. 测试

列出真实执行的命令和结果。

不得把未运行测试报告为通过。

## 8. Milestone 提交

列出：

```text
commit hash
commit message
milestone
```

## 9. 迁移

说明现有 Python Worker 如何移出活动运行路径，以及旧 Workflow 如何兼容。

## 10. 未完成内容

明确区分：

```text
未实现
已实现但未真实权重验证
外部环境阻塞
明确不属于本轮
```

不得使用：

```text
基本完成
理论上支持
应该可用
大概率能跑
```

## 11. Git 状态

说明：

-当前分支；
-工作区是否干净；
-领先远程提交数；
-未 push；
-remote 未修改。

---

# 五十、启动指令

将本文保存为：

```text
docs/execution/RUST_PLUGIN_MASTER_PROMPT.md
```

然后从 AnnotAgent 仓库根目录启动 Codex，并输入：

```text
阅读 docs/execution/RUST_PLUGIN_MASTER_PROMPT.md，并将其作为本次长期任务的最高目标。

先核验现有 HTTP Vision Protocol、Model Registry、Python Workers、Geometry Safety、Workflow Version、Artifact、Agent 和测试，不要盲信文档中的完成说明。

本次任务的硬要求是：

1. 专家模型插件的 Host、SDK、协议实现和官方插件代码全部使用 Rust；
2. 安装和运行路径不依赖 Python、pip、uv、conda 或 venv；
3. 插件以独立 Rust 进程运行，不使用 Rust dylib 热加载；
4. 当前 HTTP Vision Protocol 作为主要进程边界，兼容扩展而不是重新造协议；
5. 建立 Rust Plugin Manifest、Package、Registry 和生命周期；
6. 建立通用 Rust ONNX Runtime；
7. 先完成 Dummy Plugin 和 Generic ONNX Plugin；
8. 再完成至少一个真实 Rust ONNX 专家模型插件；
9. YOLO、SAM、RF-DETR、PIDNet 和 LocateAnything 都通过 Capability 表达，不进入 Core enum；
10. robocup.ball 只依赖 Capability；
11. Published Workflow 固定插件、模型、Contract 和权重版本；
12. 插件崩溃不得拖死 AnnotAgent；
13. 插件不得读取 Provider Secret 或任意 Workspace 文件；
14. Agent 可以发现插件模型，但不能安装插件、接受许可证或下载权重；
15. 保持 Geometry Safety、Batch、Replay、Review、Provider 和 Export 不回归。

允许 Rust 插件通过 Rust Binding 调用 ONNX Runtime、TensorRT、CUDA 等原生推理库，但不得启动 Python Worker。

从 Milestone 0 开始持续执行。

普通技术决策自行决定，并记录到 RUST_PLUGIN_DECISIONS.md。

每完成一个 Milestone：
1. 更新状态和验收证据；
2. 执行对应 Rust、Web、插件和 E2E 测试；
3. 修复回归；
4. 创建独立本地提交；
5. 继续下一 Milestone。

某个模型暂时没有可用的 Rust 推理路径时：
- 完成 Manifest；
-完成 Contract；
-完成 Registry；
-完成 Fixture；
-明确标记 live-conditional 或 unsupported；
-不得退回 Python；
-不得伪造真实推理完成。

不要 push。
不要修改 Git remote。
不要使用、恢复或提交任何对话中出现过的 API Key。
不要提交未经许可的大型权重。
不要执行 reset、rebase、amend 或破坏性 checkout。

只有 Release Blocking Acceptance Matrix 全部满足，或只剩明确记录的真实权重和硬件条件项时，才输出最终报告。
```
