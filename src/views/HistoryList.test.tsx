import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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
});

describe("HistoryList", () => {
  afterEach(cleanup);

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

  it("shows an empty state", () => {
    useHistoryStore.setState({ commits: [], error: null, loading: false });
    render(<HistoryList />);
    expect(screen.getByText("No commits yet.")).toBeTruthy();
  });
});
