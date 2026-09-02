# AnnotAgent Real Prompted-Segmentation Model Delivery

你现在是 AnnotAgent 仓库的长期主实现工程师。

本次任务只解决一个阻塞真实用户的问题：

> AnnotAgent 已经能够安装 Rust 专家模型插件和 Fixture Model Bundle，但 Catalog 中没有普通用户可以直接安装、并完成真实推理的提示分割模型。用户点击 `Install compatible model` 后只能获得 Fixture，或者被要求自行寻找、转换并上传不存在的 ONNX 文件。

本次任务的最终验收不是：

* `.annotmodel` 格式存在；
* Fixture Smoke Test 通过；
* SAM 插件卡片显示正常；
* GUI 有安装按钮；
* 文档描述了未来路线。

本次最终验收只有一个核心事实：

```text
在一台没有预装模型权重、没有 Python 环境的当前目标机器上：

用户打开 AnnotAgent
→ Settings
→ Expert Model Plugins
→ Install compatible model
→ 选择一个真实提示分割模型
→ 确认来源和许可证
→ 自动下载或导入经过验证的 .annotmodel
→ 自动验证文件、Contract 和模型身份
→ 自动运行真实 Smoke Test
→ Model Instance 变为 Ready
→ Pipeline Builder 可以选择该模型
→ 使用一张真实图片和一个 bbox prompt 执行真实推理
→ 产生非空 Mask Artifact
→ 通过 Mask-to-BBox 生成精修框
→ Run Debug 可以查看完整 Artifact lineage
```

Fixture-only 模型不能满足上述验收。

---

# 一、任务名称

本次长期任务名称：

```text
AnnotAgent Real Prompted-Segmentation Delivery Alpha
```

目标能力：

```text
PromptedSegmentation
```

本轮不强制要求第一个可用模型一定叫 SAM 2。

第一个正式模型应根据以下标准选择：

1. 能在当前目标平台由 Rust 插件真实执行；
2. 不需要用户安装 Python；
3. 有合法、可核验的模型来源；
4. 有明确代码和权重许可证；
5. 有可获得的 ONNX 或其他现有 Rust Runtime 可执行格式；
6. 支持 box prompt，最好同时支持 point prompt；
7. 可以输出真实 mask；
8. 可以制作可复现的 `.annotmodel`；
9. 可以在当前机器完成真实 Smoke Test；
10. 模型体积和性能适合 Developer Preview。

候选可以包括但不限于：

```text
EfficientSAM
MobileSAM
SAM 1 compatible ONNX
其他合法的轻量 Prompted Segmentation 模型
```

必须通过官方来源审计后再选择。

如果 SAM 2 当前没有可验证的 Rust 推理资产，则：

```text
SAM 2 保持 Labs
```

不得为了保留品牌名，继续让用户面对不存在的两个 ONNX 文件。

---

# 二、硬性约束

## 2.1 用户运行路径 Rust-only

普通用户的以下流程不得依赖 Python：

```text
安装插件
安装模型
验证模型
运行 Smoke Test
正式推理
Pipeline Run
Replay
```

禁止活动路径调用：

```text
python
python3
pip
uv
conda
venv
FastAPI
PyTorch Python
Transformers Python
```

禁止：

```rust
Command::new("python")
Command::new("python3")
Command::new("pip")
Command::new("uv")
```

允许 Rust 插件通过 Rust binding 使用：

```text
ONNX Runtime
CoreML
Metal
CUDA
TensorRT
其他已有的原生推理 Runtime
```

## 2.2 模型 Bundle 构建边界

普通用户不得负责转换模型。

优先选择上游已经发布、或可信发布者已经生成的兼容模型资产。

若维护者需要构建 Bundle：

* 可以在独立供应链环境生成模型产物；
* 最终用户只接收经过验证的 `.annotmodel`；
* 用户机器不得在安装时执行任意转换代码；
* AnnotAgent 仓库不得把不受控转换脚本作为用户安装步骤；
* 构建来源、工具版本和导出过程必须进入 Bundle provenance。

如果用户坚持整个供应链也完全不使用 Python，则只允许选择：

* 上游直接发布的兼容 ONNX；
* 已有可信 ONNX 资产；
* Rust 可以直接构造或导出的模型。

不能假装任意 `.pt` 能由通用 Rust 命令自动转成正确 ONNX。

## 2.3 不提交大型权重

不要把大型模型权重提交到 Git 仓库。

权重必须通过：

```text
Curated Catalog 下载
```

或：

```text
本地 .annotmodel 导入
```

获得。

所有模型文件必须有固定 SHA-256。

## 2.4 不伪造完成状态

必须区分：

```text
Plugin implemented
Plugin installed
Model Bundle available
Model Bundle installed
Contract verified
Smoke Test passed
Model Instance Ready
Real inference passed
```

只有最后两项都满足，才能把该模型标记为 Supported。

---

# 三、开始前核验现状

首先执行：

```bash
git status --short --branch
git log --oneline -20
uname -a
uname -m
```

在 macOS 上同时执行：

```bash
sw_vers
```

然后执行当前基线：

```bash
cargo fmt --all --check
cargo test --workspace --all-features
cargo build --workspace --all-features
```

检查：

```text
README.md
docs/
docs/execution/

crates/annotagent-plugin-api/
crates/annotagent-plugin-sdk/
crates/annotagent-plugin-host/
crates/annotagent-plugin-registry/

crates/annotagent-model-runtime-common/
crates/annotagent-model-runtime-onnx/
crates/annotagent-model-bundle/
crates/annotagent-model-catalog/

plugins/sam-onnx/
plugins/
web/
apps/annotagent/
workspace/
dist/
```

核验以下真实状态：

1. 当前安装的 Plugin 版本；
2. 当前 Fixture Bundle；
3. 当前 `.annotmodel` Manifest；
4. 当前 Model Catalog；
5. 当前 Plugin required file roles；
6. 当前 SAM Plugin 实际张量 Contract；
7. 当前 Rust ONNX Runtime；
8. 当前 Model Instance 状态机；
9. 当前 Smoke Test；
10. 当前 Geometry Safety；
11. 当前 Pipeline Builder 模型选择；
12. 当前 GUI 的 `Install compatible model`；
13. 当前裸 ONNX 上传入口；
14. 当前 Published Workflow 如何固定模型资产；
15. 当前 macOS ARM64 支持状态。

必须复现：

```text
SAM Plugin 已安装
→ 点击 Install compatible model
→ Catalog 只有 Fixture 或没有真实 Bundle
→ 用户无法完成真实推理
```

把复现写成自动测试或至少可重复的 E2E Fixture。

禁止：

```text
git reset
git rebase
git commit --amend
破坏性 checkout
修改 Git remote
push
使用任何对话中出现过的 API Key
用 Fixture 冒充真实模型
```

---

# 四、长期执行账本

创建并持续维护：

```text
docs/execution/REAL_MODEL_DELIVERY_PLAN.md
docs/execution/REAL_MODEL_DELIVERY_STATUS.md
docs/execution/REAL_MODEL_DELIVERY_DECISIONS.md
docs/execution/REAL_MODEL_DELIVERY_ACCEPTANCE.md
docs/execution/REAL_MODEL_DELIVERY_BLOCKERS.md
docs/execution/REAL_MODEL_DELIVERY_MODEL_AUDIT.md
```

`REAL_MODEL_DELIVERY_STATUS.md` 必须记录：

```text
当前 Milestone
当前候选模型
候选模型淘汰原因
已完成
正在进行
下一步
最近 Rust 测试
最近 Bundle 验证
最近真实推理
最近 Web E2E
最近 Commit
Release Blocking 剩余项
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

# 五、先做模型候选审计

不要先假设现有 `sam-onnx` 插件一定能运行选中的模型。

对至少三个候选进行官方来源审计。

每个候选记录：

```text
Model family
Exact model variant
Official repository
Official model source
Model file format
Whether compatible ONNX is directly available
Whether multiple model files are required
Code license
Weight license
Redistribution status
Commercial-use notes
Model size
Input contract
Output contract
Box prompt support
Point prompt support
Expected preprocessing
Current Rust runtime compatibility
Current plugin compatibility
macOS ARM64 CPU feasibility
Linux x86_64 feasibility
Reason accepted or rejected
```

必须优先查阅：

* 官方仓库；
* 官方模型卡；
* 官方 LICENSE；
* 官方 Release；
* 官方导出文档。

不得仅依据第三方博客、随机 Hugging Face 仓库或同名网盘文件。

## 5.1 候选选择规则

第一个正式 Bundle 必须满足：

```text
Official or audited source
+
license acceptable
+
Rust runtime executable
+
box prompt supported
+
real mask output
+
current target platform runnable
```

如果现有 `sam-onnx` 插件 Contract 不兼容选中的候选：

### 允许

创建新的模型家族插件，例如：

```text
org.annotagent.efficientsam-onnx
```

或：

```text
org.annotagent.prompted-segmentation-onnx
```

前提是它继续暴露统一 Capability：

```text
PromptedSegmentation
```

### 不允许

在一个插件中加入大量：

```rust
if model_family == ...
```

把互不兼容的模型强行揉成一个后处理器。

## 5.2 不要为品牌牺牲可用性

如果 SAM 2 没有符合要求的 Rust-ready 模型资产：

```text
SAM 2:
Labs
No verified Rust model bundle available
```

同时交付另一个真实可用的 PromptedSegmentation 模型。

用户需要的是“精修 bbox”，不是 Logo 上必须出现最新型号。

---

# 六、增加 Model Supply Recipe

为了让维护者可以重复构建真实 `.annotmodel`，增加受控 Recipe：

```text
model-recipes/
└── <model-id>/
    ├── recipe.toml
    ├── contracts/
    ├── tests/
    ├── licenses/
    └── README.md
```

示例：

```toml
schema_version = "1"

id = "org.annotagent.models.prompted-segmentation-small"
version = "1.0.0"

[upstream]
project = "..."
model = "..."
version = "..."
license_url = "..."
license_sha256 = "..."

[[assets]]
role = "image_encoder"
url = "https://official.example/model_encoder.onnx"
sha256 = "..."
size_bytes = 123

[[assets]]
role = "mask_decoder"
url = "https://official.example/model_decoder.onnx"
sha256 = "..."
size_bytes = 456

[output]
bundle_id = "..."
compatible_plugin = "..."
```

Recipe 只能：

```text
下载固定 HTTPS 文件
校验大小
校验 SHA-256
复制 Contract 和测试向量
打包 .annotmodel
```

Recipe 不得：

```text
执行 Shell
执行 Python
Git clone
运行不受控转换脚本
执行下载资产内的程序
```

实现 Rust 命令：

```bash
annotagent models recipe audit <recipe>
annotagent models recipe fetch <recipe>
annotagent models recipe build <recipe> --output <file.annotmodel>
annotagent models recipe verify <recipe>
```

如果选中模型必须经过复杂转换，而没有直接可下载、可核验资产，则：

* 不能使用这种 Runtime Recipe；
* 由维护者在独立供应链环境预先生成 Bundle；
* Bundle provenance 必须记录转换来源；
* 用户安装仍然只接触最终 `.annotmodel`。

---

# 七、生成一个真实本地 Catalog

本次不要求 push，但必须让当前开发环境真的能使用 GUI 安装。

生成：

```text
dist/model-catalog/
├── catalog.json
├── bundles/
│   └── <real-model>.annotmodel
└── verification/
```

支持受控本地开发 Catalog：

```text
source kind: trusted_local_catalog
```

它只能读取 AnnotAgent 明确配置的 Catalog 根目录，不能读取任意系统路径。

增加命令：

```bash
annotagent models catalog add-local ./dist/model-catalog
annotagent models catalog list
annotagent models catalog refresh
```

当前 GUI 连接该本地 Catalog 后：

```text
Install compatible model
```

必须显示真实模型，而不仅是 Fixture。

同时准备远程发布所需的：

```text
catalog.json
Bundle SHA-256
Release asset filenames
GitHub Release workflow or release instructions
```

不要 push。

---

# 八、Model Bundle 必须是正式真实资产

真实 `.annotmodel` 必须包含：

```text
annotagent-model.toml
真实模型文件
Artifact Contracts
Preprocessing configuration
Postprocessing configuration
真实测试图片
真实 box prompt
Expected output summary
Tolerance
License
Source notice
File checksums
Bundle digest
```

必须明确：

```text
fixture_only = false
production_eligible = true
```

Fixture Bundle 必须继续标记：

```text
fixture_only = true
production_eligible = false
```

Static Validator 和 Model Selector 必须阻止 Fixture 进入正式 Workflow。

---

# 九、Contract 验证必须以模型图为准

安装时通过 Rust ONNX Runtime读取真实模型：

* input names；
* output names；
* tensor dtype；
* tensor shape；
* dynamic dimensions；
* opset；
* external data references。

不得只相信 Manifest 声明。

对于多文件提示分割模型，必须检查文件之间的连接：

```text
Encoder output embedding
→ Decoder input embedding
```

至少验证：

-名称或声明的 alias；
-dtype；
-shape；
-通道数；
-空间尺寸；
-提示坐标格式；
-原图尺寸输入；
-mask 输出尺寸；
-score 输出。

如果模型的实际接口与 Plugin 不符：

```text
ContractMismatch
```

不得通过重命名文件蒙混过去。

---

# 十、真实 Smoke Test

本轮必须执行真实推理，不是只加载 ONNX Session。

Smoke Test 输入：

```text
一张真实、可自由分发的测试图片
+
一个明确包含前景目标的 bbox prompt
```

测试链：

```text
Image
→ Plugin preprocessing
→ Real encoder inference
→ Real decoder inference
→ MaskSetArtifact
→ Mask validation
→ Mask-to-BBox
```

必须验证：

1. 模型文件真实加载；
2. 推理没有使用 Mock；
3. 输出 tensor 非空；
4. 输出不存在 NaN/Infinity；
5. 至少产生一个有效 mask；
6. mask 面积大于合理下限；
7. mask 不覆盖整张图；
8. mask 与 box prompt 有合理交集；
9. mask-to-bbox 坐标合法；
10. Artifact lineage 完整；
11. Plugin 进程未崩溃；
12. 当前机器上推理真实完成。

Smoke Test Report 至少保存：

```text
Plugin ID/version
Bundle ID/version
Bundle digest
File digests
Execution provider
Operating system
Architecture
Inference duration
Input image digest
Prompt
Mask summary
Refined bbox
Pass/fail
```

---

# 十一、真实 Workflow E2E

Smoke Test 通过后，必须在 AnnotAgent 中执行一条真实 Pipeline：

```text
Image Input
→ 一个真实或现有的 coarse bbox
→ Detections to Box Prompts
→ Prompted Segmentation Model
→ MaskSet
→ Mask-to-BBox
→ Geometry Evaluation
→ Geometry Decision
→ Human Review 或 Commit
```

至少使用一张真实足球图片或可自由分发的真实目标图片。

验收：

* Run 状态完成或进入 Review；
* Debug 页面可查看原框；
  -可查看 box prompt；
  -可查看真实 mask；
  -可查看 refined bbox；
  -可查看几何变化；
  -来源模型显示真实 Model Instance；
  -不得显示 Fixture；
  -不得显示 Mock；
  -重启 AnnotAgent 后仍可查看和 Replay。

---

# 十二、当前 SAM 插件的产品迁移

当前插件卡片不能继续给普通用户显示：

```text
Upload image_encoder.onnx
Upload mask_decoder.onnx
```

作为主要操作。

改为：

```text
SAM-compatible Prompted Segmentation

Plugin runtime:
Installed

Compatible models:
- 当前真实兼容 Bundle，若存在
- SAM 2 Labs，若尚不可用

[Install compatible model]
```

如果当前插件只兼容 SAM 1 Contract，名称和说明必须准确。

不能把：

```text
SAM 1 ViT-B compatible runtime
```

继续模糊显示成：

```text
SAM 2
```

若当前用户已有：

```text
sam2.1_hiera_tiny.pt
```

GUI 必须明确说明：

```text
This checkpoint is not compatible with the installed Rust ONNX plugin.
A verified model bundle is required.
```

不得提供一个看似可以上传、实际永远失败的入口。

---

# 十三、安装体验

普通用户路径：

```text
Settings
→ Expert Model Plugins
→ Prompted Segmentation
→ Install compatible model
```

安装面板显示：

```text
Model name
Model family
Capability
Plugin compatibility
Platform compatibility
Execution provider
Download size
Installed size
Source
License
Bundle digest
Real/Fixture status
```

用户点击：

```text
Install model
```

后显示真实阶段：

```text
Resolving model
Downloading model bundle
Verifying bundle digest
Verifying model files
Checking ONNX contract
Starting Rust plugin
Loading model
Running real sample inference
Registering model profile
Ready
```

失败时显示具体阶段和修复动作。

禁止只显示：

```text
Installation failed
```

---

# 十四、Pipeline Builder 集成

模型安装前：

```text
Prompted segmentation is unavailable.

Current safe automation:
VLM coarse detection
→ Mandatory human review

Optional improvement:
Install a compatible prompted-segmentation model.
```

提供：

```text
Install compatible model
```

安装完成后：

```text
Retry from saved draft
```

Pipeline Builder 重新读取 Registry，并可以生成：

```text
Detection
→ Box Prompt
→ Prompted Segmentation
→ Mask to BBox
→ Geometry Evaluation
→ Decision
```

Agent 不能：

* 自动下载安装；
* 自动确认许可证；
* 自动选择 Fixture；
* 自动将 Labs 模型当成 Ready；
* 自动发布新 Workflow。

---

# 十五、Geometry Safety 继续生效

真实模型安装完成不等于结果自动安全。

Model Profile 应声明：

```text
Geometry semantics: RefinedGeometry
Calibration: Uncalibrated
```

因此仍然需要：

```text
Geometry Evaluation
→ Decision
→ Commit / Review
```

不能改成：

```text
Real model installed
→ Trust all masks
```

必须保留：

* coarse bbox；
  -mask；
  -refined bbox；
  -面积变化；
  -中心偏移；
  -mask quality；
  -最终 Decision。

---

# 十六、跨平台策略

第一目标平台是当前实际开发机器。

Codex 必须先检测，不要假设。

如果当前是：

```text
macOS ARM64
```

第一个真实 Model Bundle 必须在：

```text
macOS ARM64 + CPU/CoreML/Metal or available Rust runtime
```

完成真实推理。

不要只制作：

```text
linux-x86_64-cuda
```

然后告诉当前 Mac 用户产品理论上已经解决。

第二目标平台可以是：

```text
linux-x86_64
```

如果同一个 Bundle 可跨平台使用，记录平台独立。

如果执行 Provider 不同，Model Instance 必须分别记录。

---

# 十七、失败时的候选切换规则

不要在第一个模型候选失败后停止任务。

按照以下循环审计最多三个候选：

```text
候选 A
→ 来源或许可证不合格
→ 记录淘汰原因

候选 B
→ Rust Runtime 或 Contract 不兼容
→ 记录淘汰原因

候选 C
→ 可安装并真实推理
→ 选为首个正式 Bundle
```

只有当所有已审计候选均因真实原因不可行时，才能记录 Blocker。

不得因为“最想要的 SAM 2 不可行”而放弃 PromptedSegmentation Capability 的交付。

---

# 十八、供应链和安全要求

必须覆盖：

* Archive path traversal；
  -Zip Slip；
  -symlink；
  -hard link；
  -绝对路径；
  -大小写路径冲突；
  -重复文件；
  -解压炸弹；
  -超大文件；
  -Checksum 错误；
  -Manifest 与真实文件不一致；
  -Contract 欺骗；
  -测试向量篡改；
  -许可证摘要变化；
  -下载重定向；
  -私网和 loopback 下载；
  -下载中断；
  -安装中断；
  -并发安装；
  -磁盘不足；
  -模型加载崩溃。

本地开发 Catalog 可以读取受控本地目录，但普通远程 Catalog 仍必须使用固定 HTTPS URL 和 digest。

---

# 十九、需要新增或完善的 CLI

用户命令：

```bash
annotagent models catalog list
annotagent models search prompted-segmentation
annotagent models show <bundle-id>
annotagent models install <bundle-id>@<version>
annotagent models import <file.annotmodel>
annotagent models list
annotagent models test <model-instance-id>
annotagent models doctor <model-instance-id>
annotagent models references <bundle-id>@<version>
```

维护者命令：

```bash
annotagent models recipe audit <recipe>
annotagent models recipe fetch <recipe>
annotagent models recipe build <recipe> --output <bundle>
annotagent models bundle inspect <bundle>
annotagent models bundle verify <bundle>
annotagent models catalog build <directory>
```

提供一条当前仓库可运行的真实安装命令，例如：

```bash
annotagent models install \
  org.annotagent.models.<real-model>@1.0.0
```

该命令结束后必须产生：

```text
Model instance: Ready
Smoke test: Passed
Fixture only: No
```

---

# 二十、数据库和版本固定

现有 Published Workflow 必须固定：

```text
Plugin ID
Plugin Version
Plugin Digest
Model Bundle ID
Model Bundle Version
Model Bundle Digest
Model File Digests
Model Instance ID
Model Profile Revision
Contract Hash
Execution Provider
```

删除模型前检查：

* Published Workflow；
  -Draft；
  -Project Binding；
  -Active Run；
  -Replay；
  -Calibration；
  -历史 Run。

已有引用时阻止删除。

---

# 二十一、Milestones

## Milestone 0：真实缺口回归

完成：

* 复现 Fixture-only 安装；
* 复现当前用户无法获得真实模型；
* 记录当前 Plugin Contract；
* 记录当前平台；
* 建立状态文档；
* 建立 Release Matrix。

提交：

```text
test(models): reproduce missing real prompted-segmentation model
```

## Milestone 1：候选模型审计

完成：

* 至少三个候选；
* 官方来源；
  -许可证；
  -ONNX 可用性；
  -Rust Runtime；
  -平台；
  -选择或淘汰理由。

提交：

```text
docs(models): select a deliverable prompted-segmentation model
```

## Milestone 2：受控模型 Recipe 和本地 Catalog

完成：

* Model Recipe；
  -Rust fetch；
  -Checksum；
  -Bundle build；
  -本地 Catalog；
  -真实 Bundle 产物；
  -测试。

提交：

```text
feat(models): build verified bundles from audited model assets
```

## Milestone 3：真实 Contract 和 Smoke Test

完成：

* 真实模型图检查；
  -Plugin compatibility；
  -真实模型加载；
  -真实 box-prompt inference；
  -Mask；
  -Mask-to-BBox；
  -Smoke Test Report。

提交：

```text
feat(models): validate a real prompted-segmentation model instance
```

## Milestone 4：一键安装 GUI

完成：

* Catalog model 展示；
  -来源和许可证；
  -一键安装；
  -进度；
  -失败修复；
  -Ready 状态；
  -移除默认裸 ONNX 上传；
  -E2E。

提交：

```text
feat(ui): install real prompted-segmentation models without raw files
```

## Milestone 5：Pipeline 和 Agent 闭环

完成：

* Model Profile 选择；
  -Pipeline Builder Retry；
  -真实 PromptedSegmentation 节点；
  -Geometry Safety；
  -Run Debug；
  -Review；
  -Replay；
  -重启恢复。

提交：

```text
feat(workflow): use installed real segmentation models in geometry refinement
```

## Milestone 6：跨平台、发布产物和回归

完成：

* 当前平台 Release Bundle；
* Catalog；
  -GitHub Release asset list；
  -安装说明；
  -全量测试；
  -文档；
  -验收证据。

提交：

```text
test(release): validate real model delivery and one-click installation
```

---

# 二十二、Release Blocking Acceptance Matrix

以下全部满足后，才能声称本任务完成。

## A. 真实模型

* [ ] 至少一个非 Fixture PromptedSegmentation 模型。
* [ ] 模型来源来自官方或经过完整审计的发布者。
* [ ] 权重许可证明确。
* [ ] Bundle 可合法安装。
* [ ] Rust Runtime 可以真实加载。
* [ ] 真实 box prompt 推理通过。
* [ ] 真实 MaskSetArtifact 非空。
* [ ] Mask-to-BBox 通过。
* [ ] 当前目标平台真实运行通过。
* [ ] 报告包含所有文件 SHA-256。

## B. 用户体验

* [ ] 用户不需要自己搜索 ONNX。
* [ ] 用户不需要运行转换脚本。
* [ ] 用户不需要安装 Python。
* [ ] 用户不需要分别上传 encoder 和 decoder。
* [ ] `Install compatible model` 显示真实模型。
* [ ] Fixture 与真实模型明显区分。
* [ ] 安装完成后状态为 Ready。
* [ ] 安装失败有具体原因。
* [ ] 重启后 Model Instance 仍为 Ready。

## C. Pipeline

* [ ] Pipeline Builder 可以选择真实模型。
* [ ] 真实模型可用于 PromptedSegmentation 节点。
* [ ] Artifact lineage 包含真实 Model Instance。
* [ ] Geometry Safety 保持。
* [ ] Published Workflow 固定 Bundle 身份。
* [ ] Replay 可以重新使用该模型。
* [ ] 不产生重复 Commit。

## D. SAM 状态

* [ ] 当前 SAM 插件名称和 Contract 描述准确。
* [ ] 不再告诉用户寻找不存在的两个 ONNX。
* [ ] SAM 2 若无真实 Bundle，保持 Labs。
* [ ] `.pt` 与当前插件不兼容时明确提示。
* [ ] 产品仍提供一个真实可用的 PromptedSegmentation 方案。

## E. Rust-only

* [ ] 用户安装路径不启动 Python。
* [ ] 用户推理路径不启动 Python。
* [ ] Smoke Test 不启动 Python。
* [ ] Rust ONNX 或其他 Rust 原生 Runtime真实执行。
* [ ] 活动进程树无 Python Worker。
* [ ] 不用 Fixture 代替真实模型。

## F. 回归

* [ ] Plugin Host 不回归。
* [ ] Model Bundle 验证不回归。
* [ ] Provider Registry 不回归。
* [ ] Workflow Static Validator 不回归。
* [ ] Geometry Safety 不回归。
* [ ] Batch 不回归。
* [ ] Pause/Resume/Cancel 不回归。
* [ ] Review 不回归。
* [ ] Replay 不回归。
* [ ] Export 不回归。

---

# 二十三、必须完成的 E2E

## E2E 1：从空环境安装

```text
Fresh workspace
→ Plugin installed
→ No model
→ Install compatible model
→ Accept license
→ Download/import
→ Verify
→ Smoke Test
→ Ready
```

## E2E 2：真实推理

```text
Real image
→ bbox prompt
→ real model inference
→ mask
→ refined bbox
→ geometry report
```

## E2E 3：Pipeline Builder

```text
Draft initially uses mandatory review
→ install model
→ Retry from saved draft
→ add prompted segmentation path
→ validate
→ dry run
→ human approval
```

## E2E 4：重启

```text
Install model
→ restart AnnotAgent
→ model remains Ready
→ workflow still resolves exact bundle
```

## E2E 5：版本与删除保护

```text
Publish Workflow
→ model bundle referenced
→ attempt delete
→ rejected with reference list
```

## E2E 6：无 Python

检查插件进程和子进程，不得出现 Python。

---

# 二十四、最终测试

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
npm run typecheck
npm run test
npm run build
```

重点真实命令：

```bash
annotagent models catalog list
annotagent models search prompted-segmentation
annotagent models install <real-bundle-id>@<version>
annotagent models test <real-model-instance-id>
annotagent models doctor <real-model-instance-id>
```

最终报告必须附上真实输出摘要。

---

# 二十五、不得采用的假修复

禁止：

* 只增加文档；
* 只增加一个新的 Fixture；
* 把 Fixture 改名为 Real；
* 继续要求用户分别上传两个 ONNX；
* 只把两个 ONNX 放进 ZIP，却没有来源、Contract 和 Smoke Test；
* 使用随机第三方 ONNX；
* 为了叫 SAM 2 而选择不可执行模型；
* 安装成功但真实推理失败仍显示 Ready；
* 只加载 Session，不执行真实 mask 推理；
* 只验证输出 shape，不验证有效 mask；
* 在用户安装时执行 Python 转换；
* 用 `.pt` 文件冒充兼容 ONNX；
* 自动接受许可证；
* Agent 自动下载模型；
* 让 Agent 选择 Fixture；
* 修改历史 Published Workflow；
* push；
  -修改 remote；
  -提交未经许可的大型权重；
  -提交 API Key。

---

# 二十六、最终报告格式

最终报告必须包含：

## 1. 原问题

说明为什么当前用户不能安装真实模型。

## 2. 候选模型审计

列出所有审计候选及淘汰理由。

## 3. 最终模型选择

说明：

* 模型；
  -来源；
  -许可证；
  -格式；
  -大小；
  -平台；
  -为什么可由 Rust 运行。

## 4. Bundle

列出：

* Bundle ID；
  -Version；
  -Bundle digest；
  -所有模型文件和 SHA-256；
  -Plugin compatibility；
  -Contract；
  -Smoke Test。

## 5. 用户安装路径

给出从 GUI 和 CLI 安装的准确步骤。

## 6. 真实推理证据

说明：

-真实输入；
-bbox prompt；
-mask；
-refined bbox；
-duration；
-execution provider；
-非 Mock 证明。

## 7. Pipeline 集成

说明 Pipeline Builder、Geometry Safety、Run、Review 和 Replay。

## 8. SAM 状态

准确说明：

-当前 SAM 1/SAM 2 插件分别是什么；
-哪些 Supported；
-哪些 Labs；
-为什么。

## 9. 测试结果

列出真实命令和结果。

不得将未执行测试写成通过。

## 10. Milestone 提交

列出：

```text
commit hash
commit message
milestone
```

## 11. 未完成项

明确区分：

```text
未实现
平台限制
许可证限制
外部托管限制
不属于本轮
```

不得使用：

```text
基本完成
理论上支持
应该能运行
大概率兼容
```

## 12. Git 状态

说明：

-当前分支；
-工作区是否干净；
-领先远程提交数；
-未 push；
-remote 未修改。

---

# 二十七、启动指令

将本文保存为：

```text
docs/execution/REAL_MODEL_DELIVERY_MASTER_PROMPT.md
```

然后从仓库根目录启动 Codex，输入：

```text
阅读 docs/execution/REAL_MODEL_DELIVERY_MASTER_PROMPT.md，并将其作为本次长期任务的最高目标。

先核验现有 Rust Plugin、Model Bundle、Fixture、SAM Contract、ONNX Runtime、Catalog、GUI 和当前平台，不要盲信已有完成说明。

本次任务只在以下事实成立后才算完成：

普通用户无需寻找 ONNX、无需转换模型、无需安装 Python，
即可通过 Install compatible model 安装至少一个真实的
PromptedSegmentation Model Bundle，并在当前目标平台完成真实
box-prompt → mask → mask-to-bbox 推理。

不要把 SAM 2 设为唯一候选。
审计至少三个官方候选。
优先交付一个真正可运行的小型模型。
SAM 2 没有可验证 Rust Bundle 时继续保持 Labs。

不得用 Fixture 冒充真实模型。
不得只实现 Bundle 基础设施。
不得只修改安装页面。
不得继续要求用户上传裸 encoder/decoder。
不得让用户运行 Python 转换。
不得修改历史 Published Workflow。

从 Milestone 0 开始持续执行。
每完成一个 Milestone：
1. 更新状态和验收证据；
2. 运行对应 Rust、Bundle、Plugin、Web 和 E2E 测试；
3. 修复回归；
4. 创建独立本地提交；
5. 继续下一 Milestone。

普通技术决策自行完成，并记录到
REAL_MODEL_DELIVERY_DECISIONS.md。

如果第一个候选不可行，记录淘汰理由并继续审计下一个候选，
不要因为 SAM 2 不可行就停止整个任务。

不要 push。
不要修改 Git remote。
不要使用或恢复任何对话中出现过的 API Key。
不要提交未经许可的大型权重。
不要执行 reset、rebase、amend 或破坏性 checkout。

只有 Release Blocking Acceptance Matrix 全部满足，
或者存在无法绕过且有官方证据支持的法律或平台阻塞时，
才输出最终报告。
```
