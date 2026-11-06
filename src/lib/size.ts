const UNITS = ["bytes", "KB", "MB", "GB", "TB"];

/** "812 bytes", "4.0 KB", "12.3 MB" (1 KB = 1024 bytes, like Windows Explorer). */
export function formatSize(bytes: number): string {
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  if (unit === 0) return `${bytes} ${bytes === 1 ? "byte" : "bytes"}`;
  return `${value.toFixed(value < 10 ? 1 : 0)} ${UNITS[unit]}`;
}

/** "+2.0 KB", "−512 bytes", or "" when equal. */
export function formatSizeChange(before: number, after: number): string {
  if (before === after) return "";
  const sign = after > before ? "+" : "−";
  return `${sign}${formatSize(Math.abs(after - before))}`;
}
