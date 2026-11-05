import { describe, expect, it } from "vitest";
import { splitChoices } from "./choices";

describe("splitChoices", () => {
  it("takes the choices block out and keeps the question", () => {
    const raw = "Which campus are you on?\n\n```choices\nTempe\nPolytechnic\n- West\n```\n";
    expect(splitChoices(raw)).toEqual({ text: "Which campus are you on?", choices: ["Tempe", "Polytechnic", "West"] });
  });

  it("hides a block that is still streaming and caps the count", () => {
    expect(splitChoices("Pick one\n```choices\nTem")).toEqual({ text: "Pick one", choices: [] });
    const many = "Q\n```choices\na\nb\nc\nd\ne\n```";
    expect(splitChoices(many).choices).toEqual(["a", "b", "c", "d"]);
  });

  it("leaves other code blocks alone", () => {
    const raw = "Run this\n```python\nprint(1)\n```";
    expect(splitChoices(raw)).toEqual({ text: raw, choices: [] });
  });
});
