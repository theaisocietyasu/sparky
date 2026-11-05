/** The repository tree the explorer shows. */
export const EXPLORER = [
  "apps/",
  "  engine/src/runtime/tools/",
  "  evals/",
  "deploy/",
  ".env",
  "justfile",
  "sparky.toml",
];

/** The explorer with a new file listed under its folder. */
export const withFile = (rows: string[], after: string, file: string) => {
  const i = rows.indexOf(after);
  return [...rows.slice(0, i + 1), file, ...rows.slice(i + 1)];
};

/** Where the first line of code sits on the stage, and the height of a line and width of a character. */
const CODE = { x: 420, y: 178, line: 25, char: 9.6 };

/** The stage point of column col on line i of the editor. */
export const spot = (i: number, col = 0, s = 1.3) => ({ x: CODE.x + col * CODE.char, y: CODE.y + i * CODE.line, s });

/** Where the pointer rests while the developer types. */
export const REST = { t: 0, x: 1180, y: 700, s: 1 };
