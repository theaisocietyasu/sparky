import type { Element, ElementContent, Root, RootContent, Text } from "hast";

/** Tags whose text stays as it is: code keeps its spacing and copies cleanly. */
const KEEP = new Set(["code", "pre"]);

/** One word of text that starts at or after from, wrapped so it fades in when it first mounts. */
const wrap = (node: Text, from: number): ElementContent[] => {
  const start = node.position?.start.offset;
  if (start === undefined) return [node];
  const out: ElementContent[] = [];
  let at = 0;
  for (const part of node.value.split(/(\s+)/)) {
    if (part) {
      const fresh = start + at >= from && /\S/.test(part);
      out.push(fresh ? { type: "element", tagName: "span", properties: { className: ["word-in"] }, children: [{ type: "text", value: part }] } : { type: "text", value: part });
    }
    at += part.length;
  }
  return out;
};

const walk = (parent: Root | Element, from: number) => {
  const next: (RootContent | ElementContent)[] = [];
  for (const child of parent.children) {
    if (child.type === "text") next.push(...wrap(child, from));
    else {
      if (child.type === "element" && !KEEP.has(child.tagName)) walk(child, from);
      next.push(child);
    }
  }
  parent.children = next as typeof parent.children;
};

/** A rehype plugin: every word from character from of the markdown on fades in as it appears. */
export const fadeWords = (from: number) => () => (tree: Root) => walk(tree, from);
