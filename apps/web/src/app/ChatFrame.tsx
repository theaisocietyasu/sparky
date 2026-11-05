import { SquarePen } from "lucide-react";
import type { ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import Bell from "./Bell";
import ChatList from "./ChatList";
import Frame from "./Frame";

/** The frame of the student pages: the notification bell, a new chat button and the chat list. Settings and admin sit in the profile menu. */
const ChatFrame = ({ onNewChat, chats, children }: { onNewChat?: () => void; chats?: number; children: ReactNode }) => {
  const navigate = useNavigate();
  const actions = (
    <button
      type="button"
      onClick={onNewChat ?? (() => navigate("/chat"))}
      className="flex h-9 w-full items-center gap-2.5 rounded-lg px-2 text-sm font-medium text-fg transition-colors hover:bg-sunk/70"
    >
      <SquarePen className="size-4" />
      New chat
    </button>
  );
  return (
    <Frame label="Chats" groups={[]} tools={<Bell />} actions={actions} extra={<ChatList version={chats} />}>
      {children}
    </Frame>
  );
};

export default ChatFrame;
