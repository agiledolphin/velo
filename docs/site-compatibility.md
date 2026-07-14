# 真实站点兼容性验证

本文件记录 Velo 使用固定版本 yt-dlp 读取公开媒体信息的兼容性结果。测试仅用于调用方有权访问和验证的公开页面，不加载 Cookie、不绕过登录、付费或地区限制，也不下载媒体文件。

## 执行方式

本机运行前先安装并验证固定版本引擎：

```bash
bun run engine:install
VELO_INTEGRATION_TEST_URL=https://example.com/video \
VELO_YT_DLP_PATH=/absolute/path/to/verified/yt-dlp \
bun run test:integration
```

跨平台验证使用 GitHub Actions 的 `Site compatibility check` 手动工作流。待测地址保存在仓库 Actions Secret `VELO_INTEGRATION_TEST_URL` 中，不作为工作流输入、源码或构建产物保存。工作流在 macOS arm64、Windows x64 和 Linux x64 上分别安装并校验固定版本引擎，然后运行同一个被默认忽略的集成测试。

测试日志只记录 yt-dlp 版本、平台、规范化站点名、时长和格式数量，不记录原始地址或媒体标题。更新地址后应重新运行三平台工作流，并按站点新增一行结果。

## 兼容性矩阵

| 日期 | 站点 | yt-dlp | macOS arm64 | Windows x64 | Linux x64 | 备注 |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-07-14 | `x.com` | `2026.07.04` | 通过 | 通过 | 通过 | [运行 29313750794](https://github.com/agiledolphin/velo/actions/runs/29313750794)；2056 秒，5 个格式 |

结果使用以下状态：

- `通过`：成功返回非空标题和至少一个规范化格式。
- `受限`：站点可识别，但需要登录、Cookie、地区权限或触发限流。
- `不支持`：当前安全参数或固定引擎版本无法解析。
- `失败`：网络、引擎响应或未知错误，需要进一步诊断。
