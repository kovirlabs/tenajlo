import { commands, type AccountSshKeys, type LocalSshKey } from "../bindings";
import type { Result } from "./result";

/** Public keys in the user's `.ssh` folder. */
export function listSshKeys(): Promise<Result<LocalSshKey[]>> {
  return commands.listSshKeys();
}

/** Creates `.ssh/id_ed25519`. The passphrase goes to Rust only and is never stored. */
export function createSshKey(
  comment: string,
  passphrase: string | null,
): Promise<Result<LocalSshKey>> {
  return commands.createSshKey(comment, passphrase);
}

/** Which keys an account already has, and whether its token may add more. */
export function getAccountSshKeys(accountId: string): Promise<Result<AccountSshKeys>> {
  return commands.getAccountSshKeys(accountId);
}

/** Adds a public key from `.ssh` to the account. Rust checks the token's permission first. */
export function addSshKey(
  accountId: string,
  fileName: string,
  title: string,
): Promise<Result<null>> {
  return commands.addSshKey(accountId, fileName, title);
}
