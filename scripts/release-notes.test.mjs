import { describe, expect, it } from "vitest";
import { changelogSection, releaseNotes } from "./release-notes.mjs";

const CHANGELOG = `# Changelog

## [Unreleased]

- Not yet.

## [1.4.1] - 2026-11-07

### Fixed

- A fix.

## [1.4.0] - 2026-11-05

### Added

- A feature.
`;

describe("release-notes", () => {
  it("takes one version's section", () => {
    expect(changelogSection(CHANGELOG, "1.4.1")).toBe("### Fixed\n\n- A fix.");
    expect(changelogSection(CHANGELOG, "1.4.0")).toBe("### Added\n\n- A feature.");
    expect(() => changelogSection(CHANGELOG, "9.9.9")).toThrow(/no "## \[9\.9\.9\]"/);
  });

  it("links the version's installers and includes its changes", () => {
    const notes = releaseNotes(CHANGELOG, "1.4.1");
    expect(notes).toContain("/releases/download/v1.4.1/Tenajlo_1.4.1_x64-setup.exe");
    expect(notes).toContain("## What's new in 1.4.1\n\n### Fixed\n\n- A fix.");
    expect(notes).not.toContain("A feature.");
    expect(() => releaseNotes(CHANGELOG, "v1.4.1")).toThrow(/not a release version/);
  });
});
