import type { Upload } from "./types";

/** File types the engine reads. */
export const ACCEPT = ".pdf,.txt,.md,.csv,.tsv,.json,.xml,.yaml,.yml,.toml,.html,.css,.js,.ts,.tsx,.py,.rs,.java,.c,.cpp,.h,.go,.sql,.tex";
export const MAX_FILES = 4;
export const MAX_FILE_BYTES = 2_000_000;
export const MAX_TOTAL_BYTES = 3_000_000;

/** The files after adding more, or why they do not fit. */
export const addFiles = (held: File[], more: File[]): { files: File[]; error: string | null } => {
  const files = [...held, ...more].slice(0, MAX_FILES);
  if (held.length + more.length > MAX_FILES) return { files, error: `Up to ${MAX_FILES} files per message.` };
  if (more.some((f) => f.size > MAX_FILE_BYTES)) return { files: held, error: "Each file can be up to 2 MB." };
  if (files.reduce((n, f) => n + f.size, 0) > MAX_TOTAL_BYTES) return { files: held, error: "Files in one message can add up to 3 MB." };
  return { files, error: null };
};

/** A file as the engine takes it inline: its bytes in base64. */
export const toUpload = (file: File): Promise<Upload> =>
  new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error);
    reader.onload = () => {
      const url = String(reader.result);
      resolve({ name: file.name, media_type: file.type, size: file.size, data: url.slice(url.indexOf(",") + 1) });
    };
    reader.readAsDataURL(file);
  });
