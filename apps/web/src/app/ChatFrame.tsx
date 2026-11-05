import { House } from "lucide-react";
import { type ReactNode, useEffect, useRef, useState } from "react";
import { NavLink } from "react-router-dom";
import { notices } from "../chat/client";
import { cue } from "../components/sparky/cue";
import ChatList from "./ChatList";
import Frame from "./Frame";

const EVERY_MS = 60_000;

/** How many of Sparky's own messages the student has not seen, checked every minute. Sparky hops when one arrives. */
const useUnread = (seen: number) => {
  const [unread, setUnread] = useState(0);
  const last = useRef<number | null>(null);
  useEffect(() => {
    const load = () =>
      notices()
        .then((n) => {
          if (last.current !== null && n.unread > last.current) cue();
          last.current = n.unread;
          setUnread(n.unread);
        })
        .catch(() => undefined);
    load();
    const timer = window.setInterval(() => document.visibilityState === "visible" && load(), EVERY_MS);
    return () => window.clearInterval(timer);
  }, [seen]);
  return unread;
};

/**
 * The frame of the student pages: Home, where Sparky's own messages land, and the threads list.
 * seen changes when home has been read, so the dot clears. Settings and admin sit in the profile menu.
 */
const ChatFrame = ({ chats, seen = 0, children }: { chats?: number; seen?: number; children: ReactNode }) => {
  const unread = useUnread(seen);
  const actions = (
    <NavLink
      to="/chat"
      end
      className={({ isActive }) =>
        `flex h-9 w-full items-center gap-2.5 rounded-lg px-2 text-sm font-medium transition-colors ${
          isActive ? "bg-sunk text-fg" : "text-fg hover:bg-sunk/70"
        }`
      }
    >
      <House className="size-4" />
      Home
      {unread > 0 && <span aria-label={`${unread} new`} className="ml-auto size-2 rounded-full bg-kin" />}
    </NavLink>
  );
  return (
    <Frame label="Chats" groups={[]} actions={actions} extra={<ChatList version={chats} />}>
      {children}
    </Frame>
  );
};

export default ChatFrame;
