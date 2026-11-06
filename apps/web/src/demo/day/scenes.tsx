import Message from "@/components/discord/Message";
import { PEOPLE, SPARKY, type Author, type LinkButton } from "@/components/discord/people";
import { LIVE, button } from "@/components/discord/examples";
import Window, { Composer } from "@/components/discord/Window";
import { Caret } from "../Caret";
import MentionThread from "../MentionThread";
import { progress, typed, type Scene, type Shot } from "../timeline";
import { approved, cardAt, finished, FINISH, type Turn } from "./turn";
import { BETWEEN, BOOK, DUE, GRADE, WEEK } from "./turns";

/** The student whose day the video follows. */
const ME: Author = PEOPLE.maya;

/** When each beat of a turn happens, in seconds from the start of its scene. */
const beats = (turn: Turn) => {
  const type = 0.9;
  const typedAt = type + Math.min(2.6, Math.max(1.4, turn.question.length * 0.028));
  const send = typedAt + 0.3;
  const post = send + 0.5;
  const done = post + FINISH;
  const press = turn.held ? done + 1.8 : undefined;
  const end = (press !== undefined ? press + 1.2 : done) + 2.2;
  return { type, typedAt, send, post, done, press, end };
};

/** The composer, typing text between from and to. */
const typing = (placeholder: string, text: string, t: number, from: number, to: number, mention = false) => (
  <Composer placeholder={placeholder}>
    {t >= from && t < to + 0.3 && (
      <span>
        {mention && <span className="rounded-[3px] bg-[#5865f2]/30 px-0.5 font-medium text-[#c9cdfb]">@Sparky</span>}
        {mention && " "}
        {typed(text, t, from, to)}
        <Caret t={t} className="ml-px h-5 w-0.5 translate-y-1 bg-[#dbdee1]" />
      </span>
    )}
  </Composer>
);

/** A turn already finished earlier in the day, as the conversation above shows it. */
type Past = { turn: Turn; time: string };

/** A direct message with Sparky: earlier turns, then the turn the scene plays. */
const dmScene = ({ title, turn, time, past, focus }: { title: string; turn: Turn; time: string; past: Past[]; focus: Omit<Shot, "t"> }): Scene => {
  const b = beats(turn);
  const view = (t: number) => {
    const pressed = b.press !== undefined && t >= b.press + 0.6;
    const card = pressed ? approved(turn) : cardAt(turn, t - b.post);
    const asking = turn.held && t >= b.post + FINISH && !pressed;
    const buttons: LinkButton[] = asking ? [{ label: "Yes, do it", style: "danger" }, { label: "No", style: "secondary" }] : [];
    return (
      <Window
        dm={SPARKY}
        channel="Sparky"
        topic="Your Canvas and Google Calendar, read only here"
        me={ME}
        messages={
          <>
            {past.map((p) => (
              <div key={p.turn.question}>
                <Message author={ME} time={p.time} text={p.turn.question} />
                <Message author={SPARKY} time={p.time} text={p.turn.held ? approved(p.turn) : finished(p.turn)} />
              </div>
            ))}
            {t >= b.send && <Message author={ME} time={time} text={turn.question} />}
            {card && (
              <Message
                author={SPARKY}
                time={time}
                text={card}
                buttons={buttons}
                hovered={b.press !== undefined && t >= b.press - 0.4 ? 0 : undefined}
              />
            )}
          </>
        }
        composer={typing("Message @Sparky", turn.question, t, b.type, b.typedAt)}
      />
    );
  };
  return {
    title,
    length: b.end,
    shots: [
      { t: 0, x: 800, y: 500, s: 1 },
      { t: b.type - 0.2, x: 900, y: 790, s: 1.45 },
      { t: b.send, x: 900, y: 790, s: 1.45 },
      { ...focus, t: b.post + 0.6 },
      { ...focus, t: b.end - 1.0 },
      { t: b.end, x: 800, y: 500, s: 1 },
    ],
    pointer: [
      { t: 0, x: 1420, y: 860, s: 1 },
      ...(b.press !== undefined
        ? [
            { t: b.press - 0.8, x: 1420, y: 860, s: 1 },
            { t: b.press - 0.1, x: 510, y: 825, s: 1 },
          ]
        : []),
    ],
    clicks: b.press !== undefined ? [b.press] : [],
    keys: [],
    view,
  };
};

/** Other students in the channel before the question. */
const CHATTER: { author: Author; text: string }[] = [
  { author: PEOPLE.devon, text: "anyone know if the MU starbucks line is still insane" },
  { author: PEOPLE.sam, text: "it's always insane lol" },
];

/** A mention in the server channel, answered in a thread. */
const serverScene = ({ title, turn, time, sources }: { title: string; turn: Turn; time: string; sources: LinkButton[] }): Scene => {
  const b = beats(turn);
  const view = (t: number) => {
    const card = cardAt(turn, t - b.post);
    return (
      <MentionThread
        topic="Ask Sparky about classes, hours, shuttles and more. Mention it to start a thread."
        asker={ME}
        time={time}
        question={turn.question}
        earlier={CHATTER.map((m) => (
          <Message key={m.text} author={m.author} time="Today at 12:14 PM" text={m.text} />
        ))}
        composer={typing("Message #ask-sparky", turn.question, t, b.type, b.typedAt, true)}
        asked={t >= b.send}
        chip={t >= b.send + 0.2}
        opening={progress(t, b.post - 0.4, 0.4)}
        reply={card ? { text: card, buttons: t >= b.done ? sources : [] } : null}
      />
    );
  };
  return {
    title,
    length: b.end,
    shots: [
      { t: 0, x: 800, y: 500, s: 1 },
      { t: b.type - 0.2, x: 760, y: 790, s: 1.45 },
      { t: b.send, x: 760, y: 790, s: 1.45 },
      { t: b.post + 0.6, x: 1220, y: 560, s: 1.3 },
      { t: b.end - 1.0, x: 1220, y: 560, s: 1.3 },
      { t: b.end, x: 800, y: 500, s: 1 },
    ],
    pointer: [{ t: 0, x: 700, y: 640, s: 1 }],
    clicks: [],
    keys: [],
    view,
  };
};

/** The day, scene by scene, from the morning check to the plan for the week. */
export const DAY: Scene[] = [
  dmScene({ title: "7:48 AM · What's due this week", turn: DUE, time: "Today at 7:48 AM", past: [], focus: { x: 900, y: 560, s: 1.25 } }),
  serverScene({
    title: "12:20 PM · Between classes",
    turn: BETWEEN,
    time: "Today at 12:20 PM",
    sources: [button(LIVE.dining), button(LIVE.libraryHours)],
  }),
  dmScene({
    title: "3:40 PM · Book a study room",
    turn: BOOK,
    time: "Today at 3:40 PM",
    past: [{ turn: DUE, time: "Today at 7:48 AM" }],
    focus: { x: 900, y: 620, s: 1.3 },
  }),
  dmScene({
    title: "6:30 PM · What the final needs",
    turn: GRADE,
    time: "Today at 6:30 PM",
    past: [{ turn: BOOK, time: "Today at 3:40 PM" }],
    focus: { x: 900, y: 620, s: 1.3 },
  }),
  dmScene({
    title: "9:50 PM · Plan the week",
    turn: WEEK,
    time: "Today at 9:50 PM",
    past: [{ turn: GRADE, time: "Today at 6:30 PM" }],
    focus: { x: 900, y: 540, s: 1.2 },
  }),
];
