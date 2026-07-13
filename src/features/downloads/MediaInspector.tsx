import { FormEvent, useId, useState } from "react";
import type { MediaInfo } from "../../lib/media";
import { normalizeWebUrl } from "../../lib/url";
import { inspectMedia } from "../../lib/velo-api";

type InspectorState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "ready"; media: MediaInfo }
  | { status: "error"; message: string };

function formatDuration(seconds: number | null) {
  if (seconds === null) return "时长未知";
  const minutes = Math.floor(seconds / 60);
  const remainder = seconds % 60;
  return `${minutes}:${remainder.toString().padStart(2, "0")}`;
}

function formatFileSize(bytes: number | null) {
  if (bytes === null) return "大小待定";
  return `${(bytes / 1024 / 1024).toFixed(0)} MB`;
}

export function MediaInspector() {
  const inputId = useId();
  const [url, setUrl] = useState("");
  const [state, setState] = useState<InspectorState>({ status: "idle" });

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const normalizedUrl = normalizeWebUrl(url);

    if (!normalizedUrl) {
      setState({ status: "error", message: "请输入完整的 http 或 https 视频页面地址。" });
      return;
    }

    setState({ status: "loading" });
    try {
      const media = await inspectMedia(normalizedUrl);
      setState({ status: "ready", media });
    } catch (error) {
      setState({
        status: "error",
        message: error instanceof Error ? error.message : "暂时无法解析这个地址。",
      });
    }
  }

  return (
    <div className="inspector">
      <form className="capture-form" onSubmit={handleSubmit} noValidate>
        <label htmlFor={inputId}>视频页面地址</label>
        <div className={`url-capture ${state.status === "loading" ? "is-loading" : ""}`}>
          <span className="link-icon" aria-hidden="true">
            ↗
          </span>
          <input
            id={inputId}
            type="url"
            inputMode="url"
            autoComplete="url"
            placeholder="https://example.com/video"
            value={url}
            disabled={state.status === "loading"}
            aria-describedby={state.status === "error" ? `${inputId}-error` : undefined}
            onChange={(event) => {
              setUrl(event.target.value);
              if (state.status === "error") setState({ status: "idle" });
            }}
          />
          <button type="submit" disabled={state.status === "loading"}>
            {state.status === "loading" ? "正在辨认" : "解析视频"}
            <span aria-hidden="true">→</span>
          </button>
          <span className="capture-glint" aria-hidden="true" />
        </div>
      </form>

      <div
        className={`inspector-status is-${state.status}`}
        aria-live="polite"
      >
        {state.status === "idle" && (
          <div className="empty-state">
            <span className="pulse-dot" aria-hidden="true" />
            <p>等待一个视频地址</p>
            <span>第一阶段使用模拟引擎，不会发起真实下载。</span>
          </div>
        )}

        {state.status === "loading" && (
          <div className="loading-state">
            <span className="loading-orbit" aria-hidden="true" />
            <div>
              <strong>正在辨认页面内容</strong>
              <p>检查标题、时长与可用画质…</p>
            </div>
          </div>
        )}

        {state.status === "error" && (
          <div className="error-state" id={`${inputId}-error`} role="alert">
            <strong>地址无法解析</strong>
            <p>{state.message}</p>
          </div>
        )}

        {state.status === "ready" && <MediaResult media={state.media} />}
      </div>
    </div>
  );
}

function MediaResult({ media }: { media: MediaInfo }) {
  return (
    <article className="media-result">
      <div className="media-summary">
        <div className="thumbnail-placeholder" aria-hidden="true">
          <span>VELO</span>
        </div>
        <div>
          <span className="site-label">{media.site}</span>
          <h2>{media.title}</h2>
          <p>{formatDuration(media.durationSeconds)} · 模拟解析结果</p>
        </div>
      </div>

      <div className="format-list" aria-label="可用格式">
        {media.formats.map((format, index) => (
          <label className="format-option" key={format.id}>
            <input
              type="radio"
              name="media-format"
              value={format.id}
              defaultChecked={index === 0}
            />
            <span className="radio-mark" aria-hidden="true" />
            <span className="format-title">{format.label}</span>
            <span>{format.container.toUpperCase()}</span>
            <span>{formatFileSize(format.filesizeBytes)}</span>
          </label>
        ))}
      </div>

      <button className="download-button" type="button" disabled>
        下载功能将在第二阶段启用
      </button>
    </article>
  );
}
