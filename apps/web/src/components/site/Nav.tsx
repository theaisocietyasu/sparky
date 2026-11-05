import { Link } from "react-router-dom";
import GithubMark from "./GithubMark";
import { CHAT, REPO } from "./content";

const LINKS = [
  { href: "#in-action", label: "In action" },
  { href: "#web-chat", label: "Web chat" },
  { href: "#sources", label: "Sources" },
  { href: "#how-it-works", label: "How it works" },
  { href: "#open-source", label: "Open source" },
];

/** Top bar: mark, section links, and the repository. */
const Nav = () => (
  <header className="fixed inset-x-0 top-0 z-30 border-b border-ink/10 bg-paper/85 backdrop-blur-md">
    <nav
      aria-label="Primary navigation"
      className="mx-auto flex h-16 max-w-6xl items-center justify-between px-5 sm:px-8"
    >
      <Link to="/" className="flex items-center gap-2.5" aria-label="SparkyAI home">
        <img src="/brand/sparkyai-logo.png" alt="" className="h-8 w-auto" />
        <span className="font-serif text-lg font-semibold tracking-tight">SparkyAI</span>
      </Link>

      <div className="flex items-center gap-3 md:gap-7">
        <ul className="hidden items-center gap-7 md:flex">
          {LINKS.map((link) => (
            <li key={link.href}>
              <a
                href={link.href}
                className="text-sm text-ink-soft transition-colors hover:text-shu"
              >
                {link.label}
              </a>
            </li>
          ))}
        </ul>
        <Link
          to={CHAT}
          className="inline-flex h-9 items-center rounded-full bg-shu px-4 text-sm font-semibold text-paper transition-colors hover:bg-shu-deep"
        >
          Open chat
        </Link>
        <a
          href={REPO}
          target="_blank"
          rel="noreferrer"
          aria-label="GitHub repository"
          className="grid h-9 w-9 place-items-center rounded-full border border-ink/15 text-ink transition-colors hover:border-shu hover:text-shu"
        >
          <GithubMark className="h-4 w-4" />
        </a>
      </div>
    </nav>
  </header>
);

export default Nav;
