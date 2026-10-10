import { describe, expect, it } from "vitest";
import { fill, renderGuide, renderToc, rewriteHref, slug } from "./build-site.mjs";

describe("build-site", () => {
  it("makes GitHub-style heading anchors", () => {
    expect(slug("Get a copy of a repository (clone)")).toBe("get-a-copy-of-a-repository-clone");
    expect(slug("Large files (Git LFS)")).toBe("large-files-git-lfs");
    expect(slug("SSH keys")).toBe("ssh-keys");
  });

  it("points links at the website or the repository", () => {
    expect(rewriteHref("#ssh-keys")).toBe("#ssh-keys");
    expect(rewriteHref("https://git-lfs.com")).toBe("https://git-lfs.com");
    expect(rewriteHref("../README.md")).toBe(
      "https://github.com/kovirlabs/tenajlo/blob/main/README.md",
    );
    expect(rewriteHref("releasing.md#update-signing")).toBe(
      "https://github.com/kovirlabs/tenajlo/blob/main/docs/releasing.md#update-signing",
    );
    expect(rewriteHref("user-guide.md#install")).toBe("#install");
  });

  it("fills each template marker exactly once", () => {
    expect(fill("<a>{{toc}}</a>{{content}}", { toc: "T", content: "$& C" })).toBe("<a>T</a>$& C");
    expect(() => fill("<!-- {{toc}} -->{{toc}}", { toc: "T" })).toThrow(/2 \{\{toc\}\}/);
    expect(() => fill("nothing", { toc: "T" })).toThrow(/0/);
  });

  it("collects in-page links and outlines h2 and h3", () => {
    const { html, headings, anchors } = renderGuide(
      "# Guide\n\n## One (a)\n\nSee [two](#two) and [missing](#nope).\n\n### Sub\n\n## Two\n\n## Two\n",
    );
    expect(headings.map((h) => h.id)).toEqual(["one-a", "sub", "two", "two-1"]);
    expect(anchors).toEqual(["two", "nope"]);
    expect(html).toContain('<h2 id="one-a">');
    const toc = renderToc(headings);
    expect(toc).toContain('<li><a href="#one-a">One (a)</a><ol><li><a href="#sub">Sub</a></li>');
    expect(toc).not.toContain("<ol></ol>");
  });
});
