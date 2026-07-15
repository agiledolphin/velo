import { describe, expect, it } from "vitest";
import { InspectionGeneration } from "./inspection-generation";

describe("InspectionGeneration", () => {
  it("invalidates a result even when cancellation happens after the request resolved", () => {
    const guard = new InspectionGeneration();
    const request = guard.start();

    expect(guard.isCurrent(request)).toBe(true);
    guard.invalidate();
    expect(guard.isCurrent(request)).toBe(false);
  });

  it("prevents an older request from replacing a newer one", () => {
    const guard = new InspectionGeneration();
    const first = guard.start();
    const second = guard.start();

    expect(guard.isCurrent(first)).toBe(false);
    expect(guard.isCurrent(second)).toBe(true);
  });
});
