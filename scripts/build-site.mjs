// Builds the website: copies site/ and renders docs/user-guide.md into guide/index.html with
// scripts/guide-template.html. The guide stays Markdown in the repo (reviewed with the code,
// tagged with each release); this only publishes it. The Pages workflow runs it on main.
//
//   node scripts/build-site.mjs [out-dir]    (default: target/site)
//
// Fails if an in-page link in the guide points to a heading that doesn't exist.
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, posix, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { Marked } from "marked";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = resolve(process.argv[2] ?? join(root, "target", "site"));
const GUIDE = "docs/user-guide.md";
const REPO_BLOB = "https://github.com/kovirlabs/tenajlo/blob/main/";

/** GitHub's heading anchors: lowercase, punctuation dropped, spaces to hyphens. */
export function slug(text) {
  return text
    .toLowerCase()
    .trim()
    .replace(/<[^>]+>/g, "")
    .replace(/[^\p{L}\p{N}\s_-]/gu, "")
    .replace(/\s/g, "-");
}

/** Where a link in docs/user-guide.md should point on the website. */
export function rewriteHref(href) {
  if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith("#")) return href;
  const [path, hash] = href.split("#");
  const target = posix.normalize(posix.join(posix.dirname(GUIDE), path ?? ""));
  if (target === GUIDE) return hash ? `#${hash}` : "./";
  return REPO_BLOB + target + (hash ? `#${hash}` : "");
}

function escapeHtml(s) {
  return s.replace(
    /[&<>"]/g,
    (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c],
  );
}

/** Renders the guide; returns the HTML, its h2/h3 outline, and every in-page link. */
export function renderGuide(markdown) {
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
        const to = rewriteHref(href);
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
        throw new Error(`images aren't supported in the guide yet: ${href}`);
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

function build() {
  const markdown = readFileSync(join(root, GUIDE), "utf8");
  const { html, headings, anchors } = renderGuide(markdown);
  // Every heading's id, not only the h2/h3 shown in the contents.
  const known = new Set([...html.matchAll(/ id="([^"]+)"/g)].map((m) => m[1]));
  const broken = anchors.filter((a) => !known.has(a));
  if (broken.length > 0) {
    throw new Error(`${GUIDE} links to missing headings: ${broken.map((a) => `#${a}`).join(", ")}`);
  }

  const template = readFileSync(join(root, "scripts", "guide-template.html"), "utf8");
  const page = fill(template, { toc: renderToc(headings), content: html });

  rmSync(out, { recursive: true, force: true });
  cpSync(join(root, "site"), out, { recursive: true });
  mkdirSync(join(out, "guide"), { recursive: true });
  writeFileSync(join(out, "guide", "index.html"), page);
  console.log(`site: built ${relative(root, out) || out} (guide: ${headings.length} sections)`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) build();
