// Builds the website: copies site/ and renders each Markdown page in PAGES (the user guide and
// the Forgejo setup guide) into <dir>/index.html with scripts/guide-template.html. The pages stay
// Markdown in the repo (reviewed with the code, tagged with each release); this only publishes
// them. The Pages workflow runs it on main.
//
//   node scripts/build-site.mjs [out-dir]    (default: target/site)
//
// Fails if an in-page link points to a heading that doesn't exist.
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, posix, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { Marked } from "marked";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = resolve(process.argv[2] ?? join(root, "target", "site"));
const SITE = "https://kovirlabs.github.io/tenajlo/";
const REPO_BLOB = "https://github.com/kovirlabs/tenajlo/blob/main/";

/**
 * The Markdown pages on the website. `dir` is the page's folder under the site root (one level
 * deep, so the template's `../` links reach the site root); `nav` is the main-menu link it
 * highlights.
 */
export const PAGES = [
  {
    src: "docs/user-guide.md",
    dir: "guide",
    nav: "guide",
    title: "User guide · Tenajlo",
    ogTitle: "Tenajlo user guide",
    description:
      "How to install Tenajlo, sign in to Forgejo, clone, commit, push and pull, handle conflicts, use Git LFS and SSH keys, and fix common problems.",
    ogDescription:
      "Install Tenajlo and use it with your Forgejo server: clone, commit, share changes, conflicts, large files and SSH keys.",
  },
  {
    src: "docs/forgejo-team-setup.md",
    dir: "forgejo-setup",
    nav: "setup",
    title: "Forgejo setup for engineering teams · Tenajlo",
    ogTitle: "Setting up Forgejo for an engineering team that won't use the CLI",
    description:
      "For Forgejo admins: server settings, reverse proxy limits, teams, Git LFS and file locks for CAD and PLC files, branch protection and onboarding for engineers who use Tenajlo instead of the command line.",
    ogDescription:
      "Server settings, Git LFS and file locks for CAD and PLC files, teams, branch protection and onboarding for engineers who won't use the command line.",
  },
];

/** GitHub's heading anchors: lowercase, punctuation dropped, spaces to hyphens. */
export function slug(text) {
  return text
    .toLowerCase()
    .trim()
    .replace(/<[^>]+>/g, "")
    .replace(/[^\p{L}\p{N}\s_-]/gu, "")
    .replace(/\s/g, "-");
}

/** Where a link in the Markdown page `from` (a repo path, e.g. docs/user-guide.md) should point. */
export function rewriteHref(href, from = PAGES[0].src) {
  if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith("#")) return href;
  const [path, hash] = href.split("#");
  const target = posix.normalize(posix.join(posix.dirname(from), path ?? ""));
  const suffix = hash ? `#${hash}` : "";
  if (target === from) return suffix || "./";
  const page = PAGES.find((p) => p.src === target);
  const fromDir = PAGES.find((p) => p.src === from)?.dir;
  if (page && fromDir) return `${posix.relative(fromDir, page.dir)}/${suffix}`;
  return REPO_BLOB + target + suffix;
}

function escapeHtml(s) {
  return s.replace(
    /[&<>"]/g,
    (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c],
  );
}

/** Renders a page; returns the HTML, its h2/h3 outline, and every in-page link. */
export function renderGuide(markdown, from = PAGES[0].src) {
  const headings = [];
  const used = new Map();
  const anchors = [];
  const marked = new Marked({
    gfm: true,
    renderer: {
      heading({ tokens, depth, text }) {
        const base = slug(text);
        const n = used.get(base) ?? 0;
        used.set(base, n + 1);
        const id = n === 0 ? base : `${base}-${n}`;
        const html = this.parser.parseInline(tokens);
        if (depth === 2 || depth === 3) headings.push({ depth, id, html });
        return `<h${depth} id="${id}"><a class="anchor" href="#${id}" aria-hidden="true">#</a>${html}</h${depth}>\n`;
      },
      link({ href, title, tokens }) {
        const to = rewriteHref(href, from);
        if (to.startsWith("#")) anchors.push(to.slice(1));
        const external = /^https?:/.test(to);
        const attrs = [
          `href="${escapeHtml(to)}"`,
          title ? `title="${escapeHtml(title)}"` : "",
          external ? 'rel="noopener"' : "",
        ].filter(Boolean);
        return `<a ${attrs.join(" ")}>${this.parser.parseInline(tokens)}</a>`;
      },
      image({ href }) {
        throw new Error(`images aren't supported in the site's Markdown pages yet: ${href}`);
      },
    },
  });
  const html = marked.parse(markdown);
  return { html, headings, anchors };
}

/** Nested list of the h2 sections, with their h3 subsections. */
export function renderToc(headings) {
  let html = "<ol>";
  let open = false;
  for (const h of headings) {
    const item = `<a href="#${h.id}">${h.html.replace(/<a [^>]*>|<\/a>/g, "")}</a>`;
    if (h.depth === 2) {
      html += `${open ? "</ol></li>" : ""}<li>${item}<ol>`;
      open = true;
    } else {
      html += `<li>${item}</li>`;
    }
  }
  html += open ? "</ol></li></ol>" : "</ol>";
  // Sections without subsections leave an empty list behind.
  return html.replace(/<ol><\/ol>/g, "");
}

/** Replaces each `{{name}}` in `template` exactly once; fails on a missing or extra one. */
export function fill(template, values) {
  let page = template;
  for (const [name, value] of Object.entries(values)) {
    const marker = `{{${name}}}`;
    const count = page.split(marker).length - 1;
    if (count !== 1) throw new Error(`guide template has ${count} ${marker} markers, expected 1`);
    // A function, so `$` in the guide isn't read as a replacement pattern.
    page = page.replace(marker, () => value);
  }
  return page;
}

/** The template's per-page values: metadata, menu highlight, contents and body. */
export function pageValues(page, toc, content) {
  const url = `${SITE}${page.dir}/`;
  const current = (nav) => (page.nav === nav ? ' aria-current="page"' : "");
  return {
    title: escapeHtml(page.title),
    description: escapeHtml(page.description),
    ogTitle: escapeHtml(page.ogTitle),
    ogDescription: escapeHtml(page.ogDescription),
    canonical: url,
    ogUrl: url,
    navGuide: current("guide"),
    navSetup: current("setup"),
    toc,
    content,
    source: `<a href="${REPO_BLOB}${page.src}">${page.src}</a>`,
  };
}

function renderPage(page, template) {
  const markdown = readFileSync(join(root, page.src), "utf8");
  const { html, headings, anchors } = renderGuide(markdown, page.src);
  // Every heading's id, not only the h2/h3 shown in the contents.
  const known = new Set([...html.matchAll(/ id="([^"]+)"/g)].map((m) => m[1]));
  const broken = anchors.filter((a) => !known.has(a));
  if (broken.length > 0) {
    throw new Error(
      `${page.src} links to missing headings: ${broken.map((a) => `#${a}`).join(", ")}`,
    );
  }
  const filled = fill(template, pageValues(page, renderToc(headings), html));
  return { filled, sections: headings.filter((h) => h.depth === 2).length };
}

function build() {
  const template = readFileSync(join(root, "scripts", "guide-template.html"), "utf8");
  // Render everything before touching the output, so a broken page leaves the old build alone.
  const rendered = PAGES.map((page) => ({ page, ...renderPage(page, template) }));

  rmSync(out, { recursive: true, force: true });
  cpSync(join(root, "site"), out, { recursive: true });
  for (const { page, filled } of rendered) {
    mkdirSync(join(out, page.dir), { recursive: true });
    writeFileSync(join(out, page.dir, "index.html"), filled);
  }
  const summary = rendered.map(({ page, sections }) => `${page.dir}: ${sections} sections`);
  console.log(`site: built ${relative(root, out) || out} (${summary.join(", ")})`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) build();
