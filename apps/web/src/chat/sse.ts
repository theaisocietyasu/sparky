/** One named server-sent event frame. */
export type Frame = { name: string; data: string };

/** Splits every complete frame off buffer and returns them with the partial tail left over. */
export const drainFrames = (buffer: string): { frames: Frame[]; rest: string } => {
  const frames: Frame[] = [];
  let rest = buffer.replace(/\r\n/g, "\n");
  let end = rest.indexOf("\n\n");
  while (end !== -1) {
    const block = rest.slice(0, end);
    rest = rest.slice(end + 2);
    let name = "";
    const data: string[] = [];
    for (const line of block.split("\n")) {
      if (line.startsWith("event:")) name = line.slice(6).trim();
      else if (line.startsWith("data:")) data.push(line.slice(5).trimStart());
    }
    if (data.length > 0) frames.push({ name, data: data.join("\n") });
    end = rest.indexOf("\n\n");
  }
  return { frames, rest };
};
