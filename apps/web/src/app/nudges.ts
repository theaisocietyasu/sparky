import type { NudgeKind } from "../chat/types";

/** Each check-in Sparky can send, in the words Settings and the chat use. */
export const NUDGES: { kind: NudgeKind; label: string; detail: string }[] = [
  { kind: "brief", label: "Morning brief", detail: "Weekday mornings: what is due today and this week." },
  { kind: "due_soon", label: "Due soon", detail: "A day before something is due." },
  { kind: "start_early", label: "Start early", detail: "A few days before an exam or a big project." },
  { kind: "new_item", label: "New assignments", detail: "When something new shows up on Canvas." },
  { kind: "after_due", label: "After a deadline", detail: "A few hours after a big assignment or exam: how it went." },
  { kind: "exam_week", label: "Exam week", detail: "When two or more exams land in one week. Other check-ins go quiet." },
  { kind: "event", label: "Events for you", detail: "At most one a day, picked from what Sparky knows about you." },
  { kind: "opportunity", label: "Internships and hackathons", detail: "Wednesdays: an opening that fits you, when there is one." },
  { kind: "follow_up", label: "Follow-ups", detail: "A check-in on something you said you would do." },
  { kind: "calendar", label: "Academic calendar", detail: "Drop, withdrawal, registration, aid and scholarship deadlines." },
  { kind: "study", label: "Late study spots", detail: "The night before a deadline, when the library closes early." },
  { kind: "week_plan", label: "Sunday plan", detail: "Sunday evening: your week, day by day." },
  { kind: "wins", label: "Friday wins", detail: "Friday afternoon: what you got through this week." },
  { kind: "tips", label: "Tips", detail: "Your first two weeks: one tip a day on what Sparky can do." },
];

/** The name of one check-in kind. */
export const nudgeLabel = (kind: NudgeKind) => NUDGES.find((n) => n.kind === kind)?.label ?? kind;
