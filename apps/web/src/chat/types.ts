/** A progress line the engine sends while a turn runs. */
export type Progress = { text: string; slot?: string; clear?: boolean; draft?: boolean };

/** A source the answer rests on. */
export type Citation = { key: string; title: string; url?: string | null };

/** An action the engine holds until the student approves it. */
export type Confirmation = { token: string; tool: string; summary: string };

/** The answer to one turn. */
export type Answer = {
  request_id: string;
  conversation_id: string;
  text: string;
  status: string;
  citations?: Citation[];
  confirmation?: Confirmation | null;
  memories?: string[];
  notice?: NoticeRef | null;
};

/** The notification behind a message Sparky sent on its own. */
export type NoticeRef = { id: string; kind: "reminder" | NudgeKind };

/** The message a thread starts from. */
export type Parent = { conversation_id: string; position: number };

/** The signed-in student. */
export type Me = { user_id: string; name: string; roles: string[]; expires_at: number };

/** One remembered fact. */
export type Memory = { number: number; id: string; kind: string; text: string; created_at: string; due?: string | null };

/** One thread in the sidebar, with the message it started from when it has one. */
export type ChatSummary = { id: string; title: string; updated_at: string; parent?: Parent };

/** One stored message of a chat. notice is set on a message Sparky sent on its own. */
export type ChatTurn = { position?: number; role: "user" | "assistant"; text: string; notice?: NoticeRef };

/** A file sent inline with a message. */
export type Upload = { name: string; media_type: string; size: number; data: string };

/** How often a reminder comes back. */
export type Repeat = "once" | "daily" | "weekly";

/** A reminder the student set in a chat. */
export type Reminder = { id: string; text: string; due_at: string; repeat: Repeat };

/** What made a check-in Sparky sends on its own. */
export type NudgeKind =
  | "brief"
  | "due_soon"
  | "start_early"
  | "new_item"
  | "event"
  | "follow_up"
  | "calendar"
  | "study"
  | "week_plan";

/** A message for the student: a reminder that fired, or a check-in Sparky sent on its own. */
export type Notice = {
  id: string;
  text: string;
  created_at: string;
  read: boolean;
  kind: "reminder" | NudgeKind;
  helpful: boolean | null;
};

/** The student's quiet hours and the check-ins they switched off. */
export type NudgePrefs = { quiet_start: number; quiet_end: number; off: NudgeKind[] };
