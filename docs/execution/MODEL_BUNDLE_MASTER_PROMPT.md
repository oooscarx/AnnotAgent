# AnnotAgent Model Bundle Provisioning Alpha

你现在是 AnnotAgent 仓库的长期主实现工程师。

本次任务只解决一个明确问题：

> AnnotAgent 已经能够安装和加载 Rust 专家模型插件，但普通用户无法获得与插件兼容、经过验证的模型文件。当前 SAM 插件要求用户分别上传 `image_encoder.onnx` 和 `mask_decoder.onnx`，这把模型转换、张量契约、版本兼容和供应链验证责任错误地交给了普通用户。

最终必须把产品体验从：

```text
安装插件
→ 自己寻找若干 ONNX 文件
→ 猜测张量名称和形状是否兼容
→ 分别上传
→ 运行失败后再调试
```

改成：

```text
安装 Rust 插件
→ 查看兼容模型
→ 选择一个经过验证的模型包
→ 确认来源和许可证
→ 一键安装
→ 自动校验 Contract
→ 自动执行 Smoke Test
→ 注册为 Ready Model Profile
→ 在 Pipeline 中使用
```

本次版本名称：

```text
AnnotAgent Model Bundle Provisioning Alpha
```

本次新增的核心产物：

```text
.annotmodel
```

它是一个可验证、可版本化、可安装的模型资产包。

---

# 一、严格任务边界

本次任务不是重新开发插件系统。

以下现有能力如果已经真实工作，必须复用，不得重写：

* Rust Plugin API；
* Rust Plugin SDK；
* Rust Plugin Host；
* Rust Plugin Registry；
* `.annotplugin` 安装；
* 插件启动、停止和健康检查；
* 插件独立进程；
* Session Token；
* HTTP Vision Protocol；
* Model Profile；
* Capability Registry；
* Artifact Contract；
* Workflow Runtime；
* Workflow Static Validator；
* Published Workflow Version；
* Artifact lineage；
* Cache；
* Replay；
* Review；
* Geometry Safety；
* Batch；
* Pause、Resume、Cancel；
* Provider Registry；
* TUI；
* Web GUI；
* SQLite 历史和审计。

本次只补齐：

1. Model Bundle 格式；
2. Model Bundle Catalog；
3. Model Bundle 下载或导入；
4. 模型文件完整性验证；
5. Plugin 与 Model Bundle 兼容性验证；
6. ONNX Contract 验证；
7. Smoke Test；
8. Model Profile 自动注册；
9. 模型版本固定；
10. 模型卸载引用保护；
11. 普通用户的一键安装体验；
12. 当前 SAM 裸 ONNX 上传路径的迁移。

不要新增另一套 Plugin Host。

不要新增另一套 Model Registry。

不要新增另一套 HTTP 协议。

不要把插件和模型包重新合并成一个巨大归档。

---

# 二、开始前核验仓库

首先执行：

```bash
git status --short --branch
git log --oneline -20
```

然后运行当前基线：

```bash
cargo test --workspace --all-features
cargo build --workspace --all-features
```

检查：

```text
README.md
docs/
docs/execution/

crates/annotagent-core/
crates/annotagent-runtime/
crates/annotagent-application/
crates/annotagent-storage/
crates/annotagent-server/

crates/annotagent-plugin-api/
crates/annotagent-plugin-sdk/
crates/annotagent-plugin-host/
crates/annotagent-plugin-registry/

crates/annotagent-model-runtime-common/
crates/annotagent-model-runtime-onnx/

plugins/
web/
apps/annotagent/
workspace/
```

重点核验：

* 当前 `.annotplugin` 格式；
* 当前插件安装 API；
* 当前 Plugin Manifest；
* 当前 Model Profile；
* 当前 Plugin-backed Model Profile；
* 当前权重配置方式；
* 当前 SAM 插件所需文件；
* 当前插件 Contract；
* 当前模型 Smoke Test；
* 当前 Published Workflow 如何固定模型身份；
* 当前插件更新和卸载引用保护；
* 当前 GUI 的 Expert Model Plugins 页面；
* 当前 CLI 的 `plugin` 命令；
* 当前 Rust ONNX Runtime；
* 当前 Geometry Safety；
* 当前 Pipeline Builder 如何发现模型；
* 当前数据库 migration；
* 当前测试数量。

不要盲信文件名或先前报告。

必须区分：

```text
插件代码存在
插件已安装
插件进程可启动
模型资产已安装
模型 Contract 有效
模型 Smoke Test 通过
Model Profile Ready
Workflow 可以使用
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
使用或恢复任何对话中出现过的 API Key
提交未经许可的大型模型权重
用 Mock 推理冒充真实模型
```

---

# 三、长期执行状态

创建并持续维护：

```text
docs/execution/MODEL_BUNDLE_MASTER_PLAN.md
docs/execution/MODEL_BUNDLE_STATUS.md
docs/execution/MODEL_BUNDLE_DECISIONS.md
docs/execution/MODEL_BUNDLE_ACCEPTANCE.md
docs/execution/MODEL_BUNDLE_BLOCKERS.md
docs/execution/MODEL_BUNDLE_KNOWN_LIMITATIONS.md
```

`MODEL_BUNDLE_STATUS.md` 必须记录：

```text
当前 Milestone
已完成内容
正在进行内容
下一步
最近 Rust 测试
最近 Bundle 测试
最近 Plugin Conformance Test
最近真实模型 Smoke Test
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
3. 运行对应测试；
4. 修复回归；
5. 创建独立本地提交；
6. 继续下一个 Milestone；
7. 不等待用户确认。

---

# 四、插件与模型包的最终边界

必须在代码和文档中明确：

## 4.1 Plugin Package

```text
.annotplugin
```

负责：

* Rust 可执行程序；
* 模型加载逻辑；
* 预处理；
* 后处理；
* 推理 Runtime；
* Capability；
* Artifact Contract；
* Plugin 生命周期；
* Plugin 权限；
* Plugin 版本。

它回答：

> 这个模型家族应该如何运行？

## 4.2 Model Bundle

```text
.annotmodel
```

负责：

* 一个确定模型版本的模型文件；
* 多文件角色；
* 文件 SHA-256；
* 模型来源；
* 上游 checkpoint 身份；
* 导出信息；
* ONNX opset；
* 输入输出 Contract；
* 预处理参数；
* 后处理参数；
* 测试向量；
* 许可证；
* Plugin 兼容范围；
* 硬件要求；
* Bundle 版本。

它回答：

> 实际运行的是哪一组确定模型资产？

## 4.3 Model Instance

只有 Plugin 和 Model Bundle 绑定并验证后，才能形成可运行实例。

```text
Plugin Version
+
Model Bundle Version
+
Execution Provider
+
Smoke Test Result
=
Ready Model Instance
```

## 4.4 Model Profile

现有 Model Profile 继续作为用户和 Workflow 实际选择的模型对象。

Model Profile 引用 Ready Model Instance。

不要让 Project 直接引用插件目录或 ONNX 路径。

---

# 五、状态必须彻底分离

不得继续用单个：

```text
NeedsWeights
```

描述所有情况。

实现或整理三组状态。

## 5.1 Plugin Runtime Status

```rust
pub enum PluginRuntimeStatus {
    NotInstalled,
    Installed,
    Disabled,
    Starting,
    Ready,
    Unhealthy,
    Crashed,
    Incompatible,
}
```

这里的 `Ready` 只表示插件 Runtime 可以启动并报告能力，不代表已有模型资产。

## 5.2 Model Bundle Status

```rust
pub enum ModelBundleStatus {
    NotInstalled,
    AvailableInCatalog,
    LicenseAcceptanceRequired,
    Downloading,
    Importing,
    Verifying,
    Installed,
    IncompatiblePlugin,
    InvalidManifest,
    InvalidChecksum,
    InvalidContract,
    UnsupportedPlatform,
    Corrupted,
}
```

## 5.3 Model Instance Status

```rust
pub enum ModelInstanceStatus {
    Unresolved,
    MissingPlugin,
    MissingModelBundle,
    Preparing,
    SmokeTesting,
    Ready,
    FailedSmokeTest,
    PluginUnavailable,
    ContractMismatch,
    Disabled,
    Stale,
}
```

只有：

```text
ModelInstanceStatus::Ready
```

才能进入可运行 Model Profile 和 Published Workflow。

---

# 六、Model Bundle 格式

模型包后缀：

```text
.annotmodel
```

第一版使用确定性 `tar.zst` 或 ZIP。

如果现有 `.annotplugin` 已使用一种成熟归档格式，优先复用其安全解包逻辑。

包内结构：

```text
model.annotmodel/
├── annotagent-model.toml
├── files/
│   ├── image_encoder.onnx
│   └── mask_decoder.onnx
├── contracts/
│   ├── model-contract.json
│   ├── encoder-contract.json
│   └── decoder-contract.json
├── transforms/
│   ├── preprocessing.json
│   └── postprocessing.json
├── tests/
│   ├── input-image.png
│   ├── prompts.json
│   ├── expected-summary.json
│   └── tolerances.json
├── licenses/
│   ├── MODEL-LICENSE
│   └── SOURCE-NOTICE
├── checksums.json
└── signatures/
    └── bundle.ed25519.sig
```

签名在 Alpha 中可以是官方 Bundle 的必需项、用户自建 Bundle 的可选项。

所有 Bundle 必须有文件 SHA-256。

---

# 七、Model Bundle Manifest

实现：

```rust
pub struct ModelBundleManifest {
    pub schema_version: String,

    pub id: ModelBundleId,
    pub version: Version,
    pub display_name: String,
    pub description: Option<String>,

    pub model_family: String,
    pub architecture: String,
    pub format: ModelFormat,
    pub variant: String,

    pub capabilities: BTreeSet<ModelCapability>,
    pub compatible_plugins: Vec<PluginCompatibilityRequirement>,

    pub files: Vec<ModelBundleFile>,
    pub contracts: Vec<ModelContractReference>,

    pub source: ModelSourceMetadata,
    pub export: ModelExportMetadata,
    pub runtime: ModelRuntimeMetadata,
    pub license: ModelLicenseMetadata,
    pub test_suite: ModelTestSuiteReference,
}
```

模型格式：

```rust
pub enum ModelFormat {
    Onnx,
    Safetensors,
    Native,
}
```

本次 Release Blocking 只要求 `Onnx` 真实可运行。

其他格式只能在已有真实 Rust Runtime 支持时标记可用。

---

# 八、多文件模型角色不得写死进 Core enum

不要在 Core 中增加：

```rust
enum ModelFileRole {
    SamImageEncoder,
    SamMaskDecoder,
    YoloModel,
}
```

这会让每新增一个模型都修改 Core。

改为受校验字符串新类型：

```rust
pub struct ModelFileRole(String);
```

Manifest 示例：

```toml
[[files]]
role = "image_encoder"
path = "files/image_encoder.onnx"
sha256 = "..."
size_bytes = 364000000

[[files]]
role = "mask_decoder"
path = "files/mask_decoder.onnx"
sha256 = "..."
size_bytes = 16000000
```

Plugin Manifest 声明自己需要哪些角色：

```toml
[[models]]
id = "sam-compatible"

required_file_roles = [
  "image_encoder",
  "mask_decoder"
]
```

兼容性 Resolver 检查：

* 角色是否齐全；
* 是否重复；
* Contract 是否匹配；
* Bundle capability 是否匹配；
* Plugin 版本是否满足；
* ONNX opset 是否支持。

---

# 九、模型来源和导出信息

Manifest 必须记录来源：

```rust
pub struct ModelSourceMetadata {
    pub upstream_project: String,
    pub upstream_model_id: String,
    pub upstream_version: Option<String>,
    pub upstream_checkpoint_sha256: Option<String>,
    pub source_url: Option<Url>,
}
```

记录导出过程：

```rust
pub struct ModelExportMetadata {
    pub exporter_name: String,
    pub exporter_version: String,
    pub exporter_revision: Option<String>,
    pub export_date: Option<DateTime<Utc>>,
    pub opset: Option<u32>,
    pub numerical_validation: Option<NumericalValidationSummary>,
}
```

用户安装和运行路径必须保持 Rust-only。

Bundle 可以由上游维护者使用其官方工具生成，但 AnnotAgent 不得在普通用户机器上：

* 启动 Python；
* 安装 PyTorch；
* 执行导出脚本；
* 从 `.pt` 猜测模型图；
* 自动转换未知 checkpoint。

不要实现一个虚假的通用：

```bash
annotagent convert any-model.pt
```

---

# 十、Rust-only 硬约束

本次用户安装与推理路径中不得出现：

```text
python
python3
pip
uv
conda
venv
FastAPI
Pydantic
requirements.txt
```

禁止：

```rust
Command::new("python")
Command::new("python3")
Command::new("pip")
Command::new("uv")
```

Bundle 安装器、Verifier、Catalog Client、Smoke Test 和 Plugin Runtime 全部使用 Rust。

增加 CI 检查：

```bash
rg -n \
  "Command::new\\(\"python|Command::new\\(\"python3|pip install|uv run|conda|venv" \
  crates/annotagent-plugin-* \
  crates/annotagent-model-* \
  plugins/
```

允许这些词只存在于：

```text
docs/legacy/
upstream export provenance
migration notes
```

不允许活动安装或推理路径依赖它们。

---

# 十一、Model Bundle Pack 工具

新增：

```bash
annotagent models bundle pack <directory> --output <file.annotmodel>
```

它只负责打包已经准备好的模型资产。

不得负责模型格式转换。

打包前检查：

* Manifest；
  -文件存在；
  -文件角色；
  -路径安全；
  -文件大小；
  -SHA-256；
  -Contract；
  -Test Vector；
  -License；
  -Plugin compatibility；
  -是否有未知文件；
  -是否包含 symlink；
  -是否包含绝对路径。

支持：

```bash
annotagent models bundle inspect <file.annotmodel>
annotagent models bundle verify <file.annotmodel>
```

`verify` 不安装，只执行静态验证。

---

# 十二、可信 Model Catalog

实现一个精简的 Curated Catalog，不做公共 Marketplace。

Catalog 第一版支持：

```text
Built-in local catalog fixture
+
可配置的 HTTPS curated catalog
```

Catalog 格式：

```rust
pub struct ModelCatalog {
    pub schema_version: String,
    pub catalog_id: String,
    pub generated_at: DateTime<Utc>,
    pub entries: Vec<ModelCatalogEntry>,
    pub signature: Option<CatalogSignature>,
}
```

条目：

```rust
pub struct ModelCatalogEntry {
    pub bundle_id: ModelBundleId,
    pub bundle_version: Version,
    pub display_name: String,
    pub capabilities: BTreeSet<ModelCapability>,

    pub compatible_plugins: Vec<PluginCompatibilityRequirement>,
    pub platform_requirements: Vec<PlatformRequirement>,

    pub bundle_url: Url,
    pub bundle_sha256: String,
    pub bundle_size_bytes: u64,

    pub license_summary: ModelLicenseSummary,
    pub publisher: PublisherIdentity,
}
```

Catalog 只提供元数据和固定 Bundle 地址。

不允许：

* 任意安装脚本；
* Shell；
  -动态 JavaScript；
  -重定向到本地地址；
  -未固定 SHA-256 的下载；
  -运行时从不受信任网页抓取模型。

---

# 十三、可信下载规则

Curated Bundle 下载必须：

1. 使用 HTTPS；
2. 禁止 URL 用户名和密码；
3. 禁止 file URL；
4. 禁止 loopback；
5. 禁止私有网络；
6. 禁止危险重定向；
7. 限制最大下载大小；
8. 流式写入临时文件；
9. 同时计算 SHA-256；
10. 哈希不匹配立即删除；
11. 只在校验完成后原子安装；
12. 支持取消；
13. 支持进度；
14. 下载中断不留下 Ready 状态；
15. 不执行下载内容。

本地 Bundle 导入不需要网络，但仍然执行同样的 Manifest、Contract、License 和 Smoke Test。

---

# 十四、许可证确认

模型包安装前必须显示：

```text
Model name
Upstream source
Model license
Redistribution status
Commercial-use status
Publisher
Bundle digest
Disk usage
```

许可证确认必须保存：

```rust
pub struct ModelLicenseAcceptance {
    pub bundle_id: ModelBundleId,
    pub bundle_version: Version,
    pub license_digest: String,
    pub accepted_at: DateTime<Utc>,
    pub accepted_by: LicenseAcceptanceActor,
}
```

不保存无意义的：

```text
accepted = true
```

许可证内容变化后必须重新确认。

Pipeline Builder Agent 不得接受许可证。

Agent 不得安装 Model Bundle。

---

# 十五、内容寻址模型存储

使用内容摘要保存模型：

```text
<annotagent-data>/
└── models/
    └── sha256/
        └── ab/
            └── abcdef.../
                ├── manifest/
                ├── files/
                ├── contracts/
                ├── tests/
                └── verification.json
```

数据库中保存：

* Bundle ID；
* Bundle Version；
* Bundle Digest；
  -文件 Digest；
  -安装时间；
  -来源；
  -许可证确认；
  -验证状态；
  -引用计数；
  -兼容插件；
  -模型实例；
  -Smoke Test。

多个 Project 和插件版本可以共享同一个内容地址。

不得为每个 Project 复制一份相同模型。

---

# 十六、原子安装生命周期

安装一个 Bundle 必须执行：

```text
1. Resolve catalog entry or local package
2. Download / copy to staging
3. Verify archive safety
4. Verify package checksum
5. Read manifest
6. Verify every model file checksum
7. Verify license acceptance
8. Verify platform requirements
9. Find compatible installed plugins
10. Verify required model-file roles
11. Verify Artifact Contract
12. Inspect ONNX model metadata
13. Create staged Model Instance
14. Start compatible Rust plugin
15. Load model
16. Run fixed Smoke Test
17. Validate output against tolerance
18. Write verification report
19. Atomically activate bundle
20. Register or update Model Profile
21. Mark Model Instance Ready
```

任一步失败：

* 不产生 Ready Model Profile；
* 不覆盖旧版本；
* 保留结构化错误；
* 清理临时文件；
* 不影响现有 Workflow；
* 用户可以重试。

---

# 十七、ONNX Contract 验证

如果 Rust ONNX Runtime 已存在，复用它读取：

* input names；
* output names；
* shapes；
* dtypes；
* dynamic dimensions；
* opset；
* external data files。

Model Contract 示例：

```json
{
  "contract_version": "1",
  "roles": {
    "image_encoder": {
      "inputs": [
        {
          "name": "input_image",
          "dtype": "float32",
          "shape": [1, 3, 1024, 1024]
        }
      ],
      "outputs": [
        {
          "name": "image_embeddings",
          "dtype": "float32",
          "shape": [1, 256, 64, 64]
        }
      ]
    }
  }
}
```

Contract 校验必须支持：

* 精确名称；
  -可选别名；
  -静态维度；
  -动态维度；
  -dtype；
  -角色；
  -多文件模型之间的连接；
  -Plugin 期望 Contract Hash。

文件名不能作为兼容性的唯一依据。

---

# 十八、Smoke Test

每个正式 Model Bundle 必须包含固定 Smoke Test。

```rust
pub struct ModelBundleSmokeTest {
    pub test_id: String,
    pub input_artifacts: Vec<TestArtifactReference>,
    pub expected: ExpectedOutputSummary,
    pub tolerances: OutputTolerances,
}
```

测试不应要求逐像素完全相同，除非模型和 Runtime 能保证。

应验证：

-模型能加载；
-输入 Contract 正确；
-输出 Contract 正确；
-输出值有限；
-坐标合法；
-Mask 尺寸合法；
-非空程度符合预期；
-输出摘要落入容差；
-推理耗时未明显失控；
-没有 panic；
-没有插件崩溃。

SAM 类测试至少包含：

```text
Test image
Box prompt
Expected non-empty mask
Expected mask inside broad reference region
Expected bounding box tolerance
```

只有 Smoke Test 通过才标记 Ready。

---

# 十九、Plugin Compatibility Resolver

实现：

```rust
pub struct ModelBundleCompatibilityResolver;
```

输入：

* Plugin Manifest；
  -Plugin Version；
  -Plugin Runtime status；
  -Model Bundle Manifest；
  -平台；
  -执行 Provider；
  -Contract；
  -已配置资源。

输出：

```rust
pub enum ModelBundleCompatibility {
    Compatible {
        plugin_versions: Vec<PluginVersionRef>,
    },
    MissingPlugin,
    IncompatiblePluginVersion,
    MissingFileRole,
    ContractMismatch,
    UnsupportedFormat,
    UnsupportedPlatform,
    UnsupportedExecutionProvider,
    MissingLicenseAcceptance,
}
```

GUI 必须显示具体原因。

不要只显示：

```text
Incompatible
```

---

# 二十、Model Instance 与 Model Profile

新增或整理：

```rust
pub struct InstalledModelInstance {
    pub id: ModelInstanceId,

    pub plugin_id: PluginId,
    pub plugin_version: Version,
    pub plugin_package_digest: String,

    pub model_bundle_id: ModelBundleId,
    pub model_bundle_version: Version,
    pub model_bundle_digest: String,

    pub model_variant: String,
    pub model_file_digests: BTreeMap<ModelFileRole, String>,

    pub execution_provider: String,
    pub capability_contract_hash: String,

    pub status: ModelInstanceStatus,
    pub smoke_test_id: Option<String>,
    pub smoke_test_result: Option<SmokeTestResult>,
}
```

Ready Model Instance 自动产生或更新现有 Model Profile。

Model Profile 不直接保存裸文件路径。

---

# 二十一、Published Workflow 固定模型资产

现有 Published Workflow Version 必须继续不可变。

新增固定信息：

```rust
pub struct PublishedModelAssetReference {
    pub plugin_id: PluginId,
    pub plugin_version: Version,
    pub plugin_package_digest: String,

    pub model_bundle_id: ModelBundleId,
    pub model_bundle_version: Version,
    pub model_bundle_digest: String,

    pub model_instance_id: ModelInstanceId,
    pub model_profile_id: ModelProfileId,
    pub model_profile_revision: u64,

    pub model_file_digests: BTreeMap<ModelFileRole, String>,
    pub capability_contract_hash: String,
    pub execution_provider: String,
}
```

要求：

* Bundle 更新不改变旧 Workflow；
* Plugin 更新不改变旧 Workflow；
  -模型文件替换不改变旧 Workflow；
  -旧模型删除前检查引用；
  -Replay 可以定位精确模型；
  -模型缺失时历史仍可查看；
  -新 Run 显示精确安装要求。

---

# 二十二、删除和垃圾回收

模型 Bundle 删除前检查：

* Published Workflow；
  -Workflow Draft；
  -Project Model Binding；
  -Active Run；
  -Replay；
  -Calibration；
  -Model Profile；
  -Artifact Cache；
  -历史 Run。

存在引用时：

```text
Cannot remove this model bundle.

Referenced by:
- Workflow robocup-ball@v2
- Calibration geometry-17
- 4 historical runs
```

允许：

```text
Disable for new workflows
Remove executable cache while preserving metadata
Garbage collect unreferenced bundles
```

实现：

```bash
annotagent models gc
```

只删除：

* 无引用 Bundle；
  -失败的 staging；
  -过期下载缓存；
  -无引用测试缓存。

---

# 二十三、普通用户 CLI

用户级命令：

```bash
annotagent models catalog
annotagent models search prompted-segmentation
annotagent models show <bundle-id>
annotagent models install <bundle-id>@<version>
annotagent models import <file.annotmodel>
annotagent models list
annotagent models test <model-instance-id>
annotagent models disable <model-instance-id>
annotagent models enable <model-instance-id>
annotagent models remove <bundle-id>@<version>
annotagent models references <bundle-id>@<version>
annotagent models doctor <model-instance-id>
annotagent models gc
```

开发者命令：

```bash
annotagent models bundle pack <directory> --output <file>
annotagent models bundle inspect <file>
annotagent models bundle verify <file>
annotagent models catalog build <directory>
annotagent models catalog verify <catalog-file>
```

不要让普通用户使用：

```text
--encoder
--decoder
--input-name
--output-name
```

这些属于 Bundle 开发者。

---

# 二十四、GUI 信息架构

入口继续位于：

```text
Settings → Expert Model Plugins
```

插件详情中增加：

```text
Runtime
Compatible Models
Installed Models
Model Setup
References
```

## 24.1 插件已安装但没有模型

显示：

```text
SAM Prompted Segmentation

Runtime
Installed

Model
No compatible model installed

This plugin cannot run until a verified model is installed.

[Install compatible model]
```

不要显示两个裸 ONNX 上传框作为主要操作。

## 24.2 兼容模型列表

显示：

```text
Recommended

Efficient prompted segmentation model
Compatible with this Mac
Rust + ONNX Runtime
Disk size: ...
License: ...
Status: Ready to install

[Install model]
```

其他模型：

```text
SAM 2 model
Labs
No verified bundle is currently published for this platform
```

## 24.3 安装向导

```text
Select model
→ Review source
→ Review license
→ Check compatibility
→ Download
→ Verify
→ Smoke Test
→ Ready
```

页面只允许一个主操作。

## 24.4 安装进度

显示真实阶段：

```text
Downloading model
Verifying files
Checking ONNX contract
Starting Rust plugin
Running sample inference
Registering model
```

不要用一个永远写着 `Installing...` 的旋转图标掩盖全部错误。

---

# 二十五、专家级本地导入

提供：

```text
Import .annotmodel
```

作为高级操作。

不再默认提供：

```text
Upload image encoder ONNX
Upload mask decoder ONNX
```

如果必须保留旧入口：

* 放入 `Legacy manual provisioning`；
  -默认折叠；
  -明确标记不推荐；
  -只接受已经有 Contract 和 Hash 的文件；
  -不能让无 Contract 的裸文件进入 Ready；
  -后续版本删除。

---

# 二十六、当前 SAM 插件迁移

当前行为：

```text
SAM plugin
→ 用户分别上传 encoder.onnx 和 decoder.onnx
```

迁移为：

```text
SAM plugin runtime
→ Compatible Model Bundle Requirement
```

数据库迁移要求：

1. 保留现有 Plugin 安装；
2. 识别旧的裸 ONNX 引用；
3. 旧文件存在时标记：

   ```text
   LegacyUnbundledModel
   ```
4. 不自动声称其为可信 Bundle；
5. 提供：

   ```text
   Create local model bundle
   ```
6. Rust 工具读取现有文件；
7. 用户补充来源、许可证和 Contract；
8. 执行 Hash 和 Smoke Test；
9. 成功后转换为 Local `.annotmodel`；
10. 转换失败不删除旧文件；
11. 没有旧文件时显示：

    ```text
    No verified model bundle installed
    ```
12. 不再告诉用户自行搜索两个同名 ONNX。

---

# 二十七、第一批模型交付策略

不要把 SAM 2 作为首个真实 Model Bundle 的唯一 Release Blocker。

SAM 2 保留为：

```text
Labs
```

直到存在：

* 合法来源；
  -明确许可证；
* Rust Runtime 可执行；
  -完整 encoder/decoder Contract；
  -固定 SHA-256；
  -真实 Smoke Test；
  -Plugin compatibility；
  -可复现 Bundle。

本次 Release Blocking 要求至少交付：

```text
一个真实可安装的 PromptedSegmentation Model Bundle
```

模型选择原则：

1. 上游直接提供或允许分发 ONNX；
2. 许可证明确；
3. Rust ONNX Runtime 可加载；
4. 支持 box 或 point prompt；
5. 可以输出 Mask；
6. 文件身份可固定；
7. 体积适合 Developer Preview；
8. 有固定测试向量。

优先审计：

```text
EfficientSAM 或其他存在合法、可验证 ONNX 产物的轻量 prompted-segmentation 模型
```

必须从官方来源核验：

-代码许可证；
-权重许可证；
-ONNX 来源；
-Redistribution；
-输入输出 Contract。

如果该候选不满足要求：

* 不使用来源不明的 ONNX；
* 不将第三方随机模型重命名为官方 Bundle；
* 选择另一个合法模型；
  -或将真实 Bundle 标记为 live-conditional；
  -但 Bundle 系统、Fixture Bundle 和所有协议测试仍必须完成。

---

# 二十八、Fixture Model Bundle

为了保证 CI 和离线 Demo，必须提供一个极小 Fixture Bundle：

```text
org.annotagent.models.fixture-prompted-segmentation
```

要求：

* 小型合法 ONNX；
  -不依赖外部下载；
  -只用于测试；
  -界面明确显示 `Fixture`;
  -不能用于正式发布 Workflow；
  -可以验证：

  * Bundle 安装；
    -Contract；
    -Smoke Test；
    -Plugin 绑定；
    -Artifact；
    -删除；
    -版本固定。

Fixture 不能冒充真实 SAM。

---

# 二十九、Catalog 初期不要做 Marketplace

本次只做：

```text
Official Curated Catalog
+
Local Bundle Import
```

不做：

-任意用户上传；
-评分；
-评论；
-付费模型；
-搜索排名；
-远程代码执行；
-第三方前端扩展；
-自动信任未知发布者。

Catalog 可以使用仓库内置 Fixture 和可配置远程索引。

不要为了一个模型交付问题先建设一座没有摊主的模型商场。

---

# 三十、Pipeline Builder Agent 集成

Pipeline Builder Agent 可以读取：

```text
已安装 Plugin
可用 Catalog Bundle
Ready Model Instance
Missing Model Requirement
Capability
Contract
Platform Compatibility
License summary
```

Agent 可以：

```text
list_ready_models
list_compatible_model_bundles
inspect_model_bundle_summary
create_unresolved_model_requirement
```

Agent不能：

```text
install_model_bundle
download_model_bundle
accept_model_license
import_local_bundle
delete_model_bundle
run_billable_probe
```

如果缺少 Prompted Segmentation：

```text
Pipeline draft saved with mandatory human review.

Optional improvement:
A compatible prompted-segmentation model can refine coarse boxes.

[Install compatible model]
```

用户安装完成后：

```text
Retry from saved draft
```

Agent 重新读取 Model Registry，然后才能生成：

```text
Detection
→ Box Prompt
→ Prompted Segmentation
→ Mask to BBox
→ Geometry Evaluation
```

---

# 三十一、Geometry Safety 必须保持

当前几何安全规则不得退化。

即使模型包安装成功，也不能自动认为：

```text
模型存在
→ 几何结果一定安全
```

Prompted Segmentation Model Profile 应声明：

```text
Geometry Semantics: RefinedGeometry
Calibration Status: Uncalibrated
```

仍需：

* Geometry Evaluation；
  -Project Calibration；
  -或 Human Review。

Bundle 安装解决的是：

```text
模型能否稳定、可复现地运行
```

不是：

```text
模型在所有图片上是否正确
```

这两件事不能因为都显示绿色图标就被合并。

---

# 三十二、API

实现或整理：

```text
GET    /api/model-catalogs
POST   /api/model-catalogs/refresh
GET    /api/model-catalogs/:catalogId

GET    /api/model-bundles
GET    /api/model-bundles/available
GET    /api/model-bundles/:bundleId/:version

POST   /api/model-bundles/install
POST   /api/model-bundles/import
POST   /api/model-bundles/:bundleId/:version/verify
POST   /api/model-bundles/:bundleId/:version/test

POST   /api/model-bundles/:bundleId/:version/enable
POST   /api/model-bundles/:bundleId/:version/disable
DELETE /api/model-bundles/:bundleId/:version

GET    /api/model-bundles/:bundleId/:version/references
GET    /api/model-bundles/:bundleId/:version/compatibility

POST   /api/model-bundles/:bundleId/:version/license-acceptance

GET    /api/plugins/:pluginId/:pluginVersion/compatible-model-bundles
GET    /api/model-instances
GET    /api/model-instances/:instanceId
POST   /api/model-instances/:instanceId/test
```

如果当前 API 已有相同功能，应扩展，不能制造平行接口。

API 不返回：

-本地绝对模型路径；
-Session Token；
-Provider Secret；
-许可证确认人的敏感身份；
-未脱敏下载 Header。

---

# 三十三、Storage

至少保存：

```text
model_catalogs
model_catalog_entries
model_bundles
model_bundle_files
model_bundle_contracts
model_bundle_installations
model_bundle_verifications
model_bundle_smoke_tests
model_bundle_license_acceptances
model_instances
model_instance_health
model_bundle_references
model_bundle_events
```

数据写操作使用事务。

安装激活必须原子完成。

下载、校验和 Smoke Test 失败不得留下 Ready 状态。

---

# 三十四、安全测试

必须覆盖：

1. Archive path traversal；
2. Zip Slip；
3. symlink；
4. hard link；
   5.绝对路径；
   6.重复文件名；
   7.大小写冲突；
   8.解压炸弹；
   9.超大文件；
   10.超多文件；
   11.Manifest 缺失；
   12.Manifest 重复；
   13.未知字段；
   14.文件 Hash 错误；
   15.Bundle Hash 错误；
   16.签名错误；
   17.许可证未确认；
   18.HTTP 重定向；
   19.loopback 下载地址；
   20.私网下载地址；
   21.下载超时；
   22.下载取消；
   23.磁盘空间不足；
   24.ONNX Contract 错误；
   25.ONNX 外部数据缺失；
   26.Smoke Test 崩溃；
   27.Plugin 崩溃；
   28.安装中进程退出；
   29.并发安装同一 Bundle；
   30.删除被引用 Bundle。

---

# 三十五、用户体验验收

普通用户在已有 Rust 插件的情况下，应能完成：

```text
打开插件详情
→ Install compatible model
→ 查看模型来源和许可证
→ 确认
→ 查看下载进度
→ 自动验证
→ 自动 Smoke Test
→ Model Ready
→ 返回原 Draft
→ 选择模型
```

用户不需要：

* 自己搜索 ONNX；
  -运行导出脚本；
  -安装 Python；
  -理解 encoder/decoder 张量名称；
  -分别上传多个裸文件；
  -手工计算 SHA-256；
  -编辑 Model Profile JSON；
  -重启 AnnotAgent；
  -复制模型路径。

---

# 三十六、Milestone 计划

## Milestone 0：基线与当前 SAM 缺口回归

完成：

* 复现当前 SAM 插件安装后缺少两个 ONNX；
* 记录当前 GUI 路径；
* 记录当前数据库；
* 记录当前 Model Profile；
* 建立状态文档；
* 建立测试基线。

提交：

```text
test(models): reproduce unprovisioned sam plugin experience
```

## Milestone 1：Model Bundle API 与 Manifest

完成：

* `.annotmodel`；
* Manifest；
  -多文件角色；
  -来源；
  -导出信息；
  -License；
  -Contract；
  -Test Suite；
  -序列化；
  -校验；
  -测试。

提交：

```text
feat(models): define versioned installable model bundles
```

## Milestone 2：Bundle Verifier 与安全解包

完成：

-归档验证；
-Path Traversal；
-Checksum；
-Signature；
-License digest；
-大小限制；
-原子 staging；
-安全测试。

提交：

```text
feat(models): verify model bundles before installation
```

## Milestone 3：Catalog 与 Provisioner

完成：

-本地 Catalog；
-远程 Curated Catalog；
-安全下载；
-进度；
-取消；
-内容寻址存储；
-数据库；
-API；
-测试。

提交：

```text
feat(models): install curated bundles into content-addressed storage
```

## Milestone 4：Plugin Compatibility 与 Model Instance

完成：

* Plugin required file roles；
  -Compatibility Resolver；
  -ONNX Contract；
  -Model Instance；
  -Model Profile 自动注册；
  -Ready 状态；
  -测试。

提交：

```text
feat(models): bind verified bundles to rust model plugins
```

## Milestone 5：Smoke Test 和版本固定

完成：

* Bundle Test Vector；
  -Plugin 推理测试；
  -容差；
  -Verification report；
  -Published Workflow reference；
  -删除保护；
  -GC；
  -测试。

提交：

```text
feat(models): validate and pin model assets for reproducible workflows
```

## Milestone 6：SAM 旧路径迁移与 GUI

完成：

-移除默认裸 ONNX 上传；
-显示 Compatible Models；
-安装向导；
-进度；
-错误；
-LegacyUnbundledModel；
-Create Local Bundle；
-状态恢复；
-E2E。

提交：

```text
feat(ui): replace raw onnx uploads with verified model installation
```

## Milestone 7：首个真实 Prompted Segmentation Bundle

完成：

-官方来源审计；
-许可证审计；
-真实 ONNX Bundle；
-真实 Rust Plugin 推理；
-Smoke Test；
-Geometry Safety Workflow；
-模型说明；
-若 SAM 2 尚不可行则保持 Labs。

提交：

```text
feat(models): ship the first verified prompted-segmentation bundle
```

## Milestone 8：Agent、TUI、迁移与 Release

完成：

-Pipeline Builder setup requirement；
-安装后 Retry；
-TUI；
-CLI；
-旧项目迁移；
-全部回归；
-文档；
-Release Matrix。

提交：

```text
test(release): validate model bundle provisioning alpha
```

---

# 三十七、Release Blocking Acceptance Matrix

## A. 架构

* [ ] Plugin Package 与 Model Bundle 是独立实体。
* [ ] Plugin 描述如何运行模型。
* [ ] Model Bundle 描述具体模型资产。
* [ ] Project 不保存裸 ONNX 路径。
* [ ] Model Profile 引用 Ready Model Instance。
* [ ] Core 不包含 SAM 专用文件角色 enum。
* [ ] 新多文件模型不需要修改 Core。

## B. Rust-only

* [ ] 普通用户安装不依赖 Python。
* [ ] Bundle Verifier 使用 Rust。
* [ ] Catalog Client 使用 Rust。
* [ ] 下载器使用 Rust。
* [ ] Smoke Test 使用 Rust Plugin。
* [ ] 推理不启动 Python。
* [ ] 不执行转换脚本。
* [ ] 活动路径没有 pip、uv、conda 或 venv。

## C. Bundle

* [ ] `.annotmodel` 可 pack。
* [ ] `.annotmodel` 可 inspect。
* [ ] `.annotmodel` 可 verify。
* [ ] 支持多模型文件。
* [ ] 每个文件有 role、size 和 SHA-256。
* [ ] 有来源信息。
* [ ] 有许可证。
* [ ] 有 Contract。
* [ ] 有 Test Vector。
* [ ] 非法 Bundle 无法安装。

## D. Catalog 和下载

* [ ] 支持本地 Curated Catalog。
* [ ] 支持安全 HTTPS Catalog。
* [ ] 下载前显示大小和许可证。
* [ ] 固定 SHA-256。
* [ ] 支持取消。
* [ ] 中断后不留下 Ready 状态。
* [ ] 禁止私网、loopback 和 file URL。
* [ ] 不执行任意脚本。

## E. Plugin 兼容

* [ ] Plugin 声明 required file roles。
* [ ] Bundle 提供 file roles。
* [ ] Resolver 检查版本。
* [ ] Resolver 检查 Capability。
* [ ] Resolver 检查 Contract。
* [ ] Resolver 检查平台。
* [ ] Resolver 给出具体错误。
* [ ] Plugin Ready 不等于 Model Ready。

## F. Smoke Test

* [ ] Bundle 包含固定测试向量。
* [ ] Model 能真实加载。
* [ ] Plugin 能真实推理。
* [ ] 输出 Contract 正确。
* [ ] 输出无 NaN 和 Infinity。
* [ ] 测试结果有容差。
* [ ] Smoke Test 失败不能进入 Ready。
* [ ] 测试记录可审计。

## G. 版本和历史

* [ ] Published Workflow 固定 Plugin Version。
* [ ] 固定 Plugin Digest。
* [ ] 固定 Bundle Version。
* [ ] 固定 Bundle Digest。
* [ ] 固定 Model Profile Revision。
* [ ] 固定文件 Digest。
* [ ] 更新不改变旧 Workflow。
* [ ] 被引用 Bundle 无法直接删除。
* [ ] 历史 Run 在 Bundle 缺失时仍可查看。

## H. SAM 体验

* [ ] SAM 页面不再要求普通用户自己寻找两个 ONNX。
* [ ] 不再把裸 ONNX 双上传作为默认路径。
* [ ] 显示 Rust Plugin 状态。
* [ ] 显示 Model Bundle 状态。
* [ ] 显示兼容模型。
* [ ] 没有 Bundle 时显示真实缺口。
* [ ] SAM 2 未验证时保持 Labs。
* [ ] 至少一个真实 PromptedSegmentation Bundle 可安装并推理。

## I. Agent

* [ ] Agent 可以看到 Ready Model。
* [ ] Agent 可以看到缺失 Capability。
* [ ] Agent 可以建议兼容 Bundle。
* [ ] Agent 不能安装 Bundle。
* [ ] Agent 不能接受 License。
* [ ] 安装完成后可从原 Draft Retry。
* [ ] Agent 不会把未安装模型加入 Runnable Draft。

## J. 回归

* [ ] Geometry Safety 不回归。
* [ ] Workflow Static Validation 不回归。
* [ ] Artifact lineage 不回归。
* [ ] Cache 不回归。
* [ ] Replay 不回归。
* [ ] Batch 不回归。
* [ ] Review 不回归。
* [ ] Export 不回归。
* [ ] Provider Registry 不回归。
* [ ] TUI 和 GUI 不回归。

---

# 三十八、必须完成的端到端测试

## Case 1：Fixture Bundle

```text
Pack
→ Verify
→ Install
→ Bind Plugin
→ Smoke Test
→ Ready Model Profile
→ Workflow Run
→ Remove
```

## Case 2：损坏 Bundle

修改一个模型文件字节。

预期：

```text
InvalidChecksum
```

不得加载。

## Case 3：错误 Contract

Bundle 文件存在，但张量名称错误。

预期：

```text
InvalidContract
```

不得 Ready。

## Case 4：缺少一个 SAM 文件角色

只有：

```text
image_encoder
```

缺少：

```text
mask_decoder
```

预期：

```text
MissingFileRole(mask_decoder)
```

## Case 5：许可证未确认

预期无法下载或激活。

## Case 6：Smoke Test 失败

Model Bundle 安装完成，但推理结果不符合容差。

预期：

```text
FailedSmokeTest
```

不得进入 Model Selector。

## Case 7：版本并存

```text
Bundle v1 被 Published Workflow 使用
→ 安装 Bundle v2
→ v1 不变
→ v2 只供新 Draft 使用
```

## Case 8：删除保护

被 Published Workflow 引用的 Bundle 不能删除。

## Case 9：SAM 用户路径

```text
SAM Plugin installed
→ No compatible model
→ Install compatible bundle
→ Verify
→ Smoke Test
→ Ready
→ Pipeline Builder Retry
→ SAM chain becomes selectable
```

## Case 10：没有兼容 Bundle

预期：

```text
No verified bundle is available for this platform.
```

不得告诉用户自行下载两个同名文件。

## Case 11：下载中断

预期：

* staging 可清理；
  -无 Ready Model；
  -可重试；
  -旧模型不受影响。

## Case 12：无 Python

安装和推理进程树中不出现 Python。

---

# 三十九、最终测试命令

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

重点 crate：

```bash
cargo test -p annotagent-plugin-api
cargo test -p annotagent-plugin-host
cargo test -p annotagent-plugin-registry
cargo test -p annotagent-model-runtime-onnx
cargo test -p annotagent-model-bundle
cargo test -p annotagent-model-catalog
```

如果 crate 名称不同，使用仓库实际名称，不要为了匹配本文重新创建重复 crate。

Web：

```bash
npm run typecheck
npm run test
npm run build
```

E2E 至少覆盖：

1. 插件已安装但无模型；
2. 打开兼容模型列表；
3. 接受许可证；
4. 下载进度；
5. Bundle 验证；
6. Contract 错误；
7. Smoke Test；
8. Ready Model；
9. 返回原 Draft；
10. Pipeline 选择模型；
    11.刷新状态恢复；
    12.模型删除保护；
11. Legacy raw ONNX 迁移；
12. Generic Project；
13. RoboCup Project；
14. 390px 和 1024px 布局。

---

# 四十、文档

新增：

```text
docs/MODEL_BUNDLES.md
docs/ANNOTMODEL_FORMAT.md
docs/MODEL_CATALOG.md
docs/MODEL_PROVISIONING.md
docs/MODEL_BUNDLE_SECURITY.md
docs/MODEL_BUNDLE_CONTRACTS.md
docs/MODEL_BUNDLE_SMOKE_TESTS.md
docs/MODEL_ASSET_VERSIONING.md
docs/MODEL_BUNDLE_PUBLISHING.md
docs/SAM_MODEL_PROVISIONING.md
docs/LEGACY_RAW_ONNX_MIGRATION.md
docs/DEMO_MODEL_BUNDLE_ALPHA.md
```

更新：

```text
README.md
docs/DESIGN.md
docs/RUST_MODEL_PLUGINS.md
docs/RUST_PLUGIN_MANIFEST.md
docs/RUST_PLUGIN_SECURITY.md
docs/RUST_ONNX_RUNTIME.md
docs/VLM_GEOMETRY_SAFETY.md
docs/GUIDED_EXPERIENCE.md
docs/KNOWN_LIMITATIONS.md
docs/COURSE_REQUIREMENTS.md
```

---

# 四十一、明确不做

本次不做：

* 公共开放 Marketplace；
  -任意用户上传到官方 Catalog；
  -评分和评论；
  -付费模型；
  -自动模型训练；
  -自动模型转换；
  -从 `.pt` 自动生成 ONNX；
  -Python 导出流程；
  -Python Runtime；
  -Python Worker；
  -任意 Shell 安装脚本；
  -Agent 自动下载模型；
  -Agent 自动接受许可证；
  -自动信任未知发布者；
  -自动替换 Published Workflow 的模型；
  -云端多用户模型仓库；
  -完整模型 CDN；
  -模型 DRM；
  -前端插件代码注入。

---

# 四十二、不得采用的假修复

禁止：

* 只把两个 ONNX 上传框合并成一个 ZIP 上传框；
  -只改 UI 文案；
  -把文件名当作 Contract；
  -用户上传文件后不校验 Hash；
  -没有测试向量也显示 Ready；
  -插件安装成功就自动把模型标成 Ready；
  -将任何同名 ONNX 当成兼容；
  -自动从不明来源下载模型；
  -执行 Bundle 内脚本；
  -使用 Python 转换；
  -用 Fixture 模型冒充真实 SAM；
  -将 SAM 2 未验证 Bundle 宣传为 Supported；
  -更新 Bundle 时覆盖旧文件；
  -删除历史 Workflow 引用；
  -让 Agent 自动接受许可证；
  -让 Agent 自动下载权重；
  -破坏 Geometry Safety；
  -修改 Git remote；
  -push；
  -提交 API Key；
  -提交未经许可的模型权重。

---

# 四十三、最终报告格式

最终报告必须包含：

## 1. 原问题

说明：

* 为什么普通用户找不到两个 ONNX；
* 为什么文件名不足以表示兼容性；
* 为什么插件安装不等于模型安装。

## 2. Plugin 与 Model Bundle 边界

说明：

* Plugin 负责什么；
* Bundle 负责什么；
* Model Instance 负责什么；
* Model Profile 引用什么。

## 3. `.annotmodel`

说明：

* Manifest；
  -模型文件；
  -角色；
  -Contract；
  -Hash；
  -License；
  -Test Vector；
  -Signature。

## 4. Catalog 和安装

说明：

* Curated Catalog；
  -本地 Import；
  -下载安全；
  -内容寻址存储；
  -原子安装；
  -取消和恢复。

## 5. Contract 和 Smoke Test

说明：

-如何验证 ONNX；
-如何验证多文件模型；
-如何阻止错误模型进入 Ready；
-如何记录测试证据。

## 6. SAM 体验

说明：

-旧双 ONNX 上传如何迁移；
-当前 SAM 2 状态；
-首个真实 PromptedSegmentation Bundle；
-普通用户如何安装。

## 7. Workflow 可复现性

说明：

-固定 Plugin；
-固定 Bundle；
-固定文件 Digest；
-固定 Model Profile；
-更新与卸载保护。

## 8. Agent 集成

说明：

* Agent 如何发现 Ready 模型；
  -缺模型时如何提出 Setup Requirement；
  -为什么 Agent 不能安装或接受许可证。

## 9. 测试

列出实际执行命令和真实结果。

不得把未执行测试报告为通过。

## 10. Milestone 提交

列出：

```text
commit hash
commit message
milestone
```

## 11. Live-conditional

明确说明：

-真实 PromptedSegmentation 模型；
-SAM 2；
-外部 Catalog 托管；
-GPU；
-不同平台；
-真实模型许可证。

## 12. 未完成内容

明确区分：

```text
未实现
已实现但未真实模型验证
外部条件阻塞
明确不属于本轮
```

禁止使用：

```text
基本完成
理论上可用
应该兼容
大概率能运行
```

## 13. Git 状态

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
docs/execution/MODEL_BUNDLE_MASTER_PROMPT.md
```

然后从 AnnotAgent 仓库根目录启动 Codex，并输入：

```text
阅读 docs/execution/MODEL_BUNDLE_MASTER_PROMPT.md，并将其作为本次长程任务的最高目标。

先核验现有 Rust Plugin Host、Plugin Registry、Model Profile、ONNX Runtime、SAM 插件、GUI、数据库和测试，不要盲信文件存在即代表能力可用。

本次任务不要重写插件系统。重点是：

1. 将 Rust Plugin 与模型资产彻底分离；
2. 建立可安装的 .annotmodel Model Bundle；
3. 建立多文件角色、Contract、Hash、License 和 Smoke Test；
4. 建立 Curated Model Catalog 和本地 Bundle Import；
5. 通过 Rust 完成下载、验证、安装和推理；
6. 不要求普通用户自己寻找、转换或分别上传裸 ONNX；
7. 只有 Plugin、Bundle、Contract 和 Smoke Test 全部通过后才创建 Ready Model Profile；
8. Published Workflow 固定 Plugin 和 Model Bundle 的完整身份；
9. 保留旧版本和历史 Run；
10. 让当前 SAM 页面显示兼容模型安装，而不是两个裸文件上传框；
11. 至少交付一个真实可安装的 PromptedSegmentation Bundle；
12. SAM 2 在没有经过验证的 Bundle 前保持 Labs；
13. Agent 可以建议安装兼容模型，但不能安装、下载或接受许可证；
14. 保持 Geometry Safety、Workflow Runtime、Artifact、Replay、Review、Batch、Provider 和 Export 不回归。

从 Milestone 0 开始持续执行。

普通技术决策自行决定，并记录到 MODEL_BUNDLE_DECISIONS.md。

每完成一个 Milestone：
1. 更新状态和验收证据；
2. 执行对应 Rust、Bundle、Plugin、Web 和 E2E 测试；
3. 修复回归；
4. 创建独立本地提交；
5. 继续下一 Milestone。

如果 SAM 2 没有可验证的 Rust ONNX Bundle：
- 不退回 Python；
- 不使用来源不明 ONNX；
- 保持 SAM 2 为 Labs；
- 选择一个合法、可验证的轻量 PromptedSegmentation 模型作为首个正式 Bundle；
-精确记录 Live-conditional 项。

不要 push。
不要修改 Git remote。
不要使用、恢复或提交任何对话中出现过的 API Key。
不要提交未经许可的大型模型权重。
不要执行 reset、rebase、amend 或破坏性 checkout。

只有 Release Blocking Acceptance Matrix 全部满足，或只剩明确记录的平台和真实模型条件项时，才输出最终报告。
```

