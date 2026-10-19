import { commands, type PublishOptions, type RemoteRepository } from "../bindings";
import type { Result } from "./result";

/** Where the account's token may create repositories. Rust checks without creating anything. */
export function getPublishOptions(accountId: string): Promise<Result<PublishOptions>> {
  return commands.getPublishOptions(accountId);
}

export type PublishRequest = {
  repoId: string;
  accountId: string;
  /** `null` publishes to the signed-in user. */
  organization: string | null;
  name: string;
  description: string;
  isPrivate: boolean;
  ssh: boolean;
};

/** Creates the repository on Forgejo and adds it as `origin`. The branch push is a normal sync. */
export function publishRepository(r: PublishRequest): Promise<Result<RemoteRepository>> {
  return commands.publishRepository(
    r.repoId,
    r.accountId,
    r.organization,
    r.name,
    r.description,
    r.isPrivate,
    r.ssh,
  );
}
