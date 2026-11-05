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
};

/** The signed-in student. */
export type Me = { user_id: string; name: string; roles: string[]; expires_at: number };

/** One remembered fact. */
export type Memory = { number: number; id: string; kind: string; text: string; created_at: string; due?: string | null };

/** One web chat in the sidebar. */
export type ChatSummary = { id: string; title: string; updated_at: string };

/** One stored message of a chat. */
export type ChatTurn = { role: "user" | "assistant"; text: string };

/** A file sent inline with a message. */
export type Upload = { name: string; media_type: string; size: number; data: string };

/** How often a reminder comes back. */
export type Repeat = "once" | "daily" | "weekly";

/** A reminder the student set in a chat. */
export type Reminder = { id: string; text: string; due_at: string; repeat: Repeat };

/** A message for the student, made when a reminder fires. */
export type Notice = { id: string; text: string; created_at: string; read: boolean };
