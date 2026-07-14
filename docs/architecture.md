# Velo 架构

Velo 当前采用模块化单体结构。UI 不依赖具体下载引擎，Tauri 命令只作为进程边界，业务接口和领域模型保留在 Rust 核心内。

```text
React UI
   │ invoke("inspect_url")
   ▼
Tauri command
   ▼
AppState / MediaEngine
   ▼
YtDlpEngine / RestrictedProcessRunner
```

## 模块职责

- `domain`：媒体信息、格式和可序列化错误。
- `application`：`MediaEngine` 接口和应用状态。
- `infrastructure`：受限进程执行器和 `YtDlpEngine`；Mock 仅用于单元测试。
- `commands`：将前端调用转交给应用层，不构造媒体数据。

## 引擎替换点

第二阶段已由 `YtDlpEngine` 实现同一个 `MediaEngine` 接口。前端、命令层和领域模型没有因引擎变化而重写；浏览器开发预览仍使用只读 fixture。

`MediaEngine` 和 Tauri 解析命令已改为异步边界，以便等待外部进程而不阻塞应用线程。

每次前端解析使用一个受长度与字符集约束的请求 ID。`AppState` 只登记仍在运行的请求，取消命令通过对应通道通知解析任务；异步选择器随后释放引擎 Future。由于受限进程执行器为子进程启用了 drop 时终止，取消会实际停止对应的 yt-dlp，而不只是让界面停止等待。重复 ID 会取消旧任务，任务完成后仅清理自己的登记，从而避免并发竞态。

## 外部进程边界

第二阶段使用 `RestrictedProcessRunner` 作为所有媒体工具的唯一进程入口：

- 可执行文件在构造执行器时使用绝对路径固定，调用方只能传递独立参数。
- 不经过 Shell，不拼接命令字符串，标准输入始终关闭。
- 标准输出和标准错误并发读取，并分别应用可配置的字节上限。
- 超时后主动终止并回收子进程，避免遗留后台进程。
- 执行错误使用有限枚举表示，不向界面暴露本地路径或原始系统错误。

`YtDlpEngine` 已组合该执行器，并将 yt-dlp JSON 规范化为领域模型。可执行文件优先读取绝对路径环境变量 `VELO_YT_DLP_PATH`，其次查找应用同目录，开发环境最后回退到项目 `binaries/` 目录。

当 yt-dlp 非零退出时，基础设施层只在小写化的受限 stderr 内匹配有限信号，并按优先级映射为限流、地区限制、登录、站点不支持、内容不可用、访问拒绝或网络错误；未知情况保持通用引擎错误。原始 stderr、URL、账号信息和本地路径均不会进入序列化错误。该分类是用户提示层，不依赖它决定进程权限或执行参数。

开发工具将 yt-dlp 固定为单一版本，并按当前操作系统和 CPU 架构选择官方资源。安装过程限制响应大小，使用硬编码 SHA-256 校验内容，通过临时文件和重命名完成原子替换，再验证实际安装版本；更新或验证失败时恢复原文件。下载所得二进制位于 Git 忽略的 `binaries/`，不进入源码历史。

Tauri 开发与发布构建钩子读取 `TAURI_ENV_TARGET_TRIPLE`，将目标映射到固定的官方资产。脚本优先复用校验通过且与目标兼容的开发引擎，否则下载对应资产；随后以 `yt-dlp-$TARGET_TRIPLE` 命名写入 Git 忽略的 `src-tauri/binaries/`。`bundle.externalBin` 将其作为 sidecar 放到应用可执行文件旁，并移除目标后缀。运行时现有的同目录查找因此不需要 Shell 插件或额外前端权限。

当前已验证 Apple Silicon macOS `.app` 内包含正确 SHA-256 且可运行的 yt-dlp。Windows x64 与 Linux x64 已在原生 GitHub 运行器中通过测试、Clippy、Tauri 发布构建、NSIS/DEB sidecar 内容检查和产物上传。

持续集成分为快速验证与发布验证。非文档 push 和 pull request 使用 Ubuntu 运行前端测试与构建、Rust 测试和 Clippy；完整 Windows/Linux 安装包只在版本标签、手动触发或打包关键文件变化时生成。工作流按目标平台复用 Cargo 缓存，并保留并发取消，避免连续提交继续占用旧 runner。

真实站点兼容性使用独立的手动工作流验证。地址只从仓库 Actions Secret 读取，不进入源码、工作流输入或测试输出；同一个忽略型集成测试在 macOS arm64、Windows x64 与 Linux x64 上运行，并只输出规范化站点名、时长和格式数量。测试范围与结果记录在 `docs/site-compatibility.md`。

每次解析固定使用模拟、单视频和单行 JSON 参数，并忽略用户配置、插件目录、外部 JavaScript 运行时、远程组件、缓存与 `exec`。URL 位于参数终止符之后，不能被解释为命令选项。禁用外部 JavaScript 运行时会暂时降低部分站点兼容性，后续仅在有可控打包方案时放开。

纯浏览器开发模式无法调用 Tauri IPC，因此在 `Vite DEV` 且不处于 Tauri 环境时使用一份只读预览 fixture，方便检查完整 UI。Tauri 开发和生产环境始终调用 Rust 引擎。

## 当前安全边界

- 前后端都只接受 HTTP 和 HTTPS 地址。
- UI 不能提交任意命令行参数。
- UI 只能按已登记的请求 ID 取消解析，不能指定或终止任意系统进程。
- 默认能力仅包含 Tauri 核心权限、窗口拖拽和原生保存对话框权限。
- yt-dlp 只在模拟模式下读取媒体信息，不下载文件、不加载 Cookie、不写缓存。
- 远程封面只由 Rust 受限获取，WebView 不直接访问远程图片来源。

生产 CSP 使用 `default-src 'none'`，仅允许自身脚本、样式、字体、图片数据 URL，以及 Tauri IPC 所需的 `ipc:` 和 `http://ipc.localhost` 连接。对象、框架、媒体、Worker、Manifest、表单提交和基础 URL 均显式禁用；不开放 `unsafe-inline`、`unsafe-eval`、远程协议、通配符或文件资产协议。

Tauri 配置显式启用唯一的 `default` capability，避免以后新增 capability 文件时被构建系统自动纳入。该 capability 只授予核心默认权限、自定义顶部区域所需的窗口拖动权限，以及 `dialog:allow-save`；不使用包含打开文件等能力的 `dialog:default`。

开发 CSP 与生产策略分离，只为 Vite 增加自身连接、本机热更新 WebSocket、动态样式和预览所需的 `data:` / `blob:` 图片。扩大 WebView 网络能力时，必须单独评审来源并同步更新安全回归测试，不能直接开放任意 HTTPS 来源。

## 封面获取边界

yt-dlp 返回的封面 URL 不直接交给 WebView。前端通过独立 Tauri 命令请求封面，Rust 只接受不含账号信息的 HTTP/HTTPS 地址，并在每次请求和每次重定向前解析域名、拒绝本机、私网、链路本地、保留和文档网络。验证通过的公网 IP 会固定到该次 HTTP 客户端，降低 DNS 重绑定风险。

请求不携带 Cookie，连接和整次请求分别设置超时，最多跟随三次手动重定向。响应必须是受支持的位图 MIME，声明大小和流式读取均限制为 5 MiB；SVG 与 HTML 不进入 WebView。合法内容编码为 `data:` URL 返回，加载失败只影响封面并回退到品牌占位图，不改变媒体解析结果。

## 下载任务与事件边界

第三阶段使用不可变的 `DownloadTask` 描述一次用户选择：强类型任务 ID、来源地址、媒体标题、格式 ID 和绝对目标路径。任务 ID 继续限制为最多 64 个 ASCII 字母、数字、连字符或下划线；来源只接受 HTTP/HTTPS，标题和格式 ID 具有独立长度上限。

文件名建议由 Rust 从媒体标题生成：保留 Unicode 文本，移除控制字符和跨平台非法字符，限制长度，规避 Windows 保留设备名，并使用经过约束的媒体扩展名。前端只能通过原生保存对话框取得目标路径；对话框取消时不创建任务，选择后 Rust 再验证绝对路径、文件名、扩展名和保留名。当前步骤只准备任务，不创建文件，也不启动 yt-dlp。

任务生命周期通过 `DownloadEvent` 发送，而不是让前端推断外部进程状态。事件包含任务 ID、单调递增的序号和扁平化类型，当前类型为 `queued`、`started`、`progress`、`processing`、`completed`、`cancelled` 与 `failed`。前端后续只接受序号更新的事件，从而忽略异步通道中迟到的进度。

进度统一使用整数字节、可选总大小、每秒字节数和剩余秒数。总大小未知时不伪造百分比；下载量超过估计总量时，展示比例最多为 100%。该模型不依赖 yt-dlp 的输出文本，基础设施适配器负责把引擎进度转换为领域事件。

## 下载执行边界

`YtDlpDownloader` 使用与解析引擎相同的固定绝对 yt-dlp 路径，但拥有独立的流式执行边界。每个任务固定禁用用户配置、插件、外部 JavaScript 运行时、远程组件、缓存、更新与 `exec`，禁止播放列表和覆盖目标文件；格式 ID、输出路径和来源 URL 均作为独立参数传入，URL 位于参数终止符之后。

下载默认写入 `.part` 临时文件并禁用断点续传，成功后由 yt-dlp 原子移动到目标路径。标准输出只解析带固定前缀的 JSON 进度模板，单行和累计输出均有限制；标准错误受独立字节上限约束且不会传给界面。进度最多每 250 毫秒产生一次，转换为稳定的字节、速度和 ETA 领域事件。

`DownloadCoordinator` 在单进程内登记活动任务，拒绝重复任务 ID，并为每个任务持有独立取消通道。取消命令只接受受约束的任务 ID；下载器收到取消后终止并回收对应子进程，最终发送 `cancelled` 事件而不是失败。当前取消可能留下 `.part` 文件，后续安全清理里程碑负责识别和删除，界面不会把临时文件报告为完成结果。

所选格式的音视频标志会在任务创建和启动边界分别验证，并归一化为音视频、仅视频或仅音频三种类型。仅视频任务使用固定格式选择器追加最佳音轨，yt-dlp 通过绝对 `VELO_FFMPEG_PATH` 或应用同目录 FFmpeg 完成合并；已有音频的格式保持单流下载。后处理模板产生 `processing` 事件，目标文件存在且 yt-dlp 成功退出后才发送 `completed`。

FFmpeg 与 yt-dlp 使用相同的目标三元组 sidecar 机制，但拥有独立资产清单。Apple Silicon 的 FFmpeg 8.1 ZIP 与解压后二进制均验证硬编码 SHA-256；其他目标验证各自固定二进制。构建脚本限制下载大小、原子替换 sidecar，发布工作流进一步检查 NSIS 与 DEB 内同时包含两个可执行文件。第三方来源与许可提示作为 Tauri resource 随包分发；正式发布前仍需完成完整许可审查。
