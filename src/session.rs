use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
use std::os::unix::process::CommandExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::Result;

pub struct Session {
    #[allow(dead_code)]
    pub id: u32,
    writer: std::fs::File,
    child: std::process::Child,
    parser: Arc<Mutex<vt100::Parser>>,
    alive: Arc<AtomicBool>,
    pub cwd: String,
}

impl Session {
    pub fn spawn(id: u32, cwd: &str, command: Option<&str>) -> Result<Self> {
        let pty = nix::pty::openpty(None, None)?;
        let master_raw = pty.master.as_raw_fd();
        let slave_raw = pty.slave.as_raw_fd();

        // Set terminal size: 50 rows x 200 cols
        unsafe {
            let ws = libc::winsize {
                ws_row: 50,
                ws_col: 200,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            libc::ioctl(master_raw, libc::TIOCSWINSZ, &ws);
        }

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());

        let mut cmd = std::process::Command::new(&shell);
        cmd.current_dir(cwd);

        // Set PTY slave as stdin/stdout/stderr
        unsafe {
            use std::process::Stdio;
            cmd.stdin(Stdio::from_raw_fd(libc::dup(slave_raw)));
            cmd.stdout(Stdio::from_raw_fd(libc::dup(slave_raw)));
            cmd.stderr(Stdio::from_raw_fd(libc::dup(slave_raw)));
        }

        // New session + controlling terminal
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                libc::ioctl(0, libc::TIOCSCTTY as libc::c_ulong, 0);
                Ok(())
            });
        }

        let child = cmd.spawn()?;
        drop(pty.slave);

        // Master fd: one for writing, one dup for the reader thread
        let master_file = unsafe { std::fs::File::from_raw_fd(pty.master.into_raw_fd()) };
        let reader_file = master_file.try_clone()?;

        let parser = Arc::new(Mutex::new(vt100::Parser::new(50, 200, 10000)));
        let alive = Arc::new(AtomicBool::new(true));

        // Background reader thread: feeds PTY output into vt100 parser
        let p = parser.clone();
        let a = alive.clone();
        std::thread::spawn(move || {
            let mut reader = reader_file;
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => {
                        a.store(false, Ordering::SeqCst);
                        break;
                    }
                    Ok(n) => {
                        if let Ok(mut parser) = p.lock() {
                            parser.process(&buf[..n]);
                        }
                    }
                }
            }
        });

        let mut session = Session {
            id,
            writer: master_file,
            child,
            parser,
            alive,
            cwd: cwd.to_string(),
        };

        // Send initial command if provided
        if let Some(c) = command {
            std::thread::sleep(std::time::Duration::from_millis(50));
            session.send_text(c)?;
            session.send_key("enter")?;
        }

        Ok(session)
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    pub fn send_text(&mut self, text: &str) -> Result<()> {
        use std::io::Write;
        self.writer.write_all(text.as_bytes())?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn send_key(&mut self, key: &str) -> Result<()> {
        let bytes: &[u8] = match key.to_lowercase().as_str() {
            "enter" | "return" => b"\r",
            "tab" => b"\t",
            "escape" | "esc" => b"\x1b",
            "backspace" => b"\x7f",
            "space" => b" ",
            "up" => b"\x1b[A",
            "down" => b"\x1b[B",
            "right" => b"\x1b[C",
            "left" => b"\x1b[D",
            "ctrl-c" => b"\x03",
            "ctrl-d" => b"\x04",
            "ctrl-z" => b"\x1a",
            "ctrl-l" => b"\x0c",
            "ctrl-a" => b"\x01",
            "ctrl-e" => b"\x05",
            "ctrl-u" => b"\x15",
            "ctrl-k" => b"\x0b",
            "ctrl-w" => b"\x17",
            "ctrl-r" => b"\x12",
            other => anyhow::bail!("Unknown key: {}", other),
        };
        use std::io::Write;
        self.writer.write_all(bytes)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn read_screen(&self, lines: Option<usize>, _scrollback: bool) -> String {
        let parser = self.parser.lock().unwrap();
        let screen = parser.screen();
        let contents = screen.contents();

        // Strip trailing empty lines (vt100 buffer pads to full screen height)
        let all_lines: Vec<&str> = contents.lines().collect();
        let last_nonempty = all_lines
            .iter()
            .rposition(|l| !l.is_empty())
            .map(|i| i + 1)
            .unwrap_or(0);
        let trimmed = &all_lines[..last_nonempty];

        if let Some(n) = lines {
            let start = trimmed.len().saturating_sub(n);
            trimmed[start..].join("\n")
        } else {
            trimmed.join("\n")
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
