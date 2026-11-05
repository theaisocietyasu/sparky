import { Check, Copy } from "lucide-react";
import { type ReactNode, isValidElement, memo, useState } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { linkable } from "../../chat/links";

const textOf = (node: ReactNode): string => {
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(textOf).join("");
  if (isValidElement<{ children?: ReactNode }>(node)) return textOf(node.props.children);
  return "";
};

/** A copy button that confirms for a moment after it copies. */
export const CopyButton = ({ text, label }: { text: string; label: string }) => {
  const [done, setDone] = useState(false);
  const copy = () =>
    navigator.clipboard
      ?.writeText(text)
      .then(() => {
        setDone(true);
        setTimeout(() => setDone(false), 1500);
      })
      .catch(() => undefined);
  return (
    <button
      type="button"
      onClick={copy}
      aria-label={done ? "Copied" : label}
      className="inline-flex h-7 items-center gap-1.5 rounded-md px-2 text-xs text-dim hover:bg-sunk hover:text-fg"
    >
      {done ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
      {done ? "Copied" : "Copy"}
    </button>
  );
};

const CodeBlock = ({ children }: { children?: ReactNode }) => {
  const code = isValidElement<{ className?: string; children?: ReactNode }>(children) ? children : null;
  const language = /language-([\w-]+)/.exec(code?.props.className ?? "")?.[1];
  const text = textOf(code?.props.children ?? children).replace(/\n$/, "");
  return (
    <div className="my-3 overflow-hidden rounded-xl border border-edge bg-sunk">
      <div className="flex h-9 items-center justify-between border-b border-edge pl-3 pr-1 text-xs text-dim">
        <span>{language ?? "text"}</span>
        <CopyButton text={text} label="Copy code" />
      </div>
      <pre className="overflow-x-auto p-3 font-mono text-[13px] leading-relaxed">
        <code>{text}</code>
      </pre>
    </div>
  );
};

const components: Components = {
  p: ({ children }) => <p className="my-3 first:mt-0 last:mb-0">{children}</p>,
  a: ({ href, children }) =>
    linkable(href) ? (
      <a href={href} target="_blank" rel="noreferrer" className="font-medium text-accent underline decoration-accent/30 underline-offset-2 hover:decoration-accent">
        {children}
      </a>
    ) : (
      <span>{children}</span>
    ),
  h1: ({ children }) => <h3 className="mb-2 mt-5 text-lg font-semibold first:mt-0">{children}</h3>,
  h2: ({ children }) => <h3 className="mb-2 mt-5 text-base font-semibold first:mt-0">{children}</h3>,
  h3: ({ children }) => <h4 className="mb-1.5 mt-4 font-semibold first:mt-0">{children}</h4>,
  h4: ({ children }) => <h5 className="mb-1 mt-3 font-semibold first:mt-0">{children}</h5>,
  ul: ({ children }) => <ul className="my-3 list-disc space-y-1 pl-6 marker:text-dim">{children}</ul>,
  ol: ({ children }) => <ol className="my-3 list-decimal space-y-1 pl-6 marker:text-dim">{children}</ol>,
  li: ({ children }) => <li className="pl-1">{children}</li>,
  blockquote: ({ children }) => <blockquote className="my-3 border-l-2 border-edge pl-4 text-dim">{children}</blockquote>,
  hr: () => <hr className="my-5 border-edge" />,
  strong: ({ children }) => <strong className="font-semibold">{children}</strong>,
  pre: ({ children }) => <CodeBlock>{children}</CodeBlock>,
  code: ({ children }) => <code className="rounded-md bg-sunk px-1.5 py-0.5 font-mono text-[0.85em]">{children}</code>,
  table: ({ children }) => (
    <div className="my-3 overflow-x-auto rounded-xl border border-edge">
      <table className="w-full border-collapse text-sm [overflow-wrap:normal]">{children}</table>
    </div>
  ),
  thead: ({ children }) => <thead className="bg-sunk text-left">{children}</thead>,
  th: ({ children }) => <th className="whitespace-nowrap border-b border-edge px-3 py-2 font-medium">{children}</th>,
  td: ({ children }) => <td className="border-b border-edge px-3 py-2 align-top">{children}</td>,
  img: ({ alt }) => <span className="text-dim">{alt}</span>,
};

const plugins = [remarkGfm];

/** Model text as GitHub flavored markdown. Raw HTML stays text and images show their alt text. */
const Markdown = memo(({ text, streaming }: { text: string; streaming?: boolean }) => (
  <div className={`text-[15px] leading-7 [overflow-wrap:anywhere] ${streaming ? "streaming" : ""}`}>
    <ReactMarkdown remarkPlugins={plugins} components={components}>
      {text}
    </ReactMarkdown>
  </div>
));

export default Markdown;
