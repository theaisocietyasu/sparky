import { describe, expect, it } from "vitest";
import { parseStep } from "./step";

describe("parseStep", () => {
  it("reads the engine mark as a kind and keeps the words", () => {
    expect(parseStep("\u{1f9e0} remembering 1 thing about you")).toEqual({ kind: "memory", text: "remembering 1 thing about you" });
    expect(parseStep("\u{2705} `search_live` (query: x) \u{2192} 3 results")).toEqual({ kind: "done", text: "`search_live` (query: x) \u{2192} 3 results" });
    expect(parseStep("\u{1f5dc}\u{fe0f} summarising 4 earlier messages")).toEqual({ kind: "summary", text: "summarising 4 earlier messages" });
  });

  it("treats a line with no mark, or an unknown one, as a plain step", () => {
    expect(parseStep("searching")).toEqual({ kind: "step", text: "searching" });
    expect(parseStep("\u{1f680} launching")).toEqual({ kind: "step", text: "launching" });
  });
});
