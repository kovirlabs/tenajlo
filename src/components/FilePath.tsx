import type { ReactNode } from "react";

/** A repository path in a file list. When space runs out the folder part is cut off first,
 *  so the file name stays readable. `children` follows the name (e.g. "(submodule)"). */
export function FilePath({ path, children }: { path: string; children?: ReactNode }) {
  const cut = path.lastIndexOf("/");
  const dir = path.slice(0, cut + 1);
  const name = path.slice(cut + 1);
  return (
    <span className="file-path">
      {dir && <span className="file-dir">{dir}</span>}
      <span className="file-name">{name}</span>
      {children}
    </span>
  );
}

/** Header text for a file's diff: "old → new" for a rename. */
export function diffTitle(path: string, oldPath: string | null | undefined): string {
  return oldPath ? `${oldPath} → ${path}` : path;
}
