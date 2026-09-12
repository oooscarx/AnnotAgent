# 本轮验证记录

日期：2026-09-12。仅文档与只读取证，不改应用、真实数据、凭证、依赖声明或部署配置。

## 实际执行

- 初始 cwd、分支、status、HEAD、worktree list 已核验：主工作区 eaeae4c，有未跟踪设计/产品素材，保持不动。
- 独立 worktree / 分支 `codex/product-readme`，从 eaeae4c 新建，没有 reset/rebase/amend。
- 本机 `rustc --version`：1.98.0；Node v25.9.0；npm 11.12.1。仓库 pin Rust 1.98.0，锁定依赖允许 Node ^20.19.0 或 >=22.12.0。
- `npm --prefix web ci --ignore-scripts --no-audit --no-fund`：通过。随后运行正常 `npm --prefix web ci --no-audit --no-fund`：通过。
- `npm --prefix web run build`：tokens 检查、TypeScript 和 Vite 构建通过；仅生成独立 worktree 的忽略构建目录。
- 8788 既有服务只读拍摄工作区：完整 1600×1000，保留只读/审核提示。没有模型调用、编辑保存、数据清理或重启用户服务。
- 工程师留存 TEST ZIP：19,527 bytes，SHA-256 `f3c915950cf679742161e2bf061a67e05938cf148c935966ee31d319185e3fa9`，28 条目，CRC 无错误。11 PNG、11 TXT、data.yaml、README、四份 AnnotAgent JSON 均实际存在。未复制 ZIP 或原图进 Git。
- README 本地近似预览：Chromium，浅/深主题，1280×900 与 390×900；无横向溢出、无图片加载失败。检查数据见 preview-checks.json；全页图与顶部检查图位于本目录。
- README 只依赖 Markdown、picture/source/img，不依赖自定义 CSS/JS。preview.html 中样式仅用于本地近似预览，不进入 README 渲染逻辑。

## 未执行 / 不能据此宣称

- 本分支 `cargo run`、完整 Rust 构建和最新集成源码启动未运行。本轮 Web 构建不等于 Rust/真实推理验证。
- 没有调用付费 Provider、下载模型权重、生成新正式训练数据、运行 Ultralytics 加载器或训练。
- 未在真实 GitHub 页面检查；本地 preview 不是 GitHub 实测。
- 未取得最终集成 SHA 对应的真实同任务审核保存和训练包完整证据。旧版截图与 TEST 文件均明确标注。
- 未确认 B-Human 截图用于公开再分发的完整权利范围；保留来源标识，不分发原始数据集。发布前由维护者确认。
- 远程问题反馈 URL 使用实际仓库地址；本轮未联网验证可访问性，未上传任何素材。

## 可复现素材检查

`python3 docs/product/readme/build_assets.py` 生成 SVG。
`node docs/product/readme/preview.mjs` 需要 Playwright 和 marked；marked 可由环境变量 ANNOTAGENT_DOCS_NODE_MODULES 指向现有工具依赖目录，无需修改产品 package.json。
`python3 docs/product/readme/check_assets.py` 检查本地链接、图片 hash、体积与许可正文是否存在。

最终本地链接/hash 检查通过，默认浅色三张图片合计 575,646 bytes（约 0.58 MB）。英文版同样通过深浅色、1280/390 px 本地布局检查。
