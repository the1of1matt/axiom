//! Thin cross-platform helpers. Keep OS differences here, not scattered.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;

/// User home directory (cross-platform via `dirs`).
pub fn home_dir() -> Option<PathBuf> {
    dirs::home_dir().or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

/// Axiom root: ~/.axiom or %USERPROFILE%\.axiom
pub fn axiom_home() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".axiom"))
}

pub fn axiom_tmp() -> Option<PathBuf> {
    axiom_home().map(|h| h.join("tmp"))
}

pub fn axiom_cache() -> Option<PathBuf> {
    axiom_home().map(|h| h.join("cache"))
}

/// OS token for fingerprints (stable, explicit).
pub fn os_token() -> &'static str {
    std::env::consts::OS
}

pub fn arch_token() -> &'static str {
    std::env::consts::ARCH
}

/// Resolve a command name to something Command::new can run on this OS.
/// On Windows, prefer `.cmd` shims for npm/npx/yarn/pnpm when present on PATH.
pub fn resolve_command(name: &str) -> String {
    #[cfg(windows)]
    {
        // npm/yarn/pnpm ship as .cmd wrappers on Windows
        if matches!(name, "npm" | "npx" | "yarn" | "pnpm") {
            return format!("{}.cmd", name);
        }
    }
    name.to_string()
}

/// Spawn a simple argv command (no shell). Applies Windows command resolution.
pub fn command(name: &str) -> Command {
    Command::new(resolve_command(name))
}

/// Recursively copy a directory (works on Windows; no `cp -a`).
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if dst.exists() {
        let _ = fs::remove_dir_all(dst);
    }
    fs::create_dir_all(dst)?;
    copy_dir_inner(src, dst)
}

fn copy_dir_inner(src: &Path, dst: &Path) -> std::io::Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            fs::create_dir_all(&to)?;
            copy_dir_inner(&entry.path(), &to)?;
        } else if ty.is_symlink() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::symlink;
                if let Ok(target) = fs::read_link(entry.path()) {
                    let _ = symlink(target, &to);
                }
            }
            #[cfg(windows)]
            {
                // On Windows, copy the target file if possible
                match fs::read_link(entry.path()) {
                    Ok(target) => {
                        let abs = if target.is_absolute() {
                            target
                        } else {
                            entry.path().parent().unwrap_or(src).join(target)
                        };
                        if abs.is_dir() {
                            fs::create_dir_all(&to)?;
                            let _ = copy_dir_inner(&abs, &to);
                        } else if abs.exists() {
                            let _ = fs::copy(&abs, &to);
                        }
                    }
                    Err(_) => {
                        let _ = fs::copy(entry.path(), &to);
                    }
                }
            }
        } else {
            fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

/// Atomic-ish write of a marker file after directory is fully populated.
pub fn write_marker(path: &Path, contents: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, path)?;
    Ok(())
}


/// Minimal HTTP/1.1 request over TCP (no curl dependency).
/// Returns (status_code, content_type, body).
pub fn http_request(method: &str, url: &str, timeout_ms: u64) -> Option<(u16, String, String)> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let url = url.strip_prefix("http://")?;
    let (hostport, path) = match url.find('/') {
        Some(i) => (&url[..i], &url[i..]),
        None => (url, "/"),
    };
    let (host, port) = if let Some(i) = hostport.rfind(':') {
        let p: u16 = hostport[i + 1..].parse().ok()?;
        (&hostport[..i], p)
    } else {
        (hostport, 80)
    };

    let mut stream = TcpStream::connect_timeout(
        &format!("{}:{}", host, port).parse().ok()?,
        Duration::from_millis(timeout_ms),
    )
    .ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(timeout_ms)))
        .ok()?;
    stream
        .set_write_timeout(Some(Duration::from_millis(timeout_ms)))
        .ok()?;

    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nUser-Agent: axiom\r\nAccept: */*\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).ok()?;

    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if buf.len() > 512_000 {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let text = String::from_utf8_lossy(&buf);
    let (header, body) = match text.find("\r\n\r\n") {
        Some(i) => (&text[..i], text[i + 4..].to_string()),
        None => (text.as_ref(), String::new()),
    };
    let status = header
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let mut content_type = String::new();
    for line in header.lines().skip(1) {
        let lower = line.to_lowercase();
        if let Some(rest) = lower.strip_prefix("content-type:") {
            content_type = rest.trim().to_string();
            // recover original casing value
            if let Some(pos) = line.find(':') {
                content_type = line[pos + 1..].trim().to_string();
            }
            break;
        }
    }
    if status == 0 {
        return None;
    }
    Some((status, content_type, body))
}


/// Command line to run a shell script on the current OS.
pub fn shell_script_cmd(script: &Path) -> String {
    #[cfg(windows)]
    {
        // Prefer cmd for .bat/.cmd; otherwise try powershell
        let s = script.display().to_string();
        if s.ends_with(".bat") || s.ends_with(".cmd") {
            format!("cmd /C \"{}\"", s)
        } else {
            format!("cmd /C \"{}\"", s)
        }
    }
    #[cfg(not(windows))]
    {
        format!("sh {}", script.display())
    }
}

/// Split a command string into program + args in a platform-tolerant way.
/// Does not invoke a shell unless the command is clearly a shell builtin form.
pub fn parse_command_line(cmd: &str) -> (String, Vec<String>) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return (String::new(), vec![]);
    }
    let prog = if matches!(parts[0], "npm" | "npx" | "yarn" | "pnpm") {
        resolve_command(parts[0])
    } else {
        parts[0].to_string()
    };
    let args = parts[1..].iter().map(|s| s.to_string()).collect();
    (prog, args)
}
