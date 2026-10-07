import { commands, type Account, type Identity, type ServerInfo } from "../bindings";
import type { Result } from "./result";

export function listAccounts(): Promise<Account[]> {
  return commands.listAccounts();
}

/** Checks the address is a reachable Forgejo server Tenajlo trusts. */
export function checkServer(server: string): Promise<Result<ServerInfo>> {
  return commands.checkServer(server);
}

/** Verifies the token and saves it to the OS keychain. The token never comes back. */
export function signIn(server: string, token: string): Promise<Result<Account>> {
  return commands.signIn(server, token);
}

export function signOut(id: string): Promise<Result<null>> {
  return commands.signOut(id);
}

/** Opens the server's token settings page in the default browser. */
export function openTokenSettings(server: string): Promise<Result<null>> {
  return commands.openTokenSettings(server);
}

/** Name and email from the account's Forgejo profile (to prefill the git identity). */
export function getAccountIdentity(accountId: string): Promise<Result<Identity>> {
  return commands.getAccountIdentity(accountId);
}
