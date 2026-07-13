# Velo 第一阶段架构

Velo 当前采用模块化单体结构。UI 不依赖具体下载引擎，Tauri 命令只作为进程边界，业务接口和领域模型保留在 Rust 核心内。

```text
React UI
   │ invoke("inspect_url")
   ▼
Tauri command
   ▼
AppState / MediaEngine
   ▼
MockMediaEngine
```

## 模块职责

- `domain`：媒体信息、格式和可序列化错误。
- `application`：`MediaEngine` 接口和应用状态。
- `infrastructure`：引擎的具体实现；当前只有 Mock。
- `commands`：将前端调用转交给应用层，不构造媒体数据。

## 后续替换点

第二阶段新增 `YtDlpEngine` 并实现同一个 `MediaEngine` 接口。前端、命令层和领域模型不需要因引擎变化而重写。

纯浏览器开发模式无法调用 Tauri IPC，因此在 `Vite DEV` 且不处于 Tauri 环境时使用一份只读预览 fixture，方便检查完整 UI。Tauri 开发和生产环境始终调用 Rust 引擎。

## 当前安全边界

- 前后端都只接受 HTTP 和 HTTPS 地址。
- UI 不能提交任意命令行参数。
- 默认能力仅包含 Tauri 核心权限和顶部自定义标题区所需的窗口拖拽权限。
- 第一阶段未启动外部进程、未读写下载目录、未处理凭证。

开发阶段暂时沿用 Tauri 模板的空 CSP；引入远程封面或媒体引擎之前必须设置明确的内容安全策略。
