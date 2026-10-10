import { commands, type SavedSecretInfo } from "../bindings";
import type { Result } from "./result";

/** Remembered passwords and SSH passphrases: names only, never the secrets. */
export function listSavedSecrets(): Promise<SavedSecretInfo[]> {
  return commands.listSavedSecrets();
}

/** Deletes a remembered password or passphrase from the keychain. */
export function forgetSavedSecret(id: string): Promise<Result<null>> {
  return commands.forgetSavedSecret(id);
}
