import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { stepSize, useSmoothText } from "./useSmoothText";

describe("smooth text", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("reveals a few characters per step and catches up on a long backlog", () => {
    expect(stepSize(10)).toBe(3);
    expect(stepSize(1200)).toBe(100);
  });

  it("types toward the target from the given start and reports when done", () => {
    const { result } = renderHook(() => useSmoothText("abcdefghij", { from: 4 }));
    expect(result.current).toEqual({ text: "abcd", done: false });
    act(() => vi.advanceTimersByTime(40));
    expect(result.current.text).toBe("abcdefg");
    act(() => vi.advanceTimersByTime(200));
    expect(result.current).toEqual({ text: "abcdefghij", done: true });
  });

  it("shows everything at once when off", () => {
    const { result } = renderHook(() => useSmoothText("abc", { off: true }));
    expect(result.current).toEqual({ text: "abc", done: true });
  });

  it("keeps going when the target grows", () => {
    const { result, rerender } = renderHook(({ t }) => useSmoothText(t), { initialProps: { t: "abc" } });
    act(() => vi.advanceTimersByTime(100));
    expect(result.current.text).toBe("abc");
    rerender({ t: "abcdef" });
    act(() => vi.advanceTimersByTime(100));
    expect(result.current).toEqual({ text: "abcdef", done: true });
  });
});
