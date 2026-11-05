import { describe as group, expect, it } from "vitest";
import { describe, figures, kindName, ms, status, summary } from "./format";

group("admin format", () => {
  it("counts answered share, median time, tokens and people over finished traces", () => {
    const base = { conversation_id: "c", at: "", input: "", steps: 1, tokens: 10 };
    const f = figures([
      { ...base, request_id: "a", user_id: "1", status: "answered", duration_ms: 100 },
      { ...base, request_id: "b", user_id: "2", status: "error", duration_ms: 300 },
      { ...base, request_id: "c", user_id: "1", status: null, duration_ms: 0 },
    ]);
    expect(f).toEqual({ requests: 3, median: 100, tokens: 30, answeredShare: 50, users: 2 });
    expect(figures([])).toMatchObject({ median: null, answeredShare: null });
  });

  it("names statuses, kinds, durations and changes", () => {
    expect(status("answered")).toEqual(["good", "Answered"]);
    expect(status(null)).toEqual(["plain", "Running"]);
    expect(status("new_kind")).toEqual(["plain", "new_kind"]);
    expect(kindName("tool_call")).toBe("Tool call");
    expect(ms(950)).toBe("950 ms");
    expect(ms(2500)).toBe("2.5 s");
    expect(describe({ id: 1, kind: "tool", key: "x", value: false, actor: "a", created_at: "" })).toBe("x off");
    expect(summary({ at: "t", kind: "tool_call", name: "search", args: { q: 1 }, ok: true })).toBe("name: search, ok: true");
  });
});
