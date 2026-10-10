import { useState } from "react";
import type { AppError, AppInfo } from "../../bindings";
import { getAppInfo, openLogsFolder } from "../../api/git";
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
