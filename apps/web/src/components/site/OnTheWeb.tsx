import { ArrowRight } from "lucide-react";
import { motion, useReducedMotion } from "motion/react";
import { Link } from "react-router-dom";
import Heading from "./Heading";
import { CHAT } from "./content";

const POINTS = [
  "Sign in with the Discord account you use in the server.",
  "Answers stream in with tables, code and the sources they used.",
  "See and forget what Sparky remembers about you.",
];

/** The web chat: the same Sparky in a browser tab, shown in a window frame. */
const OnTheWeb = () => {
  const still = useReducedMotion() ?? false;
  return (
    <section id="web-chat" aria-label="Web chat" className="relative z-10 bg-paper py-20">
      <div className="mx-auto max-w-6xl px-5 sm:px-8">
        <div className="grid items-end gap-8 lg:grid-cols-[1fr_auto]">
          <div>
            <Heading title="Or just open a tab." />
            <ul className="mt-6 space-y-2 text-ink-soft">
              {POINTS.map((p) => (
                <li key={p} className="flex gap-3">
                  <span aria-hidden className="mt-2.5 h-1.5 w-1.5 shrink-0 rounded-full bg-shu" />
                  {p}
                </li>
              ))}
            </ul>
          </div>
          <Link
            to={CHAT}
            className="group inline-flex h-12 items-center gap-2 self-end rounded-full bg-ink px-7 text-sm font-semibold text-paper transition-transform hover:-translate-y-0.5"
          >
            Open the chat
            <ArrowRight className="h-4 w-4 transition-transform group-hover:translate-x-0.5" aria-hidden />
          </Link>
        </div>

        <motion.figure
          initial={still ? false : { opacity: 0, y: 32 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true, margin: "-10% 0px" }}
          transition={{ duration: 0.8, ease: [0.22, 1, 0.36, 1] }}
          className="mt-12 overflow-hidden rounded-2xl bg-white shadow-[0_40px_80px_-40px_rgba(20,18,16,0.6)] ring-1 ring-ink/10"
        >
          <div aria-hidden className="flex h-10 items-center gap-2 border-b border-ink/10 bg-[#f4f2ee] px-4">
            <span className="h-3 w-3 rounded-full bg-[#ff5f57]" />
            <span className="h-3 w-3 rounded-full bg-[#febc2e]" />
            <span className="h-3 w-3 rounded-full bg-[#28c840]" />
            <span className="mx-auto rounded-md bg-white px-12 py-1 text-xs text-ink-soft ring-1 ring-ink/10">sparky.ais-asu.com/chat</span>
          </div>
          <img
            src="/demo/sparky-web.webp"
            width={2560}
            height={1600}
            loading="lazy"
            decoding="async"
            alt="The Sparky web chat answering what is due this week with a table of deadlines and links to its sources"
            className="block h-auto w-full"
          />
        </motion.figure>
      </div>
    </section>
  );
};

export default OnTheWeb;
