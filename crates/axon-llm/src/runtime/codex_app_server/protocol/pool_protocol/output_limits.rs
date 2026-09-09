use std::io;
use tokio::io::{AsyncBufRead, AsyncBufReadExt};

const FRAME_LIMIT: usize = 1024 * 1024;
const OUTPUT_LIMIT: usize = 16 * 1024 * 1024;

fn exceeded() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "provider.output_limit: Codex output exceeded its byte limit",
    )
}

pub(super) fn accept(total: &mut usize, len: usize) -> io::Result<()> {
    if len > FRAME_LIMIT || len > OUTPUT_LIMIT.saturating_sub(*total) {
        return Err(exceeded());
    }
    *total += len;
    Ok(())
}

/// Bound allocation before copying buffered subprocess bytes, including lines
/// without terminators. The counter spans every message in this handshake.
pub(super) async fn next_line<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    total: &mut usize,
) -> io::Result<Option<String>> {
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            if line.is_empty() {
                return Ok(None);
            }
            break;
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |position| position + 1);
        if take > FRAME_LIMIT.saturating_sub(line.len()) {
            return Err(exceeded());
        }
        accept(total, take)?;
        line.extend_from_slice(&available[..take]);
        reader.consume(take);
        if newline.is_some() {
            break;
        }
    }
    String::from_utf8(line)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}
