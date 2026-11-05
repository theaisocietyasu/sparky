import { useEffect, useRef } from "react";
import { INNER, LINE, OUTER } from "./body";

/** What Sparky is doing: resting, working, pleased for a moment, dozing on an empty page, or sorry. */
export type Mood = "idle" | "thinking" | "done" | "sleepy" | "sad";

const REACH = 7;

/**
 * The Sparky dragon drawn as an S. It breathes and blinks at rest; while it works, a spark runs down
 * its gold line and the tail crackles. With watch set, its eye follows the pointer; with still set, it holds still.
 */
const Sparky = ({
  mood = "idle",
  watch = false,
  still = false,
  className = "",
}: {
  mood?: Mood;
  watch?: boolean;
  still?: boolean;
  className?: string;
}) => {
  const root = useRef<SVGSVGElement>(null);
  const pupil = useRef<SVGCircleElement>(null);

  useEffect(() => {
    const svg = root.current;
    const dot = pupil.current;
    if (!watch || !svg || !dot || window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) return;
    let frame = 0;
    const look = (e: PointerEvent) => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const box = svg.getBoundingClientRect();
        const eyeX = box.left + box.width * (566 / 720);
        const eyeY = box.top + box.height * (190 / 780);
        const dx = e.clientX - eyeX;
        const dy = e.clientY - eyeY;
        const far = Math.hypot(dx, dy) || 1;
        const pull = Math.min(1, far / 240);
        dot.setAttribute("transform", `translate(${((dx / far) * REACH * pull).toFixed(1)} ${((dy / far) * REACH * 0.6 * pull).toFixed(1)})`);
      });
    };
    window.addEventListener("pointermove", look);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("pointermove", look);
    };
  }, [watch]);

  return (
    <svg ref={root} viewBox="0 0 720 780" aria-hidden data-mood={mood} data-still={still || undefined} className={`sparky ${className}`}>
      <g className="sparky-body">
        <path d={OUTER} fill="var(--color-shu-deep)" />
        <path d={INNER} fill="var(--color-shu)" />
        <path d={LINE} fill="none" stroke="var(--color-kin)" strokeWidth="7" strokeLinecap="round" strokeLinejoin="round" />
        <path
          d={LINE}
          pathLength={1}
          className="sparky-current"
          fill="none"
          stroke="#fff4c2"
          strokeWidth="11"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <g strokeLinejoin="round">
          <path
            d="M430 130 C405 85 370 45 350 0 C395 30 440 75 470 118 Z M488 116 C482 75 488 40 505 8 C520 45 526 85 524 124 Z"
            fill="var(--color-shu-deep)"
          />
          <path
            d="M408 96 C478 80 566 112 616 182 L694 226 L700 262 L626 268 L656 300 L618 324 L546 304 C498 294 462 262 448 230 Z"
            fill="var(--color-shu)"
            stroke="var(--color-shu-deep)"
            strokeWidth="18"
          />
          <path
            d="M436 114 C490 106 558 134 604 194 L674 234 L678 252 L606 256"
            fill="none"
            stroke="var(--color-kin)"
            strokeWidth="6"
            strokeLinecap="round"
          />
          <g className="sparky-eye">
            <ellipse cx="560" cy="190" rx="25" ry="14" fill="var(--color-shu)" stroke="var(--color-kin)" strokeWidth="6" />
            <circle ref={pupil} cx="566" cy="190" r="7" fill="var(--color-kin)" />
          </g>
        </g>
      </g>
      <g className="sparky-spark" fill="none" stroke="var(--color-kin)" strokeWidth="22" strokeLinejoin="round" strokeLinecap="round">
        <path d="M150 600 L112 532 L98 592 L58 520 L46 584 L12 560" />
        <path d="M122 660 L66 642 L98 684" />
      </g>
    </svg>
  );
};

export default Sparky;
