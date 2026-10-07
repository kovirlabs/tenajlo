// Invented demo data for screenshots (scripts/screenshots.mjs). No real people, companies or
// servers: everything lives on git.example.com.
import type {
  Account,
  BranchList,
  Commit,
  CommitFile,
  FileDiff,
  OperationState,
  RemoteRepository,
  Repository,
  Settings,
  SyncState,
  WorkingDirectoryStatus,
} from "../src/bindings";

const HOUR = 3600_000;
const ago = (ms: number) => new Date(Date.now() - ms).toISOString();

export const repository: Repository = {
  id: "7d0c1c5e-2f4a-4b8e-9a51-3c2b9d6f0a11",
  name: "pump-station-controls",
  path: "C:\\Users\\sam\\Documents\\Tenajlo\\pump-station-controls",
  missing: false,
};

export const otherRepositories: Repository[] = [
  {
    id: "a1f3e2d4-0000-4000-8000-000000000002",
    name: "conveyor-line-3",
    path: "C:\\Users\\sam\\Documents\\Tenajlo\\conveyor-line-3",
    missing: false,
  },
  {
    id: "a1f3e2d4-0000-4000-8000-000000000003",
    name: "hmi-screens",
    path: "C:\\Users\\sam\\Documents\\Tenajlo\\hmi-screens",
    missing: false,
  },
];

export const account: Account = {
  id: "5b2e9c1d-7a3f-4e6b-8c0d-1f2a3b4c5d6e",
  kind: "forgejo",
  baseUrl: "https://git.example.com",
  login: "sam",
  displayName: "Sam Rivera",
  avatarUrl: null,
  needsSignIn: false,
};

export const settings: Settings = {
  theme: "Light",
  defaultCloneFolder: null,
  pullStrategy: "FastForwardOnly",
  backgroundFetchMinutes: 5,
  gitPath: null,
  editor: { kind: "VsCode" },
  welcomeCompleted: true,
};

export const status: WorkingDirectoryStatus = {
  branch: {
    name: "feature/low-pressure-alarm",
    tip: "c41f9a2e8b7d6c5a4f3e2d1c0b9a8f7e6d5c4b3a",
    upstream: "origin/feature/low-pressure-alarm",
    ahead: 2,
    behind: 0,
    upstreamGone: false,
  },
  files: [
    {
      path: "plc/PumpControl.st",
      oldPath: null,
      kind: "Modified",
      staged: "Full",
      submodule: false,
    },
    { path: "plc/Alarms.st", oldPath: null, kind: "Added", staged: "Full", submodule: false },
    {
      path: "cad/impeller-housing.SLDPRT",
      oldPath: null,
      kind: "Modified",
      staged: "Full",
      submodule: false,
    },
    {
      path: "docs/commissioning-checklist.md",
      oldPath: null,
      kind: "Modified",
      staged: "None",
      submodule: false,
    },
    {
      path: "tools/export_tags.py",
      oldPath: null,
      kind: "Untracked",
      staged: "None",
      submodule: false,
    },
  ],
  hasConflicts: false,
};

const line = (
  kind: "Context" | "Add" | "Delete",
  text: string,
  o: number | null,
  n: number | null,
) => ({
  kind,
  text,
  oldLine: o,
  newLine: n,
  noNewline: false,
});

export const pumpControlDiff: FileDiff = {
  type: "Text",
  hunks: [
    {
      header: "@@ -12,14 +12,19 @@ FUNCTION_BLOCK FB_PumpControl",
      lines: [
        line("Context", "VAR", 12, 12),
        line("Context", "    tStartDelay   : TON;", 13, 13),
        line("Context", "    tRunTimeout   : TON;", 14, 14),
        line("Add", "    tLowPressure  : TON;", null, 15),
        line("Add", "    bLowPressure  : BOOL;", null, 16),
        line("Context", "END_VAR", 15, 17),
        line("Context", "", 16, 18),
        line("Delete", "tStartDelay(IN := bStartRequest, PT := T#3S);", 17, null),
        line("Add", "tStartDelay(IN := bStartRequest, PT := T#5S);", null, 19),
        line("Context", "", 18, 20),
        line("Context", "IF tStartDelay.Q AND NOT bFault THEN", 19, 21),
        line("Context", "    bPumpRun := TRUE;", 20, 22),
        line("Context", "END_IF;", 21, 23),
        line("Add", "", null, 24),
        line(
          "Add",
          "(* Stop the pump if suction pressure stays below 0.8 bar for 10 s *)",
          null,
          25,
        ),
        line(
          "Add",
          "tLowPressure(IN := rSuctionPressure < 0.8 AND bPumpRun, PT := T#10S);",
          null,
          26,
        ),
        line("Add", "bLowPressure := tLowPressure.Q;", null, 27),
        line("Context", "", 22, 28),
        line("Delete", "IF bStopRequest OR bFault THEN", 23, null),
        line("Add", "IF bStopRequest OR bFault OR bLowPressure THEN", null, 29),
        line("Context", "    bPumpRun := FALSE;", 24, 30),
        line("Context", "END_IF;", 25, 31),
      ],
    },
  ],
};

const commit = (n: number, summary: string, author: string, hoursAgo: number): Commit => {
  // Deterministic, realistic-looking 40-hex-digit IDs.
  let x = n * 2654435761;
  const sha = Array.from({ length: 40 }, () => {
    x = (x * 1103515245 + 12345) % 2147483648;
    return (x % 16).toString(16);
  }).join("");
  return {
    sha,
    shortSha: sha.slice(0, 7),
    parents: [],
    authorName: author,
    authorEmail: `${author.split(" ")[0]?.toLowerCase()}@example.com`,
    authorDate: ago(hoursAgo * HOUR),
    summary,
    body: "",
  };
};

export const history: Commit[] = [
  commit(1, "Add low suction pressure alarm", "Sam Rivera", 2),
  commit(2, "Lengthen pump start delay to 5 s", "Sam Rivera", 3),
  commit(3, "Update impeller housing for new seal", "Priya Nair", 26),
  commit(4, "Commissioning checklist: add valve checks", "Jordan Lee", 50),
  commit(5, "Merge branch 'main' into feature/low-pressure-alarm", "Sam Rivera", 74),
  commit(6, "Export HMI tag list as CSV", "Priya Nair", 98),
  commit(7, "Fix flow totalizer rollover", "Jordan Lee", 170),
  commit(8, "Rename pump tags to match P&ID", "Sam Rivera", 220),
  commit(9, "Add VFD speed reference scaling", "Jordan Lee", 300),
  commit(10, "Initial PLC project export", "Priya Nair", 500),
];

export const commitFiles: CommitFile[] = [
  { path: "plc/PumpControl.st", oldPath: null, kind: "Modified" },
  { path: "plc/Alarms.st", oldPath: null, kind: "Added" },
];

export const branches: BranchList = {
  current: "feature/low-pressure-alarm",
  local: [
    {
      name: "feature/low-pressure-alarm",
      kind: "Local",
      remote: null,
      upstream: "origin/feature/low-pressure-alarm",
      tip: status.branch.tip ?? "",
      isCurrent: true,
      lastCommitDate: ago(2 * HOUR),
    },
    {
      name: "main",
      kind: "Local",
      remote: null,
      upstream: "origin/main",
      tip: "aa".repeat(20),
      isCurrent: false,
      lastCommitDate: ago(26 * HOUR),
    },
  ],
  remoteOnly: [],
};

export const syncState: SyncState = {
  action: { type: "Push", remote: "origin" },
  branch: "feature/low-pressure-alarm",
  ahead: 2,
  behind: 0,
  lastFetched: Math.floor((Date.now() - 4 * 60_000) / 1000),
};

export const noOperation: OperationState = { operation: null, conflicts: [], mergeSummary: null };

export const conflictStatus: WorkingDirectoryStatus = {
  branch: { ...status.branch, ahead: 3, behind: 0 },
  files: [
    {
      path: "plc/PumpControl.st",
      oldPath: null,
      kind: "Conflicted",
      staged: "None",
      submodule: false,
    },
    { path: "plc/Alarms.st", oldPath: null, kind: "Modified", staged: "Full", submodule: false },
  ],
  hasConflicts: true,
};

export const conflictOperation: OperationState = {
  operation: "Merge",
  conflicts: [{ path: "plc/PumpControl.st", markers: 2 }],
  mergeSummary:
    "Merge branch 'feature/low-pressure-alarm' of https://git.example.com/acme/pump-station-controls",
};

const remote = (
  owner: string,
  name: string,
  description: string,
  isPrivate = true,
): RemoteRepository => ({
  fullName: `${owner}/${name}`,
  owner,
  name,
  description,
  private: isPrivate,
  archived: false,
  cloneUrl: `https://git.example.com/${owner}/${name}.git`,
  sshUrl: `ssh://git@git.example.com:2222/${owner}/${name}.git`,
});

export const remoteRepositories: RemoteRepository[] = [
  remote("acme", "conveyor-line-3", "Conveyor line 3 PLC and HMI"),
  remote("acme", "hmi-screens", "Operator screens for all lines"),
  remote("acme", "packaging-cell", "Robot cell programs and layout drawings"),
  remote("acme", "pump-station-controls", "Pump station PLC logic, CAD and commissioning docs"),
  remote("acme", "standards", "Tag naming and coding standards", false),
  remote("sam", "notes", "Personal notes and scripts"),
];
