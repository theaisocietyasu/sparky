import { describe, expect, it } from "vitest";
import { applyProgress } from "./lines";

const empty = { lines: [], draft: "", next: 0 };

describe("applyProgress", () => {
  it("replaces a line by slot, removes it on clear, and appends lines with no slot", () => {
    let s = applyProgress(empty, { text: "searching", slot: "t1" });
    s = applyProgress(s, { text: "thinking" });
    s = applyProgress(s, { text: "searched: 3 results", slot: "t1" });
    expect(s.lines.map((l) => l.text)).toEqual(["searched: 3 results", "thinking"]);
    s = applyProgress(s, { text: "", slot: "t1", clear: true });
    expect(s.lines.map((l) => l.text)).toEqual(["thinking"]);
  });

  it("keeps a draft answer apart from the steps", () => {
    const s = applyProgress(empty, { text: "The library opens", draft: true });
    expect(s.draft).toBe("The library opens");
    expect(s.lines).toEqual([]);
  });
});
