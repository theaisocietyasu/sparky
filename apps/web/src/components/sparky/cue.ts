import { useEffect, useState } from "react";
import type { Mood } from "./Sparky";

const EVENT = "sparky:cue";
const HOP_MS = 900;

/** Tells every listening Sparky that something good just happened, so it hops once. */
export const cue = () => window.dispatchEvent(new Event(EVENT));

/** A mood that turns to done for a moment on each cue, then back to rest. */
export const useCue = (rest: Mood = "idle"): Mood => {
  const [mood, setMood] = useState<Mood>(rest);
  useEffect(() => {
    let timer = 0;
    const hop = () => {
      window.clearTimeout(timer);
      setMood("done");
      timer = window.setTimeout(() => setMood(rest), HOP_MS);
    };
    window.addEventListener(EVENT, hop);
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener(EVENT, hop);
    };
  }, [rest]);
  return mood;
};

/** done for a moment once is turns true, then rest. */
export const useHopWhen = (is: boolean, rest: Mood = "idle"): Mood => {
  const [over, setOver] = useState(false);
  useEffect(() => {
    if (!is) return;
    const timer = window.setTimeout(() => setOver(true), HOP_MS);
    return () => window.clearTimeout(timer);
  }, [is]);
  return is && !over ? "done" : rest;
};
