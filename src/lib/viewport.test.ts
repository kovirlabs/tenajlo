import { describe, expect, it } from "vitest";
import { clampShift } from "./viewport";

describe("clampShift", () => {
  it("leaves a box that fits alone", () => {
    expect(clampShift({ left: 100, top: 50, width: 200, height: 100 }, 800, 600)).toEqual({
      dx: 0,
      dy: 0,
    });
  });

  it("pulls a box back from the right and bottom edges", () => {
    expect(clampShift({ left: 700, top: 550, width: 200, height: 100 }, 800, 600)).toEqual({
      dx: -108,
      dy: -58,
    });
  });

  it("pushes a box off the left and top edges", () => {
    expect(clampShift({ left: -40, top: 2, width: 200, height: 100 }, 800, 600)).toEqual({
      dx: 48,
      dy: 6,
    });
  });

  it("keeps the start edge visible when the box is larger than the window", () => {
    expect(clampShift({ left: 0, top: 0, width: 1000, height: 900 }, 800, 600)).toEqual({
      dx: 8,
      dy: 8,
    });
  });
});
