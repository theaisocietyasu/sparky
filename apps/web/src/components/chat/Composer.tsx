import { ArrowUp, FileText, Paperclip, Square, X } from "lucide-react";
import { type DragEvent, type FormEvent, type KeyboardEvent, useEffect, useRef } from "react";
import { ACCEPT } from "../../chat/files";

/** The message box: grows with its text, sends on Enter, takes files, and offers Stop while a turn runs. */
const Composer = ({
  value,
  onChange,
  onSend,
  onStop,
  busy,
  autoFocus,
  files = [],
  onFiles,
  onRemoveFile,
}: {
  value: string;
  onChange: (text: string) => void;
  onSend: () => void;
  onStop: () => void;
  busy: boolean;
  autoFocus?: boolean;
  files?: File[];
  onFiles?: (files: File[]) => void;
  onRemoveFile?: (index: number) => void;
}) => {
  const box = useRef<HTMLTextAreaElement>(null);
  const picker = useRef<HTMLInputElement>(null);

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

  const drop = (e: DragEvent) => {
    if (!onFiles || e.dataTransfer.files.length === 0) return;
    e.preventDefault();
    onFiles([...e.dataTransfer.files]);
  };

  const ready = Boolean(value.trim()) || files.length > 0;

  return (
    <form
      onSubmit={submit}
      onDragOver={(e) => onFiles && e.preventDefault()}
      onDrop={drop}
      className="glass rounded-3xl p-2 transition-shadow focus-within:shadow-[0_0_0_3px_rgb(120_28_53/0.12),0_8px_32px_-12px_rgb(0_0_0/0.18)]"
    >
      {files.length > 0 && (
        <ul aria-label="Attached files" className="flex flex-wrap gap-2 px-2 pb-2 pt-1">
          {files.map((f, i) => (
            <li key={`${f.name}-${i}`} className="flex h-8 max-w-full items-center gap-1.5 rounded-lg bg-sunk pl-2.5 pr-1 text-sm">
              <FileText className="size-4 shrink-0 text-dim" />
              <span className="truncate">{f.name}</span>
              <button
                type="button"
                onClick={() => onRemoveFile?.(i)}
                aria-label={`Remove ${f.name}`}
                className="grid size-6 shrink-0 place-items-center rounded-md text-dim hover:bg-edge hover:text-fg"
              >
                <X className="size-3.5" />
              </button>
            </li>
          ))}
        </ul>
      )}
      <div className="flex items-end gap-1">
        {onFiles && (
          <>
            <input
              ref={picker}
              type="file"
              multiple
              accept={ACCEPT}
              aria-label="Attach files"
              className="hidden"
              onChange={(e) => {
                onFiles([...(e.target.files ?? [])]);
                e.target.value = "";
              }}
            />
            <button
              type="button"
              onClick={() => picker.current?.click()}
              aria-label="Attach a file"
              className="grid size-10 shrink-0 place-items-center rounded-full text-dim hover:bg-sunk hover:text-fg"
            >
              <Paperclip className="size-5" />
            </button>
          </>
        )}
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
          className={`max-h-[200px] min-h-10 flex-1 resize-none bg-transparent py-2 text-[15px] leading-6 outline-none placeholder:text-dim ${onFiles ? "" : "pl-2"}`}
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
            disabled={!ready}
            aria-label="Send"
            className="grid size-10 shrink-0 place-items-center rounded-full bg-fg text-canvas hover:opacity-90 disabled:bg-sunk disabled:text-dim"
          >
            <ArrowUp className="size-5" />
          </button>
        )}
      </div>
    </form>
  );
};

export default Composer;
