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

    pub fn finish_origin(
        &mut self,
        origin: LineOrigin,
    ) -> Option<Result<ParsedLine, ProtocolError>> {
        let buffer = self.buffer_mut(origin);
        if buffer.is_empty() {
            return None;
        }

        let trailing = std::mem::take(buffer);
        Some(decode_line(origin, &trailing))
    }

    pub fn finish(&mut self) -> Vec<Result<ParsedLine, ProtocolError>> {
        let mut trailing = Vec::new();
        if let Some(stdout) = self.finish_origin(LineOrigin::Stdout) {
            trailing.push(stdout);
        }
        if let Some(stderr) = self.finish_origin(LineOrigin::Stderr) {
            trailing.push(stderr);
        }
        trailing
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
