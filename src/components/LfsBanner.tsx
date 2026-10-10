import { useState } from "react";
import { getLfsStatus } from "../api/status";
import { useChangesStore } from "../stores/changesStore";
import { useAsyncEffect } from "../hooks/useAsyncEffect";

/** Warns when a repository stores large files with Git LFS but git-lfs is missing. */
export function LfsBanner({ repoId }: { repoId: string }) {
  const [missing, setMissing] = useState(false);
  // Re-check when the branch tip moves (a pull or switch can add .gitattributes).
  const tip = useChangesStore((s) => (s.repoId === repoId ? s.status?.branch.tip : undefined));

  useAsyncEffect(
    async (live) => {
      const res = await getLfsStatus(repoId);
      if (live()) setMissing(res.status === "ok" && res.data.used && !res.data.installed);
    },
    // `tip` re-checks after commits and pulls, which may add .gitattributes.
    [repoId, tip],
  );

  if (!missing) return null;
  return (
    <section className="conflict-banner lfs-banner" role="alert">
      <strong>Git LFS isn't installed.</strong> This repository keeps large files (like CAD or PLC
      archives) in Git LFS. Install it from git-lfs.com and restart Tenajlo; until then, committing
      and pushing are turned off so those files aren't stored the wrong way.
    </section>
  );
}
