# Velo / 微落

> **Catch the stream, Keep the moment.**
>
> **轻取流光 留住此刻**

Velo / 微落是一款正在开发中的桌面视频保存工具。当前已支持粘贴视频页面地址、解析真实媒体信息、选择格式和目标保存位置。

## 当前阶段

- Tauri 2 桌面外壳
- React + TypeScript 界面
- Rust `YtDlpEngine` 与受限外部进程执行器
- URL 校验、加载、取消、错误与结果状态
- 通过 Rust 受限获取并显示真实媒体封面
- 原生保存位置选择与跨平台安全文件名规则
- 真实媒体下载、进度、速度、剩余时间与取消
- 仅视频格式自动选择最佳音轨，并通过 FFmpeg 合并封装
- Bun 依赖与脚本管理

开发环境与应用包使用固定资产和 SHA-256 管理 yt-dlp 与 FFmpeg sidecar。保存位置、真实下载和音视频合并只在桌面模式中启用；代表帧封面仍在后续阶段。

## 开发环境

- Bun 1.3.14
- Rust 1.85 或更高版本
- 当前平台所需的 Tauri 系统依赖

```bash
bun install
bun run engine:install
bun run dev:desktop
```

`engine:install` 会为当前平台下载项目固定的 yt-dlp 版本，校验 SHA-256 和版本后原子替换 `binaries/` 下的本地文件；该二进制不会提交到 Git。`dev:desktop` 会先重新校验，再启动 Tauri。

桌面开发和发布构建会自动运行 `engine:prepare-sidecar`，根据 Tauri 的目标三元组复用或下载 yt-dlp 与 FFmpeg，校验后写入 Git 忽略的 `src-tauri/binaries/`。macOS 与 Linux 使用固定版本的 FFmpeg 8.1.2 原生架构构建；第三方来源和许可提示随应用资源一起打包。构建应用包：

```bash
bun run build:desktop
```

需要使用其他位置时，可设置绝对路径 `VELO_YT_DLP_PATH` 与 `VELO_FFMPEG_PATH`。纯浏览器开发模式继续使用只读 fixture，不会启动媒体 sidecar。

## 验证

```bash
bun run test
bun run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

`.github/workflows/ci.yml` 会在非文档 push 与 pull request 上运行 Ubuntu 快速验证，包括前端测试与构建、Rust 测试和 Clippy，不生成安装包。

`.github/workflows/platform-release-check.yml` 只在手动触发、推送 `v*` 版本标签，或 `main` 上的 Tauri 配置、sidecar 脚本、依赖清单等打包关键文件变化时运行。它会在 Windows x64 和 Linux x64 原生运行器上构建 NSIS、DEB，检查安装包确实包含 yt-dlp 与 FFmpeg sidecar，并保留 7 天产物用于人工抽查。三个工作流均复用 Cargo 缓存；真实站点兼容性测试仍保持手动触发。

真实站点测试默认忽略，仅对自己有权测试的公开地址显式运行：

```bash
VELO_INTEGRATION_TEST_URL=https://example.com/video \
VELO_YT_DLP_PATH=/absolute/path/to/verified/yt-dlp \
bun run test:integration
```

三平台真实站点验证通过手动工作流执行，地址仅从仓库 Secret `VELO_INTEGRATION_TEST_URL` 读取；测试范围和结果记录见 [`docs/site-compatibility.md`](docs/site-compatibility.md)。

## 项目结构

- `src/`：React 界面与前端用例
- `src-tauri/src/domain/`：稳定的领域模型
- `src-tauri/src/application/`：应用状态与引擎接口
- `src-tauri/src/infrastructure/`：受限进程执行器与媒体引擎适配器
- `src-tauri/src/commands/`：Tauri 命令边界
- `docs/`：产品和架构决策

## 项目文档

- [`PLAN.md`](PLAN.md)：开发阶段、任务状态与验收条件
- [`CHANGELOG.md`](CHANGELOG.md)：版本变化与当前限制
- [`docs/product.md`](docs/product.md)：当前阶段的产品范围
- [`docs/architecture.md`](docs/architecture.md)：模块职责与安全边界
- [`docs/site-compatibility.md`](docs/site-compatibility.md)：真实站点测试范围、执行方式与兼容性矩阵
