export const DEFAULT_MAX_CONCURRENT_INSPECTIONS = 2;

function abortError() {
  return new DOMException("Task was cancelled", "AbortError");
}

interface QueuedTask {
  start: () => void;
}

export class TaskScheduler {
  private activeCount = 0;
  private queue: QueuedTask[] = [];

  constructor(private readonly concurrency: number) {
    if (!Number.isInteger(concurrency) || concurrency < 1) {
      throw new Error("Task scheduler concurrency must be a positive integer.");
    }
  }

  schedule<T>(
    signal: AbortSignal,
    onStart: () => void,
    task: () => Promise<T>,
  ): Promise<T> {
    if (signal.aborted) return Promise.reject(abortError());

    return new Promise<T>((resolve, reject) => {
      let started = false;
      let settled = false;

      const queuedTask: QueuedTask = {
        start: () => {
          if (settled) return;
          if (signal.aborted) {
            settled = true;
            reject(abortError());
            return;
          }

          started = true;
          signal.removeEventListener("abort", cancelQueuedTask);
          this.activeCount += 1;
          onStart();

          void Promise.resolve()
            .then(task)
            .then(resolve, reject)
            .finally(() => {
              settled = true;
              this.activeCount -= 1;
              this.drain();
            });
        },
      };

      const cancelQueuedTask = () => {
        if (started || settled) return;
        settled = true;
        this.queue = this.queue.filter((entry) => entry !== queuedTask);
        reject(abortError());
      };

      signal.addEventListener("abort", cancelQueuedTask, { once: true });
      this.queue.push(queuedTask);
      this.drain();
    });
  }

  private drain() {
    while (this.activeCount < this.concurrency) {
      const next = this.queue.shift();
      if (!next) return;
      next.start();
    }
  }
}
