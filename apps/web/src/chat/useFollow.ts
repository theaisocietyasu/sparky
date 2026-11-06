import { useCallback, useEffect, useRef } from "react";

/** How close to the bottom, in pixels, still counts as reading the newest text. */
const NEAR = 80;

/**
 * Keeps a scroller at its newest text while the reader is there. follow glides down a share of the gap
 * each frame instead of jumping; jump goes straight there. A wheel, touch or key from the reader stops
 * a glide, and scrolling up away from the bottom unpins it until they come back down.
 */
export const useFollow = (still: boolean) => {
  const ref = useRef<HTMLDivElement>(null);
  const pinned = useRef(true);
  const gliding = useRef(false);
  const frame = useRef(0);

  const stop = useCallback(() => {
    cancelAnimationFrame(frame.current);
    gliding.current = false;
  }, []);

  const jump = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    stop();
    el.scrollTop = el.scrollHeight;
  }, [stop]);

  const follow = useCallback(() => {
    if (!pinned.current) return;
    if (still) return jump();
    if (gliding.current) return;
    gliding.current = true;
    const step = () => {
      const el = ref.current;
      if (!el || !pinned.current) return stop();
      const gap = el.scrollHeight - el.clientHeight - el.scrollTop;
      if (gap <= 1) return stop();
      el.scrollTop += Math.max(1, gap * 0.16);
      frame.current = requestAnimationFrame(step);
    };
    frame.current = requestAnimationFrame(step);
  }, [still, jump, stop]);

  const onScroll = useCallback(() => {
    const el = ref.current;
    if (!el || gliding.current) return;
    pinned.current = el.scrollHeight - el.scrollTop - el.clientHeight < NEAR;
  }, []);

  /** Sticks to the bottom again, as after the reader sends a message. */
  const pin = useCallback(() => {
    pinned.current = true;
  }, []);

  useEffect(() => stop, [stop]);

  /** Props for the scroller element. */
  const bind = { ref, onScroll, onWheel: stop, onTouchMove: stop, onKeyDown: stop };
  return { bind, pin, follow, jump };
};
