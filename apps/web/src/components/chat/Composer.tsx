import { ArrowUp, Square } from "lucide-react";
import { type FormEvent, type KeyboardEvent, useEffect, useRef } from "react";

/** The message box: grows with its text, sends on Enter, and offers Stop while a turn runs. */
const Composer = ({
  value,
  onChange,
  onSend,
  onStop,
  busy,
  autoFocus,
}: {
  value: string;
  onChange: (text: string) => void;
  onSend: () => void;
  onStop: () => void;
  busy: boolean;
  autoFocus?: boolean;
}) => {
  const box = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    const el = box.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
  }, [value]);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    onSend();
  };

  const keys = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      onSend();
    }
  };

  return (
    <form
      onSubmit={submit}
      className="flex items-end gap-2 rounded-3xl border border-edge bg-raised p-2 pl-4 shadow-[0_1px_2px_rgb(0_0_0/0.04),0_4px_16px_rgb(0_0_0/0.04)] focus-within:border-dim"
    >
      <label htmlFor="message" className="sr-only">
        Message
      </label>
      <textarea
        id="message"
        ref={box}
        rows={1}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={keys}
        placeholder="Ask Sparky anything about ASU"
        autoComplete="off"
        autoFocus={autoFocus}
        className="max-h-[200px] min-h-10 flex-1 resize-none bg-transparent py-2 text-[15px] leading-6 outline-none placeholder:text-dim"
      />
      {busy ? (
        <button
          type="button"
          onClick={onStop}
          aria-label="Stop"
          className="grid size-10 shrink-0 place-items-center rounded-full bg-fg text-canvas hover:opacity-90"
        >
          <Square className="size-3.5 fill-current" />
        </button>
      ) : (
        <button
          type="submit"
          disabled={!value.trim()}
          aria-label="Send"
          className="grid size-10 shrink-0 place-items-center rounded-full bg-fg text-canvas hover:opacity-90 disabled:bg-sunk disabled:text-dim"
        >
          <ArrowUp className="size-5" />
        </button>
      )}
    </form>
  );
};

export default Composer;
