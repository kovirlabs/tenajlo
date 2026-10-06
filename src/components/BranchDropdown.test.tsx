import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { Branch } from "../bindings";
import { useBranchStore } from "../stores/branchStore";
import { useChangesStore } from "../stores/changesStore";
import { BranchDropdown } from "./BranchDropdown";

const branch = (name: string, over: Partial<Branch> = {}): Branch => ({
  name,
  kind: "Local",
  remote: null,
  upstream: null,
  tip: "abc",
  isCurrent: false,
  lastCommitDate: "2026-10-01T00:00:00Z",
  ...over,
});

const setHead = (name: string | null, tip: string | null) =>
  useChangesStore.setState({
    status: {
      branch: { name, tip, upstream: null, ahead: 0, behind: 0, upstreamGone: false },
      files: [],
      hasConflicts: false,
    },
  });

describe("BranchDropdown", () => {
  afterEach(cleanup);

  it("groups branches and filters", () => {
    setHead("main", "abc");
    useBranchStore.setState({
      branches: {
        current: "main",
        local: [branch("main", { isCurrent: true }), branch("feature/pump-io")],
        remoteOnly: [branch("origin/hotfix", { kind: "Remote", remote: "origin" })],
      },
    });
    render(<BranchDropdown />);
    fireEvent.click(screen.getByRole("button", { name: "Current branch" }));
    expect(screen.getByText("Other branches")).toBeTruthy();
    expect(screen.getByText("origin/hotfix")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Filter branches"), { target: { value: "pump" } });
    expect(screen.queryByText("origin/hotfix")).toBeNull();
    expect(screen.getByText("feature/pump-io")).toBeTruthy();
  });

  it("labels a detached HEAD", () => {
    setHead(null, "abc");
    useBranchStore.setState({ branches: { current: null, local: [], remoteOnly: [] } });
    render(<BranchDropdown />);
    expect(screen.getByText("Detached HEAD")).toBeTruthy();
  });
});
