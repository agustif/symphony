use crate::{LineOrigin, ParsedLine, ProtocolError, decode_line};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StreamLineParser {
    stdout_buffer: String,
    stderr_buffer: String,
}

impl StreamLineParser {
    pub fn push_chunk(
        &mut self,
        origin: LineOrigin,
        chunk: &str,
    ) -> Vec<Result<ParsedLine, ProtocolError>> {
        if chunk.is_empty() {
            return Vec::new();
        }

        let buffer = self.buffer_mut(origin);
        buffer.push_str(chunk);

        drain_completed_lines(buffer)
            .into_iter()
            .map(|line| decode_line(origin, &line))
            .collect()
    }

    pub fn pending_stdout(&self) -> &str {
        self.stdout_buffer.as_str()
    }

    pub fn pending_stderr(&self) -> &str {
        self.stderr_buffer.as_str()
    }

    fn buffer_mut(&mut self, origin: LineOrigin) -> &mut String {
        match origin {
            LineOrigin::Stdout => &mut self.stdout_buffer,
            LineOrigin::Stderr => &mut self.stderr_buffer,
        }
    }
}

fn drain_completed_lines(buffer: &mut String) -> Vec<String> {
    let mut lines = Vec::new();

    while let Some(newline_index) = buffer.find('\n') {
        lines.push(buffer.drain(..=newline_index).collect::<String>());
    }

    lines
}
