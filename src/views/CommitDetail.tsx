import { getCommitDiff } from "../api/history";
import { FileStatusIcon } from "../components/FileStatusIcon";
import { useHistoryStore } from "../stores/historyStore";
import { DiffPane } from "./DiffPane";

export function CommitDetail({ repoId }: { repoId: string }) {
  const commit = useHistoryStore((s) => s.commits.find((c) => c.sha === s.selectedSha));
  const files = useHistoryStore((s) => s.files);
  const selectedFile = useHistoryStore((s) => s.selectedFile);
  const selectFile = useHistoryStore((s) => s.selectFile);

  if (!commit) return <p className="pane-message muted">Select a commit to see its changes.</p>;

  return (
    <div className="commit-detail">
      <header className="commit-header">
        <h2>{commit.summary || "(no message)"}</h2>
        {commit.body && <p className="commit-body">{commit.body}</p>}
        <p className="muted">
          {commit.authorName} committed {new Date(commit.authorDate).toLocaleString()} ·{" "}
          <code title={commit.sha}>{commit.shortSha}</code>
        </p>
      </header>
      <div className="commit-split">
        <ul role="listbox" aria-label="Files in commit" className="file-list commit-files">
          {files === null && <li className="pane-message muted">Loading…</li>}
          {files?.length === 0 && <li className="pane-message muted">No file changes.</li>}
          {files?.map((f) => (
            <li
              key={f.path}
              role="option"
              aria-selected={f.path === selectedFile?.path}
              className={f.path === selectedFile?.path ? "file-row selected" : "file-row"}
              title={f.oldPath ? `${f.oldPath} → ${f.path}` : f.path}
              onClick={() => selectFile(f)}
            >
              <span className="file-path">{f.path}</span>
              <FileStatusIcon kind={f.kind} />
            </li>
          ))}
        </ul>
        <div className="detail">
          <DiffPane
            source={
              selectedFile
                ? {
                    key: `commit:${repoId}:${commit.sha}:${selectedFile.path}`,
                    title: selectedFile.path,
                    fetch: () =>
                      getCommitDiff(repoId, commit.sha, selectedFile.path, selectedFile.oldPath),
                  }
                : null
            }
            empty="Select a file to see its changes."
          />
        </div>
      </div>
    </div>
  );
}
