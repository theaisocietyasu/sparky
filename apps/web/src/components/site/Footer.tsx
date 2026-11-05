import GithubMark from "./GithubMark";
import { Link } from "react-router-dom";
import { CHAT, REPO, ROADMAP } from "./content";

/** Bottom line: who made it, what it is not, and where the source is. */
const Footer = () => (
  <footer className="border-t border-ink/10 px-5 py-8 sm:px-8">
    <div className="mx-auto flex max-w-6xl flex-col items-center gap-4 text-xs text-ink-soft sm:flex-row sm:justify-between">
      <div className="flex items-center gap-2.5">
        <img src="/brand/sparkyai-logo.png" alt="" className="h-5 w-auto" />
        <span className="font-serif font-semibold text-ink">SparkyAI</span>
        <span>Unofficial student project. Not affiliated with Arizona State University.</span>
      </div>
      <div className="flex items-center gap-6">
        <Link to={CHAT} className="transition-colors hover:text-shu">
          Chat
        </Link>
        <a
          href={ROADMAP}
          target="_blank"
          rel="noreferrer"
          className="transition-colors hover:text-shu"
        >
          Roadmap
        </a>
        <a
          href={REPO}
          target="_blank"
          rel="noreferrer"
          className="inline-flex items-center gap-1.5 transition-colors hover:text-shu"
        >
          <GithubMark className="h-3.5 w-3.5" />
          GitHub
        </a>
      </div>
    </div>
  </footer>
);

export default Footer;
