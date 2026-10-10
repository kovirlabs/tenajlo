import { useUpdateStore } from "../stores/updateStore";
import { InlineError } from "./InlineError";

/** "A new version is available", from the startup check or Settings → About. */
export function UpdateBanner() {
  const available = useUpdateStore((s) => s.available);
  const dismissed = useUpdateStore((s) => s.dismissed);
  const installing = useUpdateStore((s) => s.installing);
  const percent = useUpdateStore((s) => s.percent);
  const error = useUpdateStore((s) => s.error);
  const install = useUpdateStore((s) => s.install);
  const dismiss = useUpdateStore((s) => s.dismiss);
  if (!available || dismissed) return null;

  return (
    <section className="conflict-banner update-banner" role="status">
      <div>
        <strong>Tenajlo {available.version} is available.</strong>{" "}
        {installing
          ? `Downloading${percent === null ? "…" : ` ${percent}%`}. Tenajlo will restart when it's ready.`
          : "Installing it restarts Tenajlo."}
        <InlineError error={error} />
      </div>
      {!installing && (
        <div className="account-actions">
          <button type="button" className="secondary" onClick={dismiss}>
            Later
          </button>
          <button type="button" onClick={() => void install()}>
            Install and restart
          </button>
        </div>
      )}
    </section>
  );
}
