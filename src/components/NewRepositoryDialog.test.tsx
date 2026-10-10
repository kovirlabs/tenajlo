import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useUiStore } from "../stores/uiStore";
import { NewRepositoryDialog } from "./NewRepositoryDialog";

const api = vi.hoisted(() => ({
  defaultRepositoryFolder: vi.fn(),
  createRepository: vi.fn(),
}));
const load = vi.hoisted(() => vi.fn(async () => {}));
vi.mock("../api/repos", () => api);
vi.mock("../api/settings", () => ({ chooseFolder: vi.fn(async () => "/work/projects") }));
vi.mock("../stores/repoStore", () => ({ useRepoStore: { getState: () => ({ load }) } }));

describe("NewRepositoryDialog", () => {
  beforeEach(() => {
    api.defaultRepositoryFolder.mockResolvedValue({
      status: "ok",
      data: "/Users/e/Documents/Tenajlo",
    });
    useUiStore.setState({ dialog: "newRepository" });
  });
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("creates a repository in the default folder with a README", async () => {
    api.createRepository.mockResolvedValue({ status: "ok", data: {} });
    render(<NewRepositoryDialog />);
    await screen.findByDisplayValue("/Users/e/Documents/Tenajlo");
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "pump-station" } });
    fireEvent.click(screen.getByRole("button", { name: "Create repository" }));
    await waitFor(() =>
      expect(api.createRepository).toHaveBeenCalledWith(
        "/Users/e/Documents/Tenajlo",
        "pump-station",
        true,
      ),
    );
    await waitFor(() => expect(useUiStore.getState().dialog).toBeNull());
    expect(load).toHaveBeenCalled();
  });

  it("uses a chosen folder and shows errors inline", async () => {
    api.createRepository.mockResolvedValue({
      status: "error",
      error: {
        kind: "InvalidInput",
        gitKind: null,
        message: "/work/projects/x already has files in it.",
        details: null,
        accountId: null,
      },
    });
    render(<NewRepositoryDialog />);
    fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    await screen.findByDisplayValue("/work/projects");
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "x" } });
    fireEvent.click(screen.getByLabelText(/Add a README/));
    fireEvent.click(screen.getByRole("button", { name: "Create repository" }));
    expect((await screen.findByRole("alert")).textContent).toContain("already has files");
    expect(api.createRepository).toHaveBeenCalledWith("/work/projects", "x", false);
    expect(useUiStore.getState().dialog).toBe("newRepository");
  });
});
