//! `Content-Length`-framed JSON-RPC 2.0.
//!
//! ```text
//! Content-Length: <n>\r\n
//! \r\n
//! <n bytes of UTF-8 JSON>
//! ```

use std::io::{BufRead, Write};

use serde_json::{json, Value};

/// Refuse messages larger than this (a body mesh as JSON is ~1 MB).
pub const MAX_MESSAGE: usize = 64 << 20;

/// Read one frame. `Ok(None)` at clean end of stream.
pub fn read_frame(r: &mut impl BufRead) -> std::io::Result<Option<String>> {
    let mut len = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            if len.is_some() {
                break;
            }
            continue; // tolerate blank lines between frames
        }
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                len = Some(
                    v.trim()
                        .parse::<usize>()
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?,
                );
            }
        }
    }
    let n = len.unwrap_or(0);
    if n > MAX_MESSAGE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "message too large",
        ));
    }
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf)?;
    String::from_utf8(buf)
        .map(Some)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

pub fn write_frame(w: &mut impl Write, body: &str) -> std::io::Result<()> {
    write!(w, "Content-Length: {}\r\n\r\n", body.len())?;
    w.write_all(body.as_bytes())?;
    w.flush()
}

/// A parsed request from a plugin.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// `None` for notifications (no reply expected).
    pub id: Option<Value>,
    pub method: String,
    pub params: Value,
}

pub fn parse_request(body: &str) -> Result<Request, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| format!("invalid JSON: {e}"))?;
    let method = v
        .get("method")
        .and_then(|m| m.as_str())
        .ok_or("missing method")?
        .to_owned();
    Ok(Request {
        id: v.get("id").cloned(),
        method,
        params: v.get("params").cloned().unwrap_or(json!({})),
    })
}

pub fn response(id: &Value, result: Result<Value, crate::RpcError>) -> String {
    match result {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err(e) => {
            json!({ "jsonrpc": "2.0", "id": id, "error": { "code": e.code, "message": e.message } })
        }
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_and_parse() {
        let mut buf = Vec::new();
        write_frame(&mut buf, r#"{"jsonrpc":"2.0","id":1,"method":"info"}"#).unwrap();
        write_frame(
            &mut buf,
            r#"{"jsonrpc":"2.0","method":"log","params":{"text":"é"}}"#,
        )
        .unwrap();
        let mut r = std::io::BufReader::new(buf.as_slice());
        let a = parse_request(&read_frame(&mut r).unwrap().unwrap()).unwrap();
        assert_eq!((a.id, a.method.as_str()), (Some(json!(1)), "info"));
        let b = parse_request(&read_frame(&mut r).unwrap().unwrap()).unwrap();
        assert!(b.id.is_none());
        assert_eq!(b.params["text"], "é");
        assert!(read_frame(&mut r).unwrap().is_none());
        assert!(parse_request("{}").is_err());
    }
}
