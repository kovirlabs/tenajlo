/**
 * Copies text to the clipboard. Uses the async Clipboard API, and falls back to a hidden
 * textarea + `execCommand("copy")` where the WebView doesn't allow it. Resolves to whether it
 * worked. Must be called from a user action (click or key press).
 */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.appendChild(area);
    area.select();
    try {
      return document.execCommand("copy");
    } catch {
      return false;
    } finally {
      area.remove();
    }
  }
}
