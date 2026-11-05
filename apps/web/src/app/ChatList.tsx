import { Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { NavLink, useNavigate, useParams } from "react-router-dom";
import { chats, deleteChat } from "../chat/client";
import type { ChatSummary } from "../chat/types";

/** The student's threads in the sidebar: open one, or delete it. Reloads when version changes. */
const ChatList = ({ version = 0 }: { version?: number }) => {
  const [items, setItems] = useState<ChatSummary[] | null>(null);
  const { id } = useParams();
  const navigate = useNavigate();

  useEffect(() => {
    let live = true;
    chats()
      .then((c) => live && setItems(c))
      .catch(() => live && setItems([]));
    return () => {
      live = false;
    };
  }, [version]);

  const drop = async (chat: ChatSummary) => {
    try {
      await deleteChat(chat.id);
    } catch {
      return;
    }
    setItems((all) => all?.filter((c) => c.id !== chat.id) ?? null);
    if (chat.id === id) navigate("/chat");
  };

  if (!items || items.length === 0) return null;
  return (
    <div className="mb-4">
      <p className="px-2 pb-1 text-xs font-medium text-dim">Threads</p>
      <ul aria-label="Threads" className="space-y-0.5">
        {items.map((c) => (
          <li key={c.id} className="group relative">
            <NavLink
              to={`/chat/${c.id}`}
              className={({ isActive }) =>
                `flex h-9 items-center rounded-lg pl-2 pr-9 text-sm transition-colors ${
                  isActive ? "bg-sunk font-medium text-fg" : "text-dim hover:bg-sunk/70 hover:text-fg"
                }`
              }
            >
              <span className="truncate">{c.title}</span>
            </NavLink>
            <button
              type="button"
              onClick={() => drop(c)}
              aria-label={`Delete thread ${c.title}`}
              className="absolute right-1 top-1/2 grid size-7 -translate-y-1/2 place-items-center rounded-md text-dim opacity-60 transition md:opacity-0 hover:text-bad focus-visible:opacity-100 group-hover:opacity-100"
            >
              <Trash2 className="size-3.5" />
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
};

export default ChatList;
