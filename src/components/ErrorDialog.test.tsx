import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import type { AppError } from "../bindings";
import { useSyncStore } from "../stores/syncStore";
import { useUiStore } from "../stores/uiStore";
import { ErrorDialog, errorTitle } from "./ErrorDialog";

const error = (kind: AppError["kind"], accountId: string | null): AppError => ({
  kind,
  gitKind: null,
  message: "Your sign-in for https://h has stopped working.",
  details: null,
  accountId,
});

describe("ErrorDialog", () => {
  it("offers to sign in again when the account's token stopped working", () => {
    useUiStore.setState({ error: error("SignInRequired", "a1"), dialog: null });
    render(<ErrorDialog />);
    fireEvent.click(screen.getByRole("button", { name: "Sign in again" }));
    const ui = useUiStore.getState();
    expect(ui.error).toBeNull();
    expect(ui.dialog).toBe("settings");
    expect(ui.signInAgainId).toBe("a1");
  });

  it("plain errors only have OK", () => {
    useUiStore.setState({ error: error("Git", null) });
    render(<ErrorDialog />);
    expect(screen.queryByRole("button", { name: "Sign in again" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "OK" }));
    expect(useUiStore.getState().error).toBeNull();
  });

  it("offers to merge when the branches have diverged", () => {
    const run = vi.fn(async () => {});
    useSyncStore.setState({ run });
    useUiStore.setState({ error: { ...error("Git", null), gitKind: "PullDiverged" } });
    render(<ErrorDialog />);
    fireEvent.click(screen.getByRole("button", { name: "Merge the server's changes" }));
    expect(run).toHaveBeenCalledWith("PullMerge");
    expect(useUiStore.getState().error).toBeNull();
  });

  it("offers to pull after a rejected push, with a title that says why", () => {
    const run = vi.fn(async () => {});
    useSyncStore.setState({ run });
    useUiStore.setState({ error: { ...error("Git", null), gitKind: "PushRejected" } });
    render(<ErrorDialog />);
    expect(screen.getByText("The server has newer commits")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Get the server's commits" }));
    expect(run).toHaveBeenCalledWith("Pull");
    expect(useUiStore.getState().error).toBeNull();
  });

  it("keeps the generic title for real failures", () => {
    expect(errorTitle(error("Git", null))).toBe("Something went wrong");
    expect(errorTitle(error("InvalidInput", null))).toBe("Can't do that");
    expect(errorTitle({ ...error("Git", null), gitKind: "LfsFileLocked" })).toBe(
      "Someone else has these files locked",
    );
  });
});
