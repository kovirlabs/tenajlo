/** Gap kept between floating panels and the window edge, in px. */
export const VIEWPORT_MARGIN = 8;

type Rect = { left: number; top: number; width: number; height: number };

/**
 * How far to shift a floating box so it stays inside the window.
 * Prefers keeping the left/top edge visible when the box is larger than the window.
 */
export function clampShift(
  rect: Rect,
  viewportWidth: number,
  viewportHeight: number,
  margin = VIEWPORT_MARGIN,
): { dx: number; dy: number } {
  return {
    dx: axisShift(rect.left, rect.width, viewportWidth, margin),
    dy: axisShift(rect.top, rect.height, viewportHeight, margin),
  };
}

function axisShift(start: number, size: number, viewport: number, margin: number): number {
  const overflowEnd = start + size - (viewport - margin);
  let shift = overflowEnd > 0 ? -overflowEnd : 0;
  if (start + shift < margin) shift = margin - start;
  return shift;
}
