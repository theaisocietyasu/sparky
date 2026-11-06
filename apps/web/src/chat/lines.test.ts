import { describe, expect, it } from "vitest";
import { applyProgress } from "./lines";

const empty = { lines: [], draft: "", held: "", next: 0 };

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

  it("holds a withdrawn draft on screen until the next draft replaces it", () => {
    let s = applyProgress(empty, { text: "I think it is sunny", draft: true });
    s = applyProgress(s, { text: "", slot: "answer", clear: true, draft: true });
    expect(s.draft).toBe("");
    expect(s.held).toBe("I think it is sunny");
    s = applyProgress(s, { text: "Sunny and 78", draft: true });
    expect(s.draft).toBe("Sunny and 78");
    expect(s.held).toBe("");
  });
});
