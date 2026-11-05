import { Brain, LayoutDashboard, MessageSquare, SquarePen } from "lucide-react";
import type { ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import Frame, { type NavGroup } from "./Frame";
import { isModerator, useMe } from "./useMe";

/** The frame of the student pages: chat and memories, and admin for a moderator. */
const ChatFrame = ({ onNewChat, children }: { onNewChat?: () => void; children: ReactNode }) => {
  const { me } = useMe();
  const navigate = useNavigate();
  const groups: NavGroup[] = [
    {
      items: [
        { to: "/chat", label: "Chat", icon: MessageSquare },
        { to: "/memories", label: "Memories", icon: Brain },
        ...(isModerator(me) ? [{ to: "/admin", label: "Admin", icon: LayoutDashboard }] : []),
      ],
    },
  ];
  const actions = (
    <button
      type="button"
      onClick={onNewChat ?? (() => navigate("/chat"))}
      className="flex h-9 w-full items-center gap-2.5 rounded-lg border border-edge bg-raised px-2 text-sm font-medium hover:bg-sunk"
    >
      <SquarePen className="size-4" />
      New conversation
    </button>
  );
  return (
    <Frame label="Chat" groups={groups} actions={actions}>
      {children}
    </Frame>
  );
};

export default ChatFrame;
