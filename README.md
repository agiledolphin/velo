# Velo / 微落

> **Catch the stream, Keep the moment.**
>
> **轻取流光 留住此刻**

Velo / 微落是一款正在开发中的桌面视频保存工具。第一阶段聚焦于一条清晰的产品链路：粘贴视频页面地址、解析媒体信息，并展示可用格式。

## 当前阶段

- Tauri 2 桌面外壳
- React + TypeScript 界面
- Rust Mock 媒体解析引擎
- URL 校验、加载、错误与结果状态
- Bun 依赖与脚本管理

真实的 yt-dlp 解析和 FFmpeg 下载处理将在后续阶段接入。

## 开发环境

- Bun 1.3.14
- Rust 1.85 或更高版本
- 当前平台所需的 Tauri 系统依赖

```bash
bun install
bun run tauri dev
```

## 验证

```bash
bun run test
bun run build
cd src-tauri && cargo test
```

## 项目结构

- `src/`：React 界面与前端用例
- `src-tauri/src/domain/`：稳定的领域模型
- `src-tauri/src/application/`：应用状态与引擎接口
- `src-tauri/src/infrastructure/`：Mock 与未来的媒体引擎适配器
- `src-tauri/src/commands/`：Tauri 命令边界
- `docs/`：产品和架构决策

## 项目文档

- [`PLAN.md`](PLAN.md)：开发阶段、任务状态与验收条件
- [`CHANGELOG.md`](CHANGELOG.md)：版本变化与当前限制
- [`docs/product.md`](docs/product.md)：当前阶段的产品范围
- [`docs/architecture.md`](docs/architecture.md)：模块职责与安全边界
