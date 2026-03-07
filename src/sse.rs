use serde_json::Value;

#[derive(Debug)]
pub enum SseEvent {
    ContentBlockDelta { text: String },
    MessageStop,
    Other,
}

pub struct SseParser {
    buffer: String,
    current_event_type: Option<String>,
    current_data: Vec<String>,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            current_event_type: None,
            current_data: Vec::new(),
        }
    }

    /// Feed a chunk of bytes into the parser.
    /// Returns any complete SSE events parsed from the accumulated data.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        let chunk_str = String::from_utf8_lossy(chunk);
        self.buffer.push_str(&chunk_str);

        let mut events = Vec::new();

        loop {
            if let Some(newline_pos) = self.buffer.find('\n') {
                let line = self.buffer[..newline_pos].trim_end_matches('\r').to_string();
                self.buffer = self.buffer[newline_pos + 1..].to_string();
                self.process_line(&line, &mut events);
            } else {
                break;
            }
        }

        events
    }

    fn process_line(&mut self, line: &str, events: &mut Vec<SseEvent>) {
        if line.is_empty() {
            // Empty line = event boundary
            if let Some(event) = self.emit_event() {
                events.push(event);
            }
            self.current_event_type = None;
            self.current_data.clear();
        } else if let Some(value) = line.strip_prefix("event: ") {
            self.current_event_type = Some(value.to_string());
        } else if line == "event:" {
            self.current_event_type = Some(String::new());
        } else if let Some(value) = line.strip_prefix("data: ") {
            self.current_data.push(value.to_string());
        } else if line == "data:" {
            self.current_data.push(String::new());
        }
        // Ignore id:, retry:, and comment lines (starting with :)
    }

    fn emit_event(&self) -> Option<SseEvent> {
        if self.current_data.is_empty() {
            return None;
        }

        let data = self.current_data.join("\n");
        let event_type = self
            .current_event_type
            .as_deref()
            .unwrap_or("message");

        match event_type {
            "content_block_delta" => {
                if let Ok(json) = serde_json::from_str::<Value>(&data) {
                    if let Some(delta) = json.get("delta") {
                        let delta_type = delta.get("type").and_then(|t| t.as_str());
                        match delta_type {
                            Some("text_delta") => {
                                if let Some(text) =
                                    delta.get("text").and_then(|t| t.as_str())
                                {
                                    return Some(SseEvent::ContentBlockDelta {
                                        text: text.to_string(),
                                    });
                                }
                            }
                            Some("thinking_delta") => {
                                if let Some(thinking) =
                                    delta.get("thinking").and_then(|t| t.as_str())
                                {
                                    return Some(SseEvent::ContentBlockDelta {
                                        text: thinking.to_string(),
                                    });
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Some(SseEvent::Other)
            }
            "message_stop" => Some(SseEvent::MessageStop),
            _ => Some(SseEvent::Other),
        }
    }
}
