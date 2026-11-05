/** An answer split into its prose and the tap-to-answer choices the engine put in a choices block. */
export type Split = { text: string; choices: string[] };

const BLOCK = /```choices[^\n]*\n([\s\S]*?)```\s*/g;
const OPEN = /```choices[\s\S]*$/;

/** Takes every choices block out of the text. An unclosed block, still streaming, is hidden. */
export const splitChoices = (raw: string): Split => {
  const choices: string[] = [];
  const text = raw
    .replace(BLOCK, (_, body: string) => {
      for (const line of body.split("\n")) {
        const choice = line.replace(/^\s*[-*\d.)]*\s*/, "").trim();
        if (choice && choices.length < 4 && !choices.includes(choice)) choices.push(choice.slice(0, 60));
      }
      return "";
    })
    .replace(OPEN, "")
    .trimEnd();
  return { text, choices };
};
