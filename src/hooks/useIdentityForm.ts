import { useState } from "react";
import type { AppError, Identity } from "../bindings";
import { setGlobalIdentity } from "../api/changes";
import { useAsyncEffect } from "./useAsyncEffect";

export type IdentityForm = {
  name: string;
  email: string;
  setName: (name: string) => void;
  setEmail: (email: string) => void;
  /** The prefill finished; until then the fields are disabled. */
  loaded: boolean;
  /** Both fields are filled in. */
  valid: boolean;
  saving: boolean;
  /** Why the last save failed. */
  error: AppError | null;
  /** Saves to the global git config. Call only from an explicit Save (CLAUDE.md rule 5). */
  save: () => Promise<boolean>;
};

/**
 * Name and email for commits, prefilled from `prefill` (run once on mount; missing values
 * stay empty), saved to the user's global git config.
 */
export function useIdentityForm(prefill: () => Promise<Partial<Identity>>): IdentityForm {
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [loaded, setLoaded] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  // Runs once: a new `prefill` closure each render must not reload over the user's typing.
  useAsyncEffect(async (live) => {
    const initial = await prefill();
    if (!live()) return;
    setName(initial.name ?? "");
    setEmail(initial.email ?? "");
    setLoaded(true);
  }, []);

  const save = async () => {
    setSaving(true);
    const res = await setGlobalIdentity(name, email);
    setSaving(false);
    setError(res.status === "error" ? res.error : null);
    return res.status === "ok";
  };

  const valid = name.trim() !== "" && email.trim() !== "";
  return { name, email, setName, setEmail, loaded, valid, saving, error, save };
}
