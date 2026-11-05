import { Brain, LayoutDashboard, Link2, MessageSquare, SquarePen } from "lucide-react";
import type { ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import ChatList from "./ChatList";
import Frame, { type NavGroup } from "./Frame";
import { isModerator, useMe } from "./useMe";

/** The frame of the student pages: chat, memories, connections and the chat list, and admin for a moderator. */
const ChatFrame = ({ onNewChat, chats, children }: { onNewChat?: () => void; chats?: number; children: ReactNode }) => {
  const { me } = useMe();
  const navigate = useNavigate();
  const groups: NavGroup[] = [
    {
      items: [
        { to: "/chat", label: "Chat", icon: MessageSquare, end: true },
        { to: "/memories", label: "Memories", icon: Brain },
        { to: "/connections", label: "Connections", icon: Link2 },
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
    <Frame label="Chat" groups={groups} actions={actions} extra={<ChatList version={chats} />}>
      {children}
    </Frame>
  );
};

export default ChatFrame;
