# Velo / 微落

> **Catch the stream, Keep the moment.**
>
> **轻取流光 留住此刻**

Velo / 微落是一款正在开发中的桌面视频保存工具。当前聚焦于一条清晰的产品链路：粘贴视频页面地址、解析真实媒体信息，并展示可用格式。

## 当前阶段

- Tauri 2 桌面外壳
- React + TypeScript 界面
- Rust `YtDlpEngine` 与受限外部进程执行器
- URL 校验、加载、取消、错误与结果状态
- Bun 依赖与脚本管理

开发环境与应用包已支持固定版本的 yt-dlp；macOS、Windows 与 Linux 的原生应用包均已验证包含对应 sidecar。真实下载与 FFmpeg 文件处理仍在后续阶段。

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

桌面开发和发布构建会自动运行 `engine:prepare-sidecar`，根据 Tauri 的目标三元组复用本地引擎或下载对应官方资产，校验后写入 Git 忽略的 `src-tauri/binaries/`。构建应用包：

```bash
bun run build:desktop
```

需要使用其他位置时，可设置绝对路径 `VELO_YT_DLP_PATH`；该文件仍必须与项目固定版本和校验和一致。纯浏览器开发模式继续使用只读 fixture，不会启动 yt-dlp。

## 验证

```bash
bun run test
bun run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

`.github/workflows/platform-release-check.yml` 会在 Windows x64 和 Linux x64 原生运行器上重复测试与 Clippy，分别构建 NSIS、DEB，并检查安装包确实包含 yt-dlp sidecar。工作流产物保留 7 天用于人工抽查。

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
