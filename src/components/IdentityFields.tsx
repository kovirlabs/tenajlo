import { useEffect, useRef } from "react";
import type { IdentityForm } from "../hooks/useIdentityForm";

/** The Name and Email inputs of an identity form (see `useIdentityForm`). */
export function IdentityFields({ form, autoFocus }: { form: IdentityForm; autoFocus?: boolean }) {
  const nameInput = useRef<HTMLInputElement>(null);
  // `autoFocus` would hit the input while it's still disabled for the prefill.
  useEffect(() => {
    if (autoFocus && form.loaded) nameInput.current?.focus();
  }, [autoFocus, form.loaded]);

  return (
    <>
      <label>
        Name
        <input
          value={form.name}
          disabled={!form.loaded}
          onChange={(e) => form.setName(e.target.value)}
          ref={nameInput}
        />
      </label>
      <label>
        Email
        <input
          type="email"
          value={form.email}
          disabled={!form.loaded}
          onChange={(e) => form.setEmail(e.target.value)}
        />
      </label>
    </>
  );
}
