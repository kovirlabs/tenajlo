import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { UpdateProgress } from "../bindings";
import { useUpdateStore } from "../stores/updateStore";
import { UpdateBanner } from "./UpdateBanner";

let progress: ((p: UpdateProgress) => void) | null = null;
const api = vi.hoisted(() => ({
  checkForUpdate: vi.fn(),
  installUpdate: vi.fn(),
  openReleasePage: vi.fn(),
  onUpdateProgress: vi.fn(),
}));
vi.mock("../api/updates", () => api);

const update = { version: "1.1.0", notes: null, releaseUrl: "https://example.invalid" };

describe("UpdateBanner", () => {
  beforeEach(() => {
    useUpdateStore.setState({
      available: null,
      upToDate: false,
      checking: false,
      installing: false,
      percent: null,
      error: null,
      dismissed: false,
    });
    api.onUpdateProgress.mockImplementation(async (cb: (p: UpdateProgress) => void) => {
      progress = cb;
      return () => {};
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("shows nothing until a check finds an update, then installs with progress", async () => {
    render(<UpdateBanner />);
    expect(screen.queryByRole("status")).toBeNull();

    api.checkForUpdate.mockResolvedValue({ status: "ok", data: update });
    await act(() => useUpdateStore.getState().check(true));
    expect(screen.getByText("Tenajlo 1.1.0 is available.")).toBeTruthy();

    let finish: (v: unknown) => void = () => {};
    api.installUpdate.mockReturnValue(new Promise((r) => (finish = r)));
    fireEvent.click(screen.getByRole("button", { name: "Install and restart" }));
    await waitFor(() => expect(progress).not.toBeNull());
    act(() => progress?.({ percent: 42 }));
    expect(screen.getByText(/Downloading 42%/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Later" })).toBeNull();

    // A refusal (e.g. a push is running) brings the buttons back with the reason.
    await act(async () => {
      finish({ status: "error", error: { kind: "InvalidInput", message: "Wait for the push" } });
    });
    expect(screen.getByRole("alert").textContent).toContain("Wait for the push");
    expect(screen.getByRole("button", { name: "Install and restart" })).toBeTruthy();
  });

  it("hides on Later, and a quiet check doesn't report errors", async () => {
    api.checkForUpdate.mockResolvedValue({
      status: "error",
      error: { kind: "InvalidInput", message: "This copy of Tenajlo can't update itself." },
    });
    await useUpdateStore.getState().check(true);
    expect(useUpdateStore.getState().error).toBeNull();

    useUpdateStore.setState({ available: update });
    render(<UpdateBanner />);
    fireEvent.click(screen.getByRole("button", { name: "Later" }));
    expect(screen.queryByRole("status")).toBeNull();
  });
});
