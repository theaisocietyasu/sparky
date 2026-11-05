import { liveResult, sourceLabel, thoughtStep, toolDone, type Args } from "./format";
import { PEOPLE, type Author, type LinkButton } from "./people";

/** One live source the bot reaches, as the engine names it. */
type Live = { key: string; label: string; url: string };

/** Live sources search can name, with the keys Platform publishes and labels for the buttons. */
export const LIVE = {
  courses: { key: "courses", label: "ASU Class Search", url: "https://catalog.apps.asu.edu/catalog/classes/classlist" },
  scholarships: { key: "scholarships", label: "ASU Scholarship Search", url: "https://onsa.asu.edu/scholarships" },
  libraryHours: { key: "library_hours", label: "ASU Library Hours", url: "https://lib.asu.edu/hours" },
  shuttles: { key: "shuttles", label: "ASU Shuttle Tracker", url: "https://asu-shuttles.rider.peaktransit.com/" },
  studyRooms: { key: "study_rooms", label: "ASU Library Study Rooms", url: "https://asu.libcal.com/r/accessible/availability" },
  dining: { key: "dining", label: "Sun Devil Dining", url: "https://asu.campusdish.com/" },
} satisfies Record<string, Live>;

/** One search call: its arguments, and the page text it brought back. */
export type LiveCall = { source: Live; query: string; text: string };

/** The arguments of a search call. */
const argsOf = (call: LiveCall): Args => ({ query: call.query, source: call.source.key });

/** The progress line of a search call once it returned. */
export const done = (call: LiveCall) =>
  toolDone("search", argsOf(call), liveResult(call.source.key, call.source.url, call.text));

/** The link button a cited source puts under the answer. */
export const button = (source: Live): LinkButton => ({ label: sourceLabel(source.label), href: source.url });

/** One question and the finished reply, as a student would see it in a thread. */
export type Example = {
  title: string;
  asker: Author;
  question: string;
  thought: string;
  calls: LiveCall[];
  answer: string;
};

/** The examples the Sources section shows, one per source. */
export const EXAMPLES: Example[] = [
  {
    title: "Courses",
    asker: PEOPLE.devon,
    question: "any open seats left in CSE 310 this fall?",
    thought: "Open seats change by the hour, so this needs a live class search for CSE 310 in Fall 2026.",
    calls: [{ source: LIVE.courses, query: "CSE 310 Fall 2026 open seats", text: "CSE 310 Data Structures and Algorithms. Tempe, in person. 3 sections, 41 open seats." }],
    answer: "Yes. CSE 310 has **3 sections** on Tempe this fall with **41 open seats** between them. The Tuesday and Thursday section has the most room.",
  },
  {
    title: "Scholarships",
    asker: PEOPLE.ren,
    question: "scholarships for a CS junior due this month",
    thought: "Scholarship deadlines change each term; search the scholarship listings for computer science juniors.",
    calls: [{ source: LIVE.scholarships, query: "computer science junior scholarships deadline October", text: "Fulton Schools undergraduate scholarship. Juniors in computer science. Deadline Oct 31." }],
    answer: "One fits: the **Fulton Schools undergraduate scholarship** is open to computer science juniors and closes **October 31**. The listing has the application link.",
  },
  {
    title: "Library hours",
    asker: PEOPLE.maya,
    question: "when does Hayden close tonight?",
    thought: "Library hours vary by day, so read today's hours for Hayden Library.",
    calls: [{ source: LIVE.libraryHours, query: "Hayden Library hours today", text: "Hayden Library. Today 7:00am to 12:00am. Noble Library 7:00am to 10:00pm." }],
    answer: "Hayden Library closes at **midnight** tonight. Noble closes earlier, at 10 PM.",
  },
  {
    title: "Shuttles",
    asker: PEOPLE.sam,
    question: "next shuttle from Tempe to Poly?",
    thought: "This needs the live shuttle tracker for the Tempe to Polytechnic route.",
    calls: [{ source: LIVE.shuttles, query: "Tempe to Polytechnic shuttle next departure", text: "Inter-campus shuttle Tempe to Polytechnic. Next departures 4:15pm, 4:45pm from the Tempe stop." }],
    answer: "The next inter-campus shuttle to Polytechnic leaves Tempe at **4:15 PM**, then **4:45 PM**.",
  },
  {
    title: "Study rooms",
    asker: PEOPLE.devon,
    question: "is there a study room for 4 at 6pm today?",
    thought: "Room availability is live, so check library study room bookings for four people at 6 PM today.",
    calls: [{ source: LIVE.studyRooms, query: "study room 4 people today 6pm", text: "Hayden Library group study rooms, capacity 4. Available 6:00pm: 2 rooms." }],
    answer: "Yes. **Two group rooms** for four are free at 6 PM in Hayden Library. Book one from the room page before someone else does.",
  },
  {
    title: "Dining",
    asker: PEOPLE.ren,
    question: "what's open for food on Tempe right now?",
    thought: "Dining hours depend on the time of day; read what Sun Devil Dining has open on Tempe now.",
    calls: [{ source: LIVE.dining, query: "Tempe dining open now", text: "Tempe. Open now: Manzanita dining hall, Barrett dining hall, Memorial Union food court." }],
    answer: "Right now on Tempe: **Manzanita** and **Barrett** dining halls, and the **Memorial Union** food court.",
  },
];

/** The steps a finished example shows: the thought, then each call that returned. */
export const stepsOf = (example: Example) => [thoughtStep(example.thought), ...example.calls.map((c) => done(c))];
