import { FormEvent, useEffect, useId, useRef, useState } from "react";
import type { DownloadTask, MediaInfo } from "../../lib/media";
import { normalizeWebUrl } from "../../lib/url";
import {
  chooseDownloadTarget,
  inspectMedia,
  isInspectionAbort,
} from "../../lib/velo-api";

type InspectorState =
  | { status: "idle"; notice?: string }
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
  const activeRequest = useRef<{
    id: string;
    controller: AbortController;
  } | null>(null);

  useEffect(
    () => () => {
      activeRequest.current?.controller.abort();
    },
    [],
  );

  function handleCancel() {
    const request = activeRequest.current;
    if (!request) return;

    activeRequest.current = null;
    request.controller.abort();
    setState({ status: "idle", notice: "解析已取消" });
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const normalizedUrl = normalizeWebUrl(url);

    if (!normalizedUrl) {
      setState({ status: "error", message: "请输入完整的 http 或 https 视频页面地址。" });
      return;
    }

    const requestId = crypto.randomUUID();
    const controller = new AbortController();
    activeRequest.current = { id: requestId, controller };
    setState({ status: "loading" });
    try {
      const media = await inspectMedia(normalizedUrl, {
        requestId,
        signal: controller.signal,
      });
      if (activeRequest.current?.id !== requestId) return;
      activeRequest.current = null;
      setState({ status: "ready", media });
    } catch (error) {
      if (activeRequest.current?.id !== requestId) return;
      activeRequest.current = null;
      if (isInspectionAbort(error)) {
        setState({ status: "idle", notice: "解析已取消" });
        return;
      }
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
              if (
                state.status === "error" ||
                (state.status === "idle" && state.notice)
              ) {
                setState({ status: "idle" });
              }
            }}
          />
          <button
            className={state.status === "loading" ? "cancel-inspection" : undefined}
            type={state.status === "loading" ? "button" : "submit"}
            onClick={state.status === "loading" ? handleCancel : undefined}
          >
            {state.status === "loading" ? "取消解析" : "解析视频"}
            <span aria-hidden="true">{state.status === "loading" ? "×" : "→"}</span>
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
            <p>{state.notice ?? "等待一个视频地址"}</p>
            <span>
              {state.notice
                ? "可以修改地址后重新开始。"
                : "微落只读取媒体信息，不会自动下载文件。"}
            </span>
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
  const [selectedFormatId, setSelectedFormatId] = useState(media.formats[0]?.id ?? "");
  const [preparing, setPreparing] = useState(false);
  const [preparedTask, setPreparedTask] = useState<DownloadTask | null>(null);
  const [prepareError, setPrepareError] = useState<string | null>(null);
  const selectedFormat = media.formats.find(({ id }) => id === selectedFormatId);

  async function handleChooseDestination() {
    if (!selectedFormat || preparing) return;
    setPreparing(true);
    setPrepareError(null);
    setPreparedTask(null);
    try {
      const task = await chooseDownloadTarget(media, selectedFormat);
      if (task) setPreparedTask(task);
    } catch (error) {
      setPrepareError(error instanceof Error ? error.message : "无法准备保存位置。");
    } finally {
      setPreparing(false);
    }
  }

  return (
    <article className="media-result">
      <div className="media-summary">
        <div className="thumbnail-placeholder" aria-hidden="true">
          <span>VELO</span>
        </div>
        <div>
          <span className="site-label">{media.site}</span>
          <h2>{media.title}</h2>
          <p>{formatDuration(media.durationSeconds)} · 解析结果</p>
        </div>
      </div>

      <div className="format-list" aria-label="可用格式">
        {media.formats.map((format) => (
          <label className="format-option" key={format.id}>
            <input
              type="radio"
              name="media-format"
              value={format.id}
              checked={format.id === selectedFormatId}
              onChange={() => {
                setSelectedFormatId(format.id);
                setPreparedTask(null);
                setPrepareError(null);
              }}
            />
            <span className="radio-mark" aria-hidden="true" />
            <span className="format-title">{format.label}</span>
            <span>{format.container.toUpperCase()}</span>
            <span>{formatFileSize(format.filesizeBytes)}</span>
          </label>
        ))}
      </div>

      {preparedTask && (
        <div className="download-prepared" role="status">
          <strong>保存位置已准备</strong>
          <span title={preparedTask.destinationPath}>{preparedTask.destinationPath}</span>
          <p>下一步将接入真实下载与进度显示。</p>
        </div>
      )}
      {prepareError && <p className="download-error" role="alert">{prepareError}</p>}
      <button
        className="download-button"
        type="button"
        disabled={!selectedFormat || preparing}
        onClick={handleChooseDestination}
      >
        {preparing ? "正在打开保存位置…" : "选择保存位置"}
      </button>
    </article>
  );
}
