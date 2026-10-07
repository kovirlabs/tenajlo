import { useState } from "react";
import type { Identity } from "../bindings";
import { setGlobalIdentity } from "../api/changes";
import { useUiStore } from "../stores/uiStore";
import { Modal } from "./Modal";

type Props = {
  open: boolean;
  initial: Identity;
  onClose: () => void;
  onSaved: () => void;
};

/** Asks for name and email and saves them to the global git config after explicit confirmation. */
export function IdentityDialog({ open, initial, onClose, onSaved }: Props) {
  const [name, setName] = useState(initial.name ?? "");
  const [email, setEmail] = useState(initial.email ?? "");
  const [saving, setSaving] = useState(false);
  const valid = name.trim() !== "" && email.trim() !== "";

  const save = async () => {
    setSaving(true);
    const res = await setGlobalIdentity(name, email);
    setSaving(false);
    if (res.status === "error") return useUiStore.getState().showError(res.error);
    onSaved();
  };

  return (
    <Modal open={open} title="Who's making these commits?" onClose={onClose}>
      <form
        className="form"
        onSubmit={(e) => {
          e.preventDefault();
          if (valid) void save();
        }}
      >
        <p>Git records a name and email on every commit you make.</p>
        <label>
          Name
          <input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
        </label>
        <label>
          Email
          <input type="email" value={email} onChange={(e) => setEmail(e.target.value)} />
        </label>
        <p className="muted">
          This saves your name and email to your global Git settings, so every repository on this
          computer will use them.
        </p>
        <div className="dialog-actions">
          <button type="button" className="secondary" onClick={onClose}>
            Cancel
          </button>
          <button type="submit" disabled={!valid || saving}>
            Save
          </button>
        </div>
      </form>
    </Modal>
  );
}
