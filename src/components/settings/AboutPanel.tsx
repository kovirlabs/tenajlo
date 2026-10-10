import { useState } from "react";
import type { AppError, AppInfo } from "../../bindings";
import { getAppInfo, openLogsFolder } from "../../api/git";
import { openReleasePage } from "../../api/updates";
import { useSettingsStore } from "../../stores/settingsStore";
import { useUpdateStore } from "../../stores/updateStore";
import { InlineError } from "../InlineError";
import { useAsyncEffect } from "../../hooks/useAsyncEffect";

/** Version, log files (for bug reports), and where licenses are listed. */
export function AboutPanel() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<AppError | null>(null);

  useAsyncEffect(async (live) => {
    const res = await getAppInfo();
    if (!live()) return;
    if (res.status === "ok") setInfo(res.data);
    else setError(res.error);
  }, []);

  const showLogs = async () => {
    const res = await openLogsFolder();
    setError(res.status === "error" ? res.error : null);
  };

  return (
    <div className="form">
      <div className="settings-group">
        <h3>Tenajlo {info?.version}</h3>
        <p className="muted">
          A desktop Git client for Forgejo. Apache License 2.0. Third-party licenses are listed in
          THIRD_PARTY_LICENSES.html in Tenajlo's install folder.
        </p>
      </div>
      {info && <UpdatesGroup canUpdate={info.canUpdate} onError={setError} />}
      <div className="settings-group">
        <h3>Log files</h3>
        <p className="muted">
          Tenajlo keeps a log for each of the last 7 days. Passwords and tokens are removed from it.
          If you report a problem, attach the latest log.
        </p>
        {info && (
          <p className="muted">
            <code>{info.logsDir}</code>
          </p>
        )}
        <div className="dialog-actions">
          <button type="button" className="secondary" onClick={() => void showLogs()}>
            Show log files
          </button>
        </div>
      </div>
      <InlineError error={error} />
    </div>
  );
}

/** Settings → About → Updates: the startup check, "Check now", and the release page. */
function UpdatesGroup({
  canUpdate,
  onError,
}: {
  canUpdate: boolean;
  onError: (e: AppError | null) => void;
}) {
  const enabled = useSettingsStore((s) => s.settings?.checkForUpdates ?? false);
  const update = useSettingsStore((s) => s.update);
  const available = useUpdateStore((s) => s.available);
  const upToDate = useUpdateStore((s) => s.upToDate);
  const checking = useUpdateStore((s) => s.checking);
  const updateError = useUpdateStore((s) => s.error);
  const check = useUpdateStore((s) => s.check);
  const showReleases = async () => {
    const res = await openReleasePage();
    onError(res.status === "error" ? res.error : null);
  };

  if (!canUpdate) {
    return (
      <div className="settings-group">
        <h3>Updates</h3>
        <p className="muted">
          This copy of Tenajlo doesn&apos;t update itself. New versions are on the release page.
        </p>
        <div className="dialog-actions">
          <button type="button" className="secondary" onClick={() => void showReleases()}>
            Open release page
          </button>
        </div>
      </div>
    );
  }
  return (
    <div className="settings-group">
      <h3>Updates</h3>
      <label className="radio">
        <input
          type="checkbox"
          checked={enabled}
          onChange={(e) => void update({ checkForUpdates: e.target.checked }).then(onError)}
        />
        Check for updates when Tenajlo starts
      </label>
      <p className="muted">
        Tenajlo asks github.com for the newest release. Updates are only installed when you choose
        to, and only if they&apos;re signed by Tenajlo&apos;s release key.
      </p>
      {available ? (
        <p>Tenajlo {available.version} is available. Use the banner at the top to install it.</p>
      ) : (
        upToDate && <p className="muted">You have the newest version.</p>
      )}
      <InlineError error={updateError} />
      <div className="dialog-actions">
        <button
          type="button"
          className="secondary"
          disabled={checking}
          onClick={() => {
            useUpdateStore.setState({ dismissed: false });
            void check(false);
          }}
        >
          {checking ? "Checking…" : "Check now"}
        </button>
      </div>
    </div>
  );
}
