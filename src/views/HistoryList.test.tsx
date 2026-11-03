import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import type { Commit } from "../bindings";
import { useHistoryStore } from "../stores/historyStore";
import { HistoryList } from "./HistoryList";

const commit = (n: number): Commit => ({
  sha: `${n}`.padStart(40, "a"),
  shortSha: `${n}`.padStart(7, "a"),
  parents: [],
  authorName: "Evan",
  authorEmail: "e@x.io",
  authorDate: new Date().toISOString(),
  summary: `Commit ${n}`,
  body: "",
  unpushed: false,
});

describe("HistoryList", () => {
  it("renders commits, selects, and asks for more at the end", () => {
    const selectCommit = vi.fn(async () => {});
    const loadMore = vi.fn(async () => {});
    useHistoryStore.setState({
      commits: [commit(1), commit(2)],
      error: null,
      loading: false,
      selectedSha: null,
      selectCommit,
      loadMore,
    });
    render(<HistoryList />);
    fireEvent.click(screen.getByText("Commit 2"));
    expect(selectCommit).toHaveBeenCalledWith(commit(2).sha);
    // Two rows fit in the viewport, so the end is visible.
    expect(loadMore).toHaveBeenCalled();
  });

  it("moves the selection with the arrow keys and marks unpushed commits", () => {
    const selectCommit = vi.fn(async () => {});
    useHistoryStore.setState({
      commits: [{ ...commit(1), unpushed: true }, commit(2)],
      error: null,
      loading: false,
      selectedSha: commit(1).sha,
      selectCommit,
      loadMore: vi.fn(async () => {}),
    });
    render(<HistoryList />);
    expect(screen.getAllByText("Not pushed")).toHaveLength(1);
    const list = screen.getByRole("listbox", { name: "Commits" });
    fireEvent.keyDown(list, { key: "ArrowDown" });
    expect(selectCommit).toHaveBeenCalledWith(commit(2).sha);
  });

  it("shows an empty state", () => {
    useHistoryStore.setState({ commits: [], error: null, loading: false });
    render(<HistoryList />);
    expect(screen.getByText("No commits yet.")).toBeTruthy();
  });
});
