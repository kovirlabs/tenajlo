/** True in the Windows app (WebView2), where some things have different names. */
export function isWindows(): boolean {
  return typeof navigator !== "undefined" && navigator.userAgent.includes("Windows");
}

/** What the OS calls the place discarded files go: "Recycle Bin" on Windows, else "Trash". */
export function trashName(): string {
  return isWindows() ? "Recycle Bin" : "Trash";
}
