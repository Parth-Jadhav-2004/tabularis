use super::provider::ProviderError;

const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy)]
pub enum Framing {
    Sse,
    Ndjson,
}

/// Buffers bytes until a complete line, preserving split UTF-8 code points.
pub struct FrameParser {
    framing: Framing,
    line: Vec<u8>,
    data: Vec<String>,
    size: usize,
    after_cr: bool,
}

impl FrameParser {
    pub fn new(framing: Framing) -> Self {
        Self {
            framing,
            line: Vec::new(),
            data: Vec::new(),
            size: 0,
            after_cr: false,
        }
    }

    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, ProviderError> {
        let mut frames = Vec::new();
        for &byte in bytes {
            if self.after_cr && byte == b'\n' {
                self.after_cr = false;
                continue;
            }
            self.after_cr = byte == b'\r';
            if byte == b'\n' || byte == b'\r' {
                self.line(&mut frames)?;
            } else {
                self.line.push(byte);
                if self.line.len() + self.size > MAX_FRAME_BYTES {
                    return Err(ProviderError::protocol());
                }
            }
        }
        Ok(frames)
    }

    pub fn finish(&mut self) -> Result<Vec<String>, ProviderError> {
        let mut frames = Vec::new();
        if !self.line.is_empty() {
            self.line(&mut frames)?;
        }
        self.dispatch(&mut frames);
        Ok(frames)
    }

    fn line(&mut self, frames: &mut Vec<String>) -> Result<(), ProviderError> {
        let bytes = std::mem::take(&mut self.line);
        let line = String::from_utf8(bytes).map_err(|_| ProviderError::protocol())?;
        match self.framing {
            Framing::Ndjson => {
                if !line.trim().is_empty() {
                    frames.push(line);
                }
            }
            Framing::Sse => {
                if line.is_empty() {
                    self.dispatch(frames);
                } else if let Some(data) = line.strip_prefix("data:") {
                    let data = data.strip_prefix(' ').unwrap_or(data);
                    self.size += data.len() + 1;
                    self.data.push(data.to_string());
                }
            }
        }
        Ok(())
    }

    fn dispatch(&mut self, frames: &mut Vec<String>) {
        if !self.data.is_empty() {
            frames.push(self.data.join("\n"));
            self.data.clear();
        }
        self.size = 0;
    }
}
