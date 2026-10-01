//! Keep iTerm2's host metadata aligned with the process owning the terminal.
//!
//! A shell in a full tmux attach can report a different host. On returning
//! to Bosun there is no shell prompt to replace that report, so iTerm2 can
//! keep treating a local session as remote and offer uploads for file drops.

use std::io::Write;

pub(crate) fn restore(writer: &mut impl Write) {
    if std::env::var("TERM_PROGRAM").as_deref() != Ok("iTerm.app") {
        return;
    }
    let (Some(hostname), Ok(username)) = (hostname(), std::env::var("USER")) else {
        return;
    };
    if let Some(sequence) = host_sequence(&username, &hostname) {
        // This is optional terminal metadata; an error must not stop the TUI.
        let _ = writer.write_all(sequence.as_bytes());
        let _ = writer.flush();
    }
}

fn host_sequence(username: &str, hostname: &str) -> Option<String> {
    // Neither field may terminate the OSC or introduce another protocol field.
    // Use the actual hostname, including on SSH hosts; never claim localhost.
    let valid = |s: &str| {
        !s.is_empty()
            && !s
                .chars()
                .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '@' | ';'))
    };
    if !valid(username) || !valid(hostname) {
        return None;
    }
    Some(format!("\x1b]1337;RemoteHost={username}@{hostname}\x07"))
}

#[cfg(unix)]
fn hostname() -> Option<String> {
    let mut buf = [0u8; 256];
    // SAFETY: buf is writable for the supplied length. Check both the return
    // value and NUL termination before interpreting any bytes as a hostname.
    if unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) } != 0 {
        return None;
    }
    let end = buf.iter().position(|&b| b == 0)?;
    String::from_utf8(buf[..end].to_vec()).ok()
}

#[cfg(not(unix))]
fn hostname() -> Option<String> {
    std::env::var("COMPUTERNAME").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_the_real_host_including_remote_hosts() {
        assert_eq!(
            host_sequence("rhuk", "hades-2.local").unwrap(),
            "\x1b]1337;RemoteHost=rhuk@hades-2.local\x07"
        );
        assert_eq!(
            host_sequence("grav", "server.example.com").unwrap(),
            "\x1b]1337;RemoteHost=grav@server.example.com\x07"
        );
    }

    #[test]
    fn invalid_fields_cannot_inject_terminal_commands() {
        for invalid in ["", "a\x07b", "a\x1b]0;title", "a@b", "a;b", "a\nb", "a b"] {
            assert!(host_sequence(invalid, "hades-2.local").is_none());
            assert!(host_sequence("rhuk", invalid).is_none());
        }
    }

    #[cfg(unix)]
    #[test]
    fn reads_a_nonempty_system_hostname() {
        let host = hostname().expect("gethostname should succeed");
        assert!(host_sequence("rhuk", &host).is_some());
    }
}
