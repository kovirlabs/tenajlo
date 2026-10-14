import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import type { Account } from "../bindings";
import { useAccountStore } from "../stores/accountStore";
import { NoRepository } from "./NoRepository";

const account: Account = {
  id: "a1",
  kind: "forgejo",
  baseUrl: "https://git.example.com",
  login: "evan",
  displayName: "Evan",
  avatarUrl: null,
  needsSignIn: false,
};

describe("NoRepository", () => {
  it("offers to sign in when there's no account", () => {
    useAccountStore.setState({ accounts: [] });
    render(<NoRepository />);
    expect(screen.getByText("Sign in to Forgejo…")).toBeTruthy();
  });

  it("doesn't offer to sign in when already signed in", () => {
    useAccountStore.setState({ accounts: [account] });
    render(<NoRepository />);
    expect(screen.queryByText("Sign in to Forgejo…")).toBeNull();
    expect(screen.getByText("Clone a repository…")).toBeTruthy();
  });
});
