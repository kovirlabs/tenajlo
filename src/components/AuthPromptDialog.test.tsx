import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { AuthPromptRequested } from "../bindings";
import { useSyncStore } from "../stores/syncStore";
import { AuthPromptDialog } from "./AuthPromptDialog";

let emit: ((p: AuthPromptRequested) => void) | null = null;
const answerAuthPrompt = vi.fn(async () => {});
vi.mock("../api/sync", () => ({
  onAuthPrompt: vi.fn(async (cb: (p: AuthPromptRequested) => void) => {
    emit = cb;
    return () => {};
  }),
  answerAuthPrompt: (...a: unknown[]) => (answerAuthPrompt as (...x: unknown[]) => unknown)(...a),
}));

const prompt = (kind: AuthPromptRequested["kind"], opId = "op1"): AuthPromptRequested => ({
  promptId: `p-${opId}`,
  repoId: "r",
  opId,
  kind,
});

describe("AuthPromptDialog", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  async function show(p: AuthPromptRequested) {
    useSyncStore.setState({ running: { opId: "op1", request: "Fetch", progress: null } });
    render(<AuthPromptDialog />);
    await waitFor(() => expect(emit).not.toBeNull());
    act(() => emit?.(p));
  }

  it("asks for username and password in one dialog", async () => {
    await show(prompt({ type: "Credentials", host: "TMC-GIT01.tmus.local" }));
    expect(screen.getByText("Sign in to TMC-GIT01.tmus.local")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Username"), { target: { value: "evan" } });
    fireEvent.change(screen.getByLabelText("Password or token"), { target: { value: "pat" } });
    fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
    expect(answerAuthPrompt).toHaveBeenCalledWith("p-op1", { username: "evan", secret: "pat" });
    expect(screen.queryByText(/Sign in to/)).toBeNull();
  });

  it("cancel sends null", async () => {
    await show(prompt({ type: "Password", host: "h", username: "evan" }));
    expect(screen.getByText("evan")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(answerAuthPrompt).toHaveBeenCalledWith("p-op1", null);
  });

  it("drops prompts for operations that are no longer running", async () => {
    await show(prompt({ type: "Credentials", host: "h" }, "old-op"));
    expect(screen.queryByText(/Sign in to/)).toBeNull();
  });
});
