//! Server-Sent Events 解析：三家大模型的流式接口都用 SSE。
//!
//! 网络分块可能切在任意字节上（包括多字节 UTF-8 字符和 `\r\n` 中间），
//! 所以只在拿到完整的一行之后才解码。

/// 一个 SSE 事件。`event` 没有写时为空字符串。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
}

#[derive(Debug, Default)]
pub struct SseParser {
    /// 还没凑成完整一行的字节
    line: Vec<u8>,
    /// 上一块是否以 `\r` 结尾（下一块开头的 `\n` 要吞掉）
    after_cr: bool,
    event: String,
    data: Vec<String>,
}

impl SseParser {
    /// 喂入一块字节，返回其中已经完整的事件。
    pub fn push(&mut self, bytes: &[u8]) -> Vec<SseEvent> {
        let mut out = Vec::new();
        for &b in bytes {
            if self.after_cr {
                self.after_cr = false;
                if b == b'\n' {
                    continue;
                }
            }
            match b {
                b'\n' => self.end_line(&mut out),
                b'\r' => {
                    self.end_line(&mut out);
                    self.after_cr = true;
                }
                _ => self.line.push(b),
            }
        }
        out
    }

    /// 流结束：把没有以空行收尾的最后一个事件也交出来。
    pub fn finish(&mut self) -> Vec<SseEvent> {
        let mut out = Vec::new();
        if !self.line.is_empty() {
            self.end_line(&mut out);
        }
        self.dispatch(&mut out);
        out
    }

    fn end_line(&mut self, out: &mut Vec<SseEvent>) {
        let line = String::from_utf8_lossy(&std::mem::take(&mut self.line)).into_owned();
        if line.is_empty() {
            self.dispatch(out);
            return;
        }
        if line.starts_with(':') {
            return; // 注释 / 心跳
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line.as_str(), ""),
        };
        match field {
            "event" => self.event = value.to_string(),
            "data" => self.data.push(value.to_string()),
            _ => {} // id、retry 用不到
        }
    }

    fn dispatch(&mut self, out: &mut Vec<SseEvent>) {
        let event = std::mem::take(&mut self.event);
        if self.data.is_empty() {
            return;
        }
        let data = std::mem::take(&mut self.data).join("\n");
        out.push(SseEvent { event, data });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(event: &str, data: &str) -> SseEvent {
        SseEvent { event: event.into(), data: data.into() }
    }

    #[test]
    fn parses_events_split_at_every_byte() {
        let raw = ": ping\r\nevent: a\r\ndata: {\"t\":\"你好\"}\r\n\r\ndata: x\ndata: y\n\nid: 3\ndata:no-space\n\ndata: tail";
        let want = vec![ev("a", "{\"t\":\"你好\"}"), ev("", "x\ny"), ev("", "no-space"), ev("", "tail")];
        // 整块
        let mut p = SseParser::default();
        let mut got = p.push(raw.as_bytes());
        got.extend(p.finish());
        assert_eq!(got, want);
        // 每个字节单独一块（会切开中文和 \r\n）
        let mut p = SseParser::default();
        let mut got = Vec::new();
        for b in raw.as_bytes() {
            got.extend(p.push(std::slice::from_ref(b)));
        }
        got.extend(p.finish());
        assert_eq!(got, want);
    }

    #[test]
    fn event_without_data_is_dropped_and_event_name_resets() {
        let mut p = SseParser::default();
        assert_eq!(p.push(b"event: ping\n\ndata: 1\n\n"), vec![ev("", "1")]);
        assert!(p.finish().is_empty());
    }
}
