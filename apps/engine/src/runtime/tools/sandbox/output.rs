//! Bounded command output: the head and tail of each stream, and of the text handed back.

use std::collections::VecDeque;

use tokio::process::Command;

/// Keeps the head and the tail of text, marking what was dropped between them.
pub(crate) fn clip(text: &str, max: usize) -> String {
    if max == 0 || text.chars().count() <= max {
        return text.to_owned();
    }
    let head_len = max.div_ceil(2);
    let tail_len = max - head_len;
    let chars: Vec<char> = text.chars().collect();
    let dropped = chars.len() - max;
    let head: String = chars.iter().take(head_len).collect();
    let tail: String = chars.iter().skip(chars.len() - tail_len).collect();
    format!("{head}\n[{dropped} characters cut]\n{tail}")
}

/// What a command produced, each stream held to its head and tail of cap bytes.
pub(super) struct Bounded {
    pub(super) status: std::process::ExitStatus,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
}

/// Runs command and reads both streams, keeping at most cap bytes of each stream's head and tail.
pub(super) async fn bounded_output(command: Command, cap: usize) -> std::io::Result<Bounded> {
    bounded_input(command, cap, None).await
}

/// The same as bounded_output, with input written to standard input and then closed.
pub(super) async fn bounded_input(
    mut command: Command,
    cap: usize,
    input: Option<Vec<u8>>,
) -> std::io::Result<Bounded> {
    use tokio::io::AsyncWriteExt;

    let mut child = command.spawn()?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        // A child that exits without reading its input is not a failure to start.
        let _ = stdin.write_all(&input).await;
        let _ = stdin.shutdown().await;
    }
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (out, err, status) = tokio::join!(
        read_bounded(stdout, cap),
        read_bounded(stderr, cap),
        child.wait()
    );
    Ok(Bounded {
        status: status?,
        stdout: out,
        stderr: err,
    })
}

/// The head and tail of a stream, cap bytes each, joined by a line naming what was dropped.
async fn read_bounded<R: tokio::io::AsyncRead + Unpin>(stream: Option<R>, cap: usize) -> Vec<u8> {
    use tokio::io::AsyncReadExt;

    let Some(mut stream) = stream else {
        return Vec::new();
    };
    let mut head: Vec<u8> = Vec::new();
    let mut tail: VecDeque<u8> = VecDeque::new();
    let mut dropped: usize = 0;
    let mut buf = vec![0u8; 8192];
    loop {
        let n = match stream.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        let chunk = buf.get(..n).unwrap_or_default();
        let room = cap.saturating_sub(head.len()).min(chunk.len());
        let (first, rest) = chunk.split_at(room);
        head.extend_from_slice(first);
        tail.extend(rest);
        if tail.len() > cap {
            let excess = tail.len() - cap;
            tail.drain(..excess);
            dropped += excess;
        }
    }
    if dropped > 0 {
        head.extend_from_slice(format!("\n[{dropped} bytes cut]\n").as_bytes());
    }
    head.extend(tail);
    head
}
