import { FormEvent, useEffect, useId, useRef, useState } from "react";
import type { DownloadEvent, DownloadTask, MediaInfo } from "../../lib/media";
import { normalizeWebUrl } from "../../lib/url";
import {
  cancelDownload,
  chooseDownloadTarget,
  fetchThumbnailDataUrl,
  inspectMedia,
  isInspectionAbort,
  onDownloadEvent,
  startDownload,
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

function formatTransferSize(bytes: number) {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function formatSpeed(bytesPerSecond: number | null) {
  if (bytesPerSecond === null) return "正在计算速度";
  return `${formatTransferSize(bytesPerSecond)}/s`;
}

function formatEta(seconds: number | null) {
  if (seconds === null) return "剩余时间待定";
  if (seconds < 60) return `约 ${seconds} 秒`;
  return `约 ${Math.ceil(seconds / 60)} 分钟`;
}

export function MediaInspector() {
  const inputId = useId();
  const [url, setUrl] = useState("");
  const [state, setState] = useState<InspectorState>({ status: "idle" });
  const [downloadBusy, setDownloadBusy] = useState(false);
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
    if (downloadBusy) return;
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
            disabled={state.status === "loading" || downloadBusy}
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
            disabled={downloadBusy}
            onClick={state.status === "loading" ? handleCancel : undefined}
          >
            {downloadBusy
              ? "下载进行中"
              : state.status === "loading"
                ? "取消解析"
                : "解析视频"}
            <span aria-hidden="true">{state.status === "loading" ? "×" : "→"}</span>
          </button>
          <span className="capture-glint" aria-hidden="true" />
        </div>
      </form>

      <div
        className={`inspector-status is-${state.status}`}
        aria-live={state.status === "ready" ? "off" : "polite"}
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

        {state.status === "ready" && (
          <MediaResult media={state.media} onBusyChange={setDownloadBusy} />
        )}
      </div>
    </div>
  );
}

function MediaResult({
  media,
  onBusyChange,
}: {
  media: MediaInfo;
  onBusyChange: (busy: boolean) => void;
}) {
  const [selectedFormatId, setSelectedFormatId] = useState(media.formats[0]?.id ?? "");
  const [preparing, setPreparing] = useState(false);
  const [preparedTask, setPreparedTask] = useState<DownloadTask | null>(null);
  const [prepareError, setPrepareError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const [downloadEvent, setDownloadEvent] = useState<DownloadEvent | null>(null);
  const activeTask = useRef<{ id: string; sequence: number } | null>(null);
  const selectedFormat = media.formats.find(({ id }) => id === selectedFormatId);
  const downloadActive =
    starting ||
    downloadEvent?.type === "queued" ||
    downloadEvent?.type === "started" ||
    downloadEvent?.type === "progress" ||
    downloadEvent?.type === "processing";

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onDownloadEvent((event) => {
      const active = activeTask.current;
      if (!active || event.taskId !== active.id || event.sequence <= active.sequence) return;
      active.sequence = event.sequence;
      setDownloadEvent(event);
      if (["completed", "cancelled", "failed"].includes(event.type)) {
        onBusyChange(false);
      }
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [onBusyChange]);

  async function handleChooseDestination() {
    if (!selectedFormat || preparing) return;
    setPreparing(true);
    setPrepareError(null);
    setPreparedTask(null);
    setDownloadEvent(null);
    activeTask.current = null;
    try {
      const task = await chooseDownloadTarget(media, selectedFormat);
      if (task) setPreparedTask(task);
    } catch (error) {
      setPrepareError(error instanceof Error ? error.message : "无法准备保存位置。");
    } finally {
      setPreparing(false);
    }
  }

  async function handleStartDownload() {
    if (!preparedTask || !selectedFormat || starting) return;
    activeTask.current = { id: preparedTask.id, sequence: -1 };
    setStarting(true);
    setPrepareError(null);
    onBusyChange(true);
    try {
      await startDownload(preparedTask, selectedFormat.container);
    } catch (error) {
      activeTask.current = null;
      onBusyChange(false);
      setPrepareError(error instanceof Error ? error.message : "无法开始下载。");
    } finally {
      setStarting(false);
    }
  }

  async function handleCancelDownload() {
    if (!activeTask.current) return;
    const cancelled = await cancelDownload(activeTask.current.id).catch(() => false);
    if (!cancelled) setPrepareError("无法取消这个任务，请稍后再试。");
  }

  return (
    <article className="media-result">
      <div className="media-summary">
        <MediaThumbnail media={media} />
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
              disabled={downloadActive}
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
        <DownloadStatus task={preparedTask} event={downloadEvent} starting={starting} />
      )}
      {prepareError && <p className="download-error" role="alert">{prepareError}</p>}
      {starting ? (
        <button className="download-button" type="button" disabled>
          正在开始下载…
        </button>
      ) : downloadActive ? (
        <button className="download-button is-cancel" type="button" onClick={handleCancelDownload}>
          取消下载
        </button>
      ) : preparedTask && !downloadEvent ? (
        <button
          className="download-button"
          type="button"
          disabled={starting}
          onClick={handleStartDownload}
        >
          {starting ? "正在开始下载…" : "开始下载"}
        </button>
      ) : (
        <button
          className="download-button"
          type="button"
          disabled={!selectedFormat || preparing}
          onClick={handleChooseDestination}
        >
          {preparing ? "正在打开保存位置…" : "选择保存位置"}
        </button>
      )}
    </article>
  );
}

function DownloadStatus({
  task,
  event,
  starting,
}: {
  task: DownloadTask;
  event: DownloadEvent | null;
  starting: boolean;
}) {
  if (!event) {
    return (
      <div className="download-prepared" role="status">
        <strong>{starting ? "正在创建下载任务" : "保存位置已准备"}</strong>
        <span title={task.destinationPath}>{task.destinationPath}</span>
        <p>{starting ? "正在启动 yt-dlp…" : "确认格式后即可开始下载。"}</p>
      </div>
    );
  }

  if (event.type === "failed") {
    return <div className="download-result is-failed" role="alert"><strong>下载失败</strong><p>{event.error.message}</p></div>;
  }
  if (event.type === "completed") {
    return <div className="download-result is-complete" role="status"><strong>下载完成</strong><span title={task.destinationPath}>{task.destinationPath}</span></div>;
  }
  if (event.type === "cancelled") {
    return <div className="download-result" role="status"><strong>下载已取消</strong><p>未完成的临时文件将在后续清理功能中处理。</p></div>;
  }

  const progress = event.type === "progress" ? event.progress : null;
  const fraction =
    progress?.totalBytes && progress.totalBytes > 0
      ? Math.min(progress.downloadedBytes / progress.totalBytes, 1)
      : null;

  return (
    <div className="download-progress">
      <div className="download-progress-copy">
        <strong>{event.type === "processing" ? "正在处理文件" : "正在下载"}</strong>
        <span>{progress ? formatTransferSize(progress.downloadedBytes) : "准备连接…"}</span>
      </div>
      <div
        className={`progress-track${fraction === null ? " is-indeterminate" : ""}`}
        role="progressbar"
        aria-label="下载进度"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={fraction === null ? undefined : Math.round(fraction * 100)}
      >
        <span style={fraction === null ? undefined : { width: `${fraction * 100}%` }} />
      </div>
      <div className="download-metrics">
        <span>{formatSpeed(progress?.speedBytesPerSecond ?? null)}</span>
        <span>{formatEta(progress?.etaSeconds ?? null)}</span>
      </div>
    </div>
  );
}

function MediaThumbnail({ media }: { media: MediaInfo }) {
  const [source, setSource] = useState<string | null>(null);
  const [loading, setLoading] = useState(Boolean(media.thumbnailUrl));

  useEffect(() => {
    let active = true;
    setSource(null);
    setLoading(Boolean(media.thumbnailUrl));
    if (!media.thumbnailUrl) return () => undefined;

    void fetchThumbnailDataUrl(media.thumbnailUrl)
      .then((dataUrl) => {
        if (active) setSource(dataUrl);
      })
      .catch(() => undefined)
      .finally(() => {
        if (active) setLoading(false);
      });

    return () => {
      active = false;
    };
  }, [media.thumbnailUrl]);

  return (
    <div
      className={`thumbnail-placeholder${loading ? " is-loading" : ""}`}
      aria-busy={loading}
    >
      {source ? (
        <img
          src={source}
          alt={`${media.title} 的视频封面`}
          onError={() => setSource(null)}
        />
      ) : (
        <span aria-hidden="true">VELO</span>
      )}
    </div>
  );
}
