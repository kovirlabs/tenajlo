import { useId, useState } from "react";
import { getBinaryPreview } from "../api/diff";
import { getCommitDiff } from "../api/history";
import { FilePath, diffTitle } from "../components/FilePath";
import { FileStatusIcon } from "../components/FileStatusIcon";
import { CommitChecksLine } from "../components/forgejo/CommitChecksLine";
import { listKeyHandler } from "../hooks/listKeys";
import { copyText } from "../lib/clipboard";
import { useHistoryStore } from "../stores/historyStore";
import { DiffPane } from "./DiffPane";

export function CommitDetail({ repoId }: { repoId: string }) {
  const commit = useHistoryStore((s) => s.commits.find((c) => c.sha === s.selectedSha));
  const files = useHistoryStore((s) => s.files);
  const selectedFile = useHistoryStore((s) => s.selectedFile);
  const selectFile = useHistoryStore((s) => s.selectFile);
  const idPrefix = useId();
  const [copied, setCopied] = useState<string | null>(null);

  if (!commit) return <p className="pane-message muted">Select a commit to see its changes.</p>;

  const active = files?.findIndex((f) => f.path === selectedFile?.path) ?? -1;
  const onKeyDown = listKeyHandler({
    count: files?.length ?? 0,
    active,
    onMove: (i) => {
      const f = files?.[i];
      if (f) {
        selectFile(f);
        document.getElementById(`${idPrefix}-${i}`)?.scrollIntoView({ block: "nearest" });
      }
    },
  });

  const copy = async () => {
    if (await copyText(commit.sha)) {
      setCopied(commit.sha);
      window.setTimeout(() => setCopied(null), 1500);
    }
  };

  return (
    <div className="commit-detail">
      <header className="commit-header">
        <h2>{commit.summary || "(no message)"}</h2>
        {commit.body && <p className="commit-body">{commit.body}</p>}
        <p className="muted">
          {commit.authorName} committed {new Date(commit.authorDate).toLocaleString()} ·{" "}
          <code title={commit.sha}>{commit.shortSha}</code>{" "}
          <button
            type="button"
            className="link"
            title="Copy the full commit ID"
            onClick={() => void copy()}
          >
            {copied === commit.sha ? "Copied" : "Copy ID"}
          </button>
        </p>
        <CommitChecksLine repoId={repoId} commit={commit} />
      </header>
      <div className="commit-split">
        <ul
          role="listbox"
          aria-label="Files in commit"
          className="file-list commit-files"
          tabIndex={files?.length ? 0 : undefined}
          aria-activedescendant={active >= 0 ? `${idPrefix}-${active}` : undefined}
          onKeyDown={onKeyDown}
        >
          {files === null && <li className="pane-message muted">Loading…</li>}
          {files?.length === 0 && <li className="pane-message muted">No file changes.</li>}
          {files?.map((f, i) => (
            <li
              key={f.path}
              id={`${idPrefix}-${i}`}
              role="option"
              aria-selected={f.path === selectedFile?.path}
              className={f.path === selectedFile?.path ? "file-row selected" : "file-row"}
              title={diffTitle(f.path, f.oldPath)}
              onClick={() => selectFile(f)}
            >
              <FilePath path={f.path} />
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
                    title: diffTitle(selectedFile.path, selectedFile.oldPath),
                    renamedFrom: selectedFile.oldPath,
                    fetch: () =>
                      getCommitDiff(repoId, commit.sha, selectedFile.path, selectedFile.oldPath),
                    preview: {
                      path: selectedFile.path,
                      fetch: () =>
                        getBinaryPreview(repoId, {
                          type: "Commit",
                          sha: commit.sha,
                          path: selectedFile.path,
                          oldPath: selectedFile.oldPath,
                        }),
                    },
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
