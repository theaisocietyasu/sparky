import { BellRing, ThumbsDown, ThumbsUp } from "lucide-react";
import { useState } from "react";
import { nudgeLabel } from "../../app/nudges";
import { rateNotice } from "../../chat/client";
import type { NoticeRef } from "../../chat/types";
import { cue } from "../sparky/cue";

/** The tag above a message Sparky sent on its own: what kind of check-in it is, or a reminder. */
export const NoticeTag = ({ notice }: { notice: NoticeRef }) => (
  <p className="mb-1.5 inline-flex items-center gap-1.5 text-xs font-medium text-dim">
    <BellRing className="size-3.5 text-kin" />
    {notice.kind === "reminder" ? "Reminder" : nudgeLabel(notice.kind)}
  </p>
);

/** Thumbs on a check-in. Not useful switches that kind off and says so. A reminder has none. */
export const NoticeThumbs = ({ notice }: { notice: NoticeRef }) => {
  const [rated, setRated] = useState<boolean | null>(null);
  const [note, setNote] = useState<string | null>(null);
  if (notice.kind === "reminder") return null;
  const rate = (helpful: boolean) => {
    if (helpful) cue();
    setRated(helpful);
    rateNotice(notice.id, helpful)
      .then((r) => setNote(r.turned_off ? `No more ${nudgeLabel(r.turned_off).toLowerCase()}. Turn it back on in Settings.` : null))
      .catch(() => setRated(null));
  };
  const button = (helpful: boolean, label: string, Icon: typeof ThumbsUp) => (
    <button
      type="button"
      aria-label={label}
      aria-pressed={rated === helpful}
      onClick={() => rate(helpful)}
      className={`grid size-7 place-items-center rounded-md transition-colors hover:bg-sunk ${rated === helpful ? "text-kin" : "text-dim hover:text-fg"}`}
    >
      <Icon className="size-3.5" />
    </button>
  );
  return (
    <>
      {button(true, "Useful", ThumbsUp)}
      {button(false, "Not useful", ThumbsDown)}
      {note && <span className="text-xs text-dim">{note}</span>}
    </>
  );
};
