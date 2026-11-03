import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import type { Repository } from "../bindings";
import { useShortcuts } from "../hooks/useShortcuts";
import { useRepoStore } from "../stores/repoStore";
import { useUiStore } from "../stores/uiStore";
import { RepoDropdown } from "./RepoDropdown";

vi.mock("../api/repos", () => ({
  listRepositories: vi.fn(),
  addLocalRepository: vi.fn(),
  removeRepository: vi.fn(),
  selectRepository: vi.fn(),
}));

const repo = (id: string, name: string, missing = false): Repository => ({
  id,
  name,
  path: `/work/${name}`,
  missing,
});

describe("RepoDropdown", () => {
  const select = vi.fn(async () => {});
  const addLocal = vi.fn(async () => {});
  const remove = vi.fn(async () => {});

  beforeEach(() => useUiStore.setState({ repoPickerOpen: false, dialog: null }));
  beforeEach(() =>
    useRepoStore.setState({
      repositories: [repo("1", "plc-exports"), repo("2", "docs", true)],
      selectedId: "1",
      loaded: true,
      select,
      addLocal,
      remove,
    }),
  );
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("shows the current repository and filters the list", () => {
    render(<RepoDropdown />);
    fireEvent.click(screen.getByRole("button", { name: "Current repository" }));
    expect(screen.getByText("(missing)")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Filter repositories"), { target: { value: "PLC" } });
    expect(screen.queryByText("docs")).toBeNull();
    fireEvent.click(screen.getByTitle("/work/plc-exports"));
    expect(select).toHaveBeenCalledWith("1");
  });

  it("adds a local repository", () => {
    render(<RepoDropdown />);
    fireEvent.click(screen.getByRole("button", { name: "Current repository" }));
    fireEvent.click(screen.getByRole("button", { name: "Add local…" }));
    expect(addLocal).toHaveBeenCalled();
  });

  it("opens and closes with Ctrl+T", () => {
    function WithShortcuts() {
      useShortcuts(true);
      return <RepoDropdown />;
    }
    render(<WithShortcuts />);
    expect(screen.queryByLabelText("Filter repositories")).toBeNull();
    fireEvent.keyDown(window, { key: "t", ctrlKey: true });
    expect(screen.getByLabelText("Filter repositories")).toBeTruthy();
    fireEvent.keyDown(window, { key: "t", ctrlKey: true });
    expect(screen.queryByLabelText("Filter repositories")).toBeNull();
    // Not while a dialog is open.
    useUiStore.setState({ dialog: "settings" });
    fireEvent.keyDown(window, { key: "t", ctrlKey: true });
    expect(screen.queryByLabelText("Filter repositories")).toBeNull();
  });

  it("opens the first match on Enter and clears the filter when closed", () => {
    render(<RepoDropdown />);
    const toggle = screen.getByRole("button", { name: "Current repository" });
    fireEvent.click(toggle);
    const filter = screen.getByLabelText("Filter repositories");
    fireEvent.change(filter, { target: { value: "doc" } });
    fireEvent.keyDown(filter, { key: "Enter" });
    expect(select).toHaveBeenCalledWith("2");
    fireEvent.click(toggle);
    expect((screen.getByLabelText("Filter repositories") as HTMLInputElement).value).toBe("");
  });

  it("removes a repository from the list after confirmation", async () => {
    render(<RepoDropdown />);
    fireEvent.click(screen.getByRole("button", { name: "Current repository" }));
    fireEvent.contextMenu(screen.getByTitle("/work/plc-exports"));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Remove from list…" }));
    expect(screen.getByText(/stay on your computer/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    expect(remove).toHaveBeenCalledWith("1");
  });
});
