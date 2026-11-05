import {
  finishedCard,
  liveResult,
  runningCard,
  sourceLabel,
  storedResult,
  thinkingStep,
  thoughtStep,
  toolDone,
  toolStarted,
  type ToolCall,
} from "@/components/discord/format";
import { PEOPLE, type Author } from "@/components/discord/people";
import { Composer, type Server } from "@/components/discord/Window";
import MentionThread from "../../MentionThread";
import { progress, type Scene } from "../../timeline";
import { CAMPUS, MCP } from "../campus";

/** The campus Discord the bot answers in once deployed. */
const SERVER: Server = { name: CAMPUS.name, badge: "EU", className: "bg-[#1d3557] text-white" };

/** The first student to ask the newly deployed bot. */
const ASKER: Author = PEOPLE.jordan;

/** The timestamp on the question and the reply. */
const TIME = "Today at 9:14 AM";
/** What the student asks. */
const QUESTION = "is the rec center open over spring break, and what's for dinner at the commons tonight?";

/** The three calls: the indexed calendar page, the new live source, and the MCP server. */
const CALLS: ToolCall[] = [
  {
    tool: "search",
    args: { query: "spring break 2027 dates" },
    output: storedResult("Academic Calendar", CAMPUS.page, "2 hours ago", "Spring 2027. Classes begin January 11. Spring break March 8 to 14."),
  },
  {
    tool: "search",
    args: { query: "rec center hours spring break", source: "rec_hours" },
    output: liveResult("rec_hours", CAMPUS.rec, "Spring break, March 8 to 14: Rec center 8:00am to 6:00pm. Pool closed. Regular hours resume March 15."),
  },
  {
    tool: MCP.tools[0],
    args: { hall: "commons", meal: "dinner" },
    output: "The Commons, dinner 5:00pm to 8:00pm: herb roast chicken, black bean burgers, pasta bar, salad bar.",
  },
];

/** The first thought line of the card. */
const THOUGHT = thoughtStep(
  "Three parts: spring break dates from the calendar, rec center hours from its live page, and tonight's dinner from the dining menu.",
);
/** The thought line after the calls returned. */
const WRAP = thoughtStep("I have the dates, the rec hours for that week, and the Commons menu.");
/** The finished answer. */
const ANSWER =
  "Yes. Over spring break (**March 8 to 14**) the rec center is open **8 AM to 6 PM**, and the pool is closed that week.\n\nDinner at the Commons tonight runs **5 to 8 PM**: herb roast chicken, black bean burgers, and the pasta bar.";

/** When each beat happens. Card edits are 1.5 s apart, as bot.edit_every_ms. */
const AT = { asked: 0.5, chip: 0.8, open: 1.0, edits: [1.5, 3.0, 4.5, 6.0, 7.5], finished: 9.0, hover: 10.0 };

/** Sparky's card at time t, or null before it is posted. */
const card = (t: number) => {
  const [first, second, third, fourth, fifth] = AT.edits;
  const done = CALLS.map((c) => toolDone(c.tool, c.args, c.output));
  if (t >= AT.finished) return finishedCard([THOUGHT, ...done, WRAP], ANSWER);
  if (t >= fifth) return runningCard([THOUGHT, ...done, WRAP], 4, ANSWER.split("\n\n")[0]);
  if (t >= fourth) return runningCard([THOUGHT, ...done, thinkingStep()], 3);
  if (t >= third) return runningCard([THOUGHT, ...done], 2);
  if (t >= second) return runningCard([THOUGHT, ...CALLS.map((c) => toolStarted(c.tool, c.args))], 1);
  if (t >= first) return runningCard([thinkingStep()], 0);
  return null;
};

/** The deployed bot answering in the campus server. */
const discordAt = (t: number) => {
  const reply = card(t);
  return (
    <MentionThread
      server={SERVER}
      topic={`Ask Sparky about ${CAMPUS.name}. Mention it to start a thread.`}
      asker={ASKER}
      time={TIME}
      question={QUESTION}
      composer={<Composer placeholder="Message #ask-sparky" />}
      asked={t >= AT.asked}
      chip={t >= AT.chip}
      opening={progress(t, AT.open, 0.4)}
      reply={
        reply
          ? {
              text: reply,
              buttons:
                t >= AT.finished
                  ? [
                      { label: "Academic Calendar", href: CAMPUS.page },
                      { label: sourceLabel(CAMPUS.recLabel), href: CAMPUS.rec },
                    ]
                  : [],
              hovered: t >= AT.hover ? 1 : undefined,
            }
          : null
      }
    />
  );
};

/** Scene: a student mentions the deployed bot and it answers in a thread. */
export const discord: Scene = {
  title: "Live on your campus Discord",
  length: 11.6,
  shots: [
    { t: 0, x: 800, y: 500, s: 1 },
    { t: 1.2, x: 800, y: 500, s: 1 },
    { t: 2.0, x: 1220, y: 600, s: 1.35 },
    { t: 9.4, x: 1220, y: 600, s: 1.35 },
    { t: 10.8, x: 800, y: 500, s: 1 },
  ],
  pointer: [
    { t: 0, x: 1180, y: 700, s: 1 },
    { t: 2.0, x: 1180, y: 700, s: 1 },
    { t: 9.8, x: 1290, y: 832, s: 1 },
  ],
  clicks: [],
  keys: [],
  view: discordAt,
};
