import { describe, expect, it, vi } from "vitest";
import { TaskScheduler } from "./task-scheduler";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((accept, decline) => {
    resolve = accept;
    reject = decline;
  });
  return { promise, resolve, reject };
}

describe("TaskScheduler", () => {
  it("starts queued tasks in FIFO order when a slot becomes available", async () => {
    const scheduler = new TaskScheduler(2);
    const first = deferred<string>();
    const second = deferred<string>();
    const third = deferred<string>();
    const starts: string[] = [];

    const one = scheduler.schedule(new AbortController().signal, () => starts.push("one"), () => first.promise);
    const two = scheduler.schedule(new AbortController().signal, () => starts.push("two"), () => second.promise);
    const three = scheduler.schedule(new AbortController().signal, () => starts.push("three"), () => third.promise);

    expect(starts).toEqual(["one", "two"]);
    first.resolve("one");
    await one;
    await vi.waitFor(() => expect(starts).toEqual(["one", "two", "three"]));

    second.resolve("two");
    third.resolve("three");
    await expect(Promise.all([two, three])).resolves.toEqual(["two", "three"]);
  });

  it("removes a cancelled task before it starts", async () => {
    const scheduler = new TaskScheduler(1);
    const first = deferred<void>();
    const controller = new AbortController();
    const queuedTask = vi.fn(async () => undefined);

    const active = scheduler.schedule(new AbortController().signal, () => undefined, () => first.promise);
    const queued = scheduler.schedule(controller.signal, () => undefined, queuedTask);
    controller.abort();

    await expect(queued).rejects.toMatchObject({ name: "AbortError" });
    first.resolve();
    await active;
    expect(queuedTask).not.toHaveBeenCalled();
  });

  it("releases a slot after a task fails", async () => {
    const scheduler = new TaskScheduler(1);
    const starts: string[] = [];

    const failed = scheduler.schedule(
      new AbortController().signal,
      () => starts.push("failed"),
      async () => {
        throw new Error("failed");
      },
    );
    const next = scheduler.schedule(
      new AbortController().signal,
      () => starts.push("next"),
      async () => "done",
    );

    await expect(failed).rejects.toThrow("failed");
    await expect(next).resolves.toBe("done");
    expect(starts).toEqual(["failed", "next"]);
  });
});
