use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use clap::{Parser, Subcommand};

mod daemon;
mod protocol;
mod session;

use protocol::{Request, Response};

fn socket_path() -> PathBuf {
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/xaku-{}.sock", uid))
}

fn connect() -> anyhow::Result<UnixStream> {
    let path = socket_path();
    if let Ok(s) = UnixStream::connect(&path) {
        return Ok(s);
    }
    // Auto-start daemon
    let exe = std::env::current_exe()?;
    Command::new(exe)
        .args(["daemon", "run"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(100));
        if let Ok(s) = UnixStream::connect(&path) {
            return Ok(s);
        }
    }
    anyhow::bail!("Failed to connect to xaku daemon")
}

fn send_request(req: &Request) -> anyhow::Result<Response> {
    let mut stream = connect()?;
    let json = serde_json::to_string(req)?;
    stream.write_all(json.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(serde_json::from_str(&line)?)
}

fn parse_ref(s: &str) -> u32 {
    s.split(':')
        .next_back()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

fn exec(req: Request, print_data: bool) -> anyhow::Result<()> {
    let resp = send_request(&req)?;
    if resp.ok {
        if print_data && let Some(data) = resp.data {
            match data {
                serde_json::Value::String(s) => println!("{}", s),
                other => println!("{}", serde_json::to_string_pretty(&other)?),
            }
        }
        Ok(())
    } else {
        eprintln!("Error: {}", resp.error.unwrap_or_default());
        std::process::exit(1);
    }
}

#[derive(Parser)]
#[command(
    name = "xaku",
    about = "Headless terminal multiplexer for agent automation"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run or manage the background daemon
    Daemon {
        #[command(subcommand)]
        action: DaemonCmd,
    },
    /// Create a new workspace with a terminal session
    #[command(name = "new-workspace")]
    NewWorkspace {
        #[arg(long)]
        cwd: Option<String>,
        #[arg(long)]
        command: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    /// List all workspaces
    #[command(name = "list-workspaces")]
    ListWorkspaces,
    /// Show workspace tree
    Tree {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        all: bool,
    },
    /// Send text to a terminal (does NOT press Enter)
    #[command(name = "send")]
    SendText {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        surface: Option<String>,
        text: String,
    },
    /// Send a special key
    #[command(name = "send-key")]
    SendKey {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        surface: Option<String>,
        key: String,
    },
    /// Read terminal screen content
    #[command(name = "read-screen")]
    ReadScreen {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        surface: Option<String>,
        #[arg(long)]
        lines: Option<usize>,
        #[arg(long)]
        scrollback: bool,
    },
    /// Alias for read-screen (tmux compat)
    #[command(name = "capture-pane")]
    CapturePan {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        surface: Option<String>,
        #[arg(long)]
        scrollback: bool,
        #[arg(long)]
        lines: Option<usize>,
    },
    /// Close a workspace and all its surfaces
    #[command(name = "close-workspace")]
    CloseWorkspace {
        #[arg(long)]
        workspace: String,
    },
    /// Close a single surface
    #[command(name = "close-surface")]
    CloseSurface {
        #[arg(long)]
        surface: String,
    },
    /// Create a new surface (tab) in a workspace
    #[command(name = "new-surface")]
    NewSurface {
        #[arg(long = "type")]
        kind: Option<String>,
        #[arg(long)]
        pane: Option<String>,
        #[arg(long)]
        workspace: Option<String>,
    },
    /// Create a new pane (alias for new-surface)
    #[command(name = "new-pane")]
    NewPane {
        #[arg(long = "type")]
        kind: Option<String>,
        #[arg(long)]
        direction: Option<String>,
        #[arg(long)]
        workspace: Option<String>,
    },
    /// Show current context
    Identify,
    /// Ping the daemon
    Ping,
    /// Rename a workspace
    #[command(name = "rename-workspace")]
    RenameWorkspace {
        #[arg(long)]
        workspace: Option<String>,
        title: String,
    },
    /// Focus a workspace
    #[command(name = "select-workspace")]
    SelectWorkspace {
        #[arg(long)]
        workspace: String,
    },
    /// Show current workspace
    #[command(name = "current-workspace")]
    CurrentWorkspace,
}

#[derive(Subcommand)]
enum DaemonCmd {
    /// Run daemon in foreground
    Run,
    /// Stop the daemon
    Stop,
    /// Check daemon status
    Status,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Cmd::Daemon { action } => match action {
            DaemonCmd::Run => daemon::run(socket_path()),
            DaemonCmd::Stop => {
                send_request(&Request::Shutdown)?;
                println!("Daemon stopped");
                Ok(())
            }
            DaemonCmd::Status => {
                match send_request(&Request::Ping) {
                    Ok(r) if r.ok => println!("Daemon running"),
                    _ => println!("Daemon not running"),
                }
                Ok(())
            }
        },

        Cmd::NewWorkspace { cwd, command, name } => {
            exec(Request::NewWorkspace { cwd, command, name }, true)
        }

        Cmd::ListWorkspaces => exec(Request::ListWorkspaces, true),

        Cmd::Tree { workspace, .. } => exec(
            Request::Tree {
                workspace: workspace.map(|w| parse_ref(&w)),
            },
            true,
        ),

        Cmd::SendText {
            workspace,
            surface,
            text,
        } => exec(
            Request::Send {
                workspace: workspace.map(|w| parse_ref(&w)),
                surface: surface.map(|s| parse_ref(&s)),
                text,
            },
            false,
        ),

        Cmd::SendKey {
            workspace,
            surface,
            key,
        } => exec(
            Request::SendKey {
                workspace: workspace.map(|w| parse_ref(&w)),
                surface: surface.map(|s| parse_ref(&s)),
                key,
            },
            false,
        ),

        Cmd::ReadScreen {
            workspace,
            surface,
            lines,
            scrollback,
        }
        | Cmd::CapturePan {
            workspace,
            surface,
            lines,
            scrollback,
        } => exec(
            Request::ReadScreen {
                workspace: workspace.map(|w| parse_ref(&w)),
                surface: surface.map(|s| parse_ref(&s)),
                lines,
                scrollback,
            },
            true,
        ),

        Cmd::CloseWorkspace { workspace } => exec(
            Request::CloseWorkspace {
                workspace: parse_ref(&workspace),
            },
            false,
        ),

        Cmd::CloseSurface { surface } => exec(
            Request::CloseSurface {
                surface: parse_ref(&surface),
            },
            false,
        ),

        Cmd::NewSurface {
            kind, workspace, ..
        }
        | Cmd::NewPane {
            kind, workspace, ..
        } => exec(
            Request::NewSurface {
                workspace: workspace.map(|w| parse_ref(&w)),
                surface_type: kind,
            },
            true,
        ),

        Cmd::Identify => exec(Request::Identify, true),
        Cmd::Ping => exec(Request::Ping, true),

        Cmd::RenameWorkspace { workspace, title } => {
            let wid = workspace.map(|w| parse_ref(&w)).unwrap_or(0);
            exec(
                Request::RenameWorkspace {
                    workspace: wid,
                    name: title,
                },
                false,
            )
        }

        Cmd::SelectWorkspace { workspace } => exec(
            Request::SelectWorkspace {
                workspace: parse_ref(&workspace),
            },
            false,
        ),

        Cmd::CurrentWorkspace => exec(Request::CurrentWorkspace, true),
    }
}
