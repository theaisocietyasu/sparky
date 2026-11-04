import { describe, expect, it } from "vitest";
import { drainFrames } from "./sse";

describe("drainFrames", () => {
  it("keeps a frame split across reads until it is complete", () => {
    const first = drainFrames('event: progress\ndata: {"text":"a"');
    expect(first.frames).toEqual([]);
    const second = drainFrames(`${first.rest}}\n\nevent: answer\ndata: {}\n\nevent: do`);
    expect(second.frames).toEqual([
      { name: "progress", data: '{"text":"a"}' },
      { name: "answer", data: "{}" },
    ]);
    expect(second.rest).toBe("event: do");
  });

  it("joins multi-line data and skips frames with no data", () => {
    const { frames } = drainFrames("event: error\ndata: one\ndata: two\n\n: keepalive\n\n");
    expect(frames).toEqual([{ name: "error", data: "one\ntwo" }]);
  });

  it("reads CRLF line endings", () => {
    const { frames } = drainFrames("event: done\r\ndata: {}\r\n\r\n");
    expect(frames).toEqual([{ name: "done", data: "{}" }]);
  });
});
