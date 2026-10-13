//! Ollama's local HTTP API (127.0.0.1 only).

use super::*;

const OLLAMA_ADDR: SocketAddr = SocketAddr::new(
    std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
    11434,
);

/// Model names the local Ollama API accepts. Letters, digits, and `: / . _ - @` only.
/// No `..`, so a name can't be a path.
pub(crate) fn valid_ollama_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 256 || name.contains("..") {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '/' | '.' | '_' | '-' | '@'))
}

/// True when something accepts TCP on 127.0.0.1:11434. The connection is closed immediately.
pub(crate) fn ollama_reachable() -> bool {
    TcpStream::connect_timeout(&OLLAMA_ADDR, Duration::from_millis(300)).is_ok()
}

/// GET one allowlisted path. Read-only.
pub(crate) fn ollama_get(path: &str) -> io::Result<String> {
    let (status, body) = ollama_http("GET", path, None, 800)?;
    if (200..300).contains(&status) {
        Ok(body)
    } else {
        Err(io::Error::other(format!("ollama replied {status}")))
    }
}

/// POST `/api/delete`, the request `ollama rm` sends. The name is checked first.
pub fn ollama_delete(name: &str) -> io::Result<()> {
    ollama_post("/api/delete", &format!(r#"{{"name":"{name}"}}"#), name)
}

/// POST `/api/generate` with `keep_alive: 0`, the request `ollama stop` sends.
pub fn ollama_stop(name: &str) -> io::Result<()> {
    ollama_post(
        "/api/generate",
        &format!(r#"{{"model":"{name}","keep_alive":0}}"#),
        name,
    )
}

fn ollama_post(path: &str, body: &str, name: &str) -> io::Result<()> {
    if !valid_ollama_name(name) {
        return Err(io::Error::other("that isn't a model name"));
    }
    let (status, text) = ollama_http("POST", path, Some(body.as_bytes()), 20_000)?;
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(io::Error::other(format!("ollama replied {status}: {text}")))
    }
}

fn ollama_http(
    method: &str,
    path: &str,
    body: Option<&[u8]>,
    read_ms: u64,
) -> io::Result<(u16, String)> {
    if method != "GET" && method != "POST" {
        return Err(io::Error::other("that request isn't allowed"));
    }
    if !matches!(
        path,
        "/api/ps" | "/api/delete" | "/api/generate" | "/api/version"
    ) {
        return Err(io::Error::other("that request isn't allowed"));
    }
    let mut stream = TcpStream::connect_timeout(&OLLAMA_ADDR, Duration::from_millis(300))?;
    stream.set_read_timeout(Some(Duration::from_millis(read_ms)))?;
    stream.set_write_timeout(Some(Duration::from_millis(300)))?;
    let mut req =
        format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:11434\r\nConnection: close\r\n");
    if let Some(bytes) = body {
        req.push_str("Content-Type: application/json\r\n");
        req.push_str(&format!("Content-Length: {}\r\n", bytes.len()));
    }
    req.push_str("\r\n");
    stream.write_all(req.as_bytes())?;
    if let Some(bytes) = body {
        stream.write_all(bytes)?;
    }
    let raw = read_http(&mut stream)?;
    let (head, rest) = raw
        .split_once("\r\n\r\n")
        .ok_or_else(|| io::Error::other("ollama sent no headers"))?;
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    Ok((status, rest.to_string()))
}

/// Reads until the server closes or the timeout. A timeout after headers is the reply.
fn read_http(stream: &mut TcpStream) -> io::Result<String> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        if buf.len() > 8_000_000 {
            break;
        }
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(e) => {
                let headers = buf.windows(4).any(|w| w == b"\r\n\r\n");
                if headers {
                    break;
                }
                if buf.is_empty() {
                    return Err(e);
                }
                break;
            }
        }
    }
    if buf.is_empty() {
        return Err(io::Error::other("empty reply"));
    }
    String::from_utf8(buf).map_err(|_| io::Error::other("ollama sent bytes that aren't text"))
}
