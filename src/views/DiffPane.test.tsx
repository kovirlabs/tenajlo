import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import type { WorkingDiff } from "../bindings";
import { DiffPane } from "./DiffPane";

const unchanged = async () => ({
  status: "ok" as const,
  data: { diff: { type: "Unchanged" }, lines: null } as WorkingDiff,
});

describe("DiffPane", () => {
  it("says a renamed file's contents didn't change, naming the old path", async () => {
    render(
      <DiffPane
        source={{
          key: "work:r:new.txt",
          title: "old.txt → new.txt",
          renamedFrom: "old.txt",
          fetch: unchanged,
        }}
        empty="No changes to show."
      />,
    );
    expect(screen.getByText("old.txt → new.txt")).toBeTruthy();
    expect(
      await screen.findByText("Renamed from old.txt. The contents didn't change."),
    ).toBeTruthy();
  });

  it("keeps the plain message for a file that wasn't renamed", async () => {
    render(
      <DiffPane
        source={{ key: "work:r:mode.sh", title: "mode.sh", fetch: unchanged }}
        empty="No changes to show."
      />,
    );
    expect(await screen.findByText("No content changes in this file.")).toBeTruthy();
  });
});
