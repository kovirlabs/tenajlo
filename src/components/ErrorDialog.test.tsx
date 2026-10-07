import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { AppError } from "../bindings";
import { useUiStore } from "../stores/uiStore";
import { ErrorDialog } from "./ErrorDialog";

const error = (kind: AppError["kind"], accountId: string | null): AppError => ({
  kind,
  gitKind: null,
  message: "Your sign-in for https://h has stopped working.",
  details: null,
  accountId,
});

describe("ErrorDialog", () => {
  afterEach(cleanup);

  it("offers to sign in again when the account's token stopped working", () => {
    useUiStore.setState({ error: error("SignInRequired", "a1"), dialog: null });
    render(<ErrorDialog />);
    fireEvent.click(screen.getByRole("button", { name: "Sign in again" }));
    const ui = useUiStore.getState();
    expect(ui.error).toBeNull();
    expect(ui.dialog).toBe("accounts");
    expect(ui.signInAgainId).toBe("a1");
  });

  it("plain errors only have OK", () => {
    useUiStore.setState({ error: error("Git", null) });
    render(<ErrorDialog />);
    expect(screen.queryByRole("button", { name: "Sign in again" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "OK" }));
    expect(useUiStore.getState().error).toBeNull();
  });
});
