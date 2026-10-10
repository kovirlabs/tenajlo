import { beforeEach, describe, expect, it } from "vitest";
import type { AppError, FileDiff } from "../bindings";
import type { Result } from "../api/result";
import { useDiffStore } from "./diffStore";

const failure: AppError = {
  kind: "Internal",
  gitKind: null,
  message: "boom",
  details: null,
  accountId: null,
};

const ok = (diff: FileDiff): Promise<Result<FileDiff>> =>
  Promise.resolve({ status: "ok", data: diff });

describe("diffStore", () => {
  beforeEach(() => useDiffStore.getState().clear());

  it("shows the latest selection, not a slower earlier one", async () => {
    let resolveFirst: (v: Result<FileDiff>) => void = () => {};
    const first = useDiffStore
      .getState()
      .load("work:r:a.txt", () => new Promise((r) => (resolveFirst = r)));
    await useDiffStore.getState().load("work:r:b.txt", () => ok({ type: "Binary" }));
    resolveFirst({ status: "ok", data: { type: "TooLarge" } });
    await first;
    expect(useDiffStore.getState()).toMatchObject({
      key: "work:r:b.txt",
      diff: { type: "Binary" },
    });
  });

  it("keeps the old diff while reloading the same file, and reports errors", async () => {
    await useDiffStore.getState().load("k", () => ok({ type: "Binary" }));
    const reload = useDiffStore.getState().load("k", () => ok({ type: "TooLarge" }));
    expect(useDiffStore.getState().diff).toEqual({ type: "Binary" });
    await reload;
    expect(useDiffStore.getState().diff).toEqual({ type: "TooLarge" });

    await useDiffStore.getState().load("k", async () => ({ status: "error", error: failure }));
    expect(useDiffStore.getState()).toMatchObject({ diff: null, error: failure });
  });
});
