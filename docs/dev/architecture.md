# Architecture

xaku is a headless terminal multiplexer for agent automation. It uses a client-daemon architecture where a single background daemon manages terminal sessions, and a CLI client communicates with it over a Unix domain socket.

## Module Overview

| File | Role |
|------|------|
| `src/main.rs` | CLI client — parses commands via clap, serializes requests, sends them to the daemon |
| `src/daemon.rs` | Background daemon — listens on a Unix socket, manages workspace/surface state, handles requests |
| `src/protocol.rs` | Shared types — `Request` enum (tagged JSON) and `Response` struct used by both client and daemon |
| `src/session.rs` | Terminal session — spawns a PTY child process, feeds output into a vt100 parser, supports input/output |

## Client-Daemon Architecture

```
┌──────────────┐       Unix socket         ┌──────────────────┐
│  xaku CLI    │ ──── JSON + newline ────▸  │  xaku daemon     │
│  (main.rs)   │ ◂──── JSON + newline ──── │  (daemon.rs)     │
└──────────────┘    /tmp/xaku-{uid}.sock    │                  │
                                            │  ┌────────────┐  │
                                            │  │ State       │  │
                                            │  │ - workspaces│  │
                                            │  │ - surfaces  │  │
                                            │  └────────────┘  │
                                            └──────────────────┘
```

The socket path is `/tmp/xaku-{uid}.sock` where `uid` is the current user's UID (`libc::getuid()`).

### Auto-Start Behavior

The `connect()` function in `main.rs` first attempts to connect to the daemon socket. If the connection fails, it spawns a new daemon process (`xaku daemon run`) with stdin/stdout/stderr set to null, then polls the socket up to 20 times (100ms intervals, 2 seconds total) until the daemon is ready. This means any xaku command will transparently start the daemon if it is not already running.

## Daemon State

The daemon runs a tokio async runtime (`daemon::run`). It accepts connections on the Unix socket, deserializes each request with `serde_json`, and dispatches to `State::handle()` inside a `spawn_blocking` task (since state is behind `Arc<Mutex<State>>`).

### `State` struct (`src/daemon.rs`)

| Field | Type | Description |
|-------|------|-------------|
| `workspaces` | `HashMap<u32, Workspace>` | All active workspaces, keyed by ID |
| `surfaces` | `HashMap<u32, Session>` | All active terminal sessions, keyed by ID |
| `next_workspace_id` | `u32` | Auto-incrementing workspace ID counter (starts at 1) |
| `next_surface_id` | `u32` | Auto-incrementing surface ID counter (starts at 1) |
| `active_workspace` | `Option<u32>` | Currently focused workspace |

### `Workspace` struct (`src/daemon.rs`)

| Field | Type | Description |
|-------|------|-------------|
| `id` | `u32` | Workspace identifier |
| `name` | `String` | Display name (default: `workspace-{id}`) |
| `surfaces` | `Vec<u32>` | Ordered list of surface IDs in this workspace |
| `active_surface` | `u32` | Currently focused surface within the workspace |

### `Session` struct (`src/session.rs`)

| Field | Type | Description |
|-------|------|-------------|
| `id` | `u32` | Surface identifier |
| `writer` | `std::fs::File` | PTY master fd for writing input to the terminal |
| `child` | `std::process::Child` | Shell child process |
| `parser` | `Arc<Mutex<vt100::Parser>>` | Virtual terminal parser (50 rows x 200 cols, 10000 line scrollback) |
| `alive` | `Arc<AtomicBool>` | Whether the PTY reader thread is still running |
| `cwd` | `String` | Working directory the session was spawned in |

## Data Flow

A CLI command follows this path:

1. `main.rs`: clap parses arguments into a `Cmd` enum variant
2. `main.rs`: The `Cmd` variant is converted into a `Request` enum variant
3. `main.rs`: `send_request()` serializes the `Request` to JSON, writes it + newline to the Unix socket
4. `daemon.rs`: The tokio listener accepts the connection, reads one JSON line, deserializes to `Request`
5. `daemon.rs`: `State::handle(req)` is called inside `spawn_blocking` (mutex-protected)
6. `daemon.rs`: The handler returns a `Response`, which is serialized to JSON + newline and written back
7. `main.rs`: `send_request()` reads the response line, deserializes to `Response`
8. `main.rs`: `exec()` prints `data` on success or `error` on failure

## PTY Session Lifecycle

When a new surface is created (`Session::spawn` in `src/session.rs`):

1. **PTY allocation**: `nix::pty::openpty()` creates a master/slave PTY pair
2. **Terminal size**: Master fd is configured to 50 rows x 200 columns via `TIOCSWINSZ` ioctl
3. **Shell spawn**: The user's `$SHELL` (default: `/bin/zsh`) is spawned with the slave PTY as stdin/stdout/stderr. `pre_exec` calls `setsid()` and `TIOCSCTTY` to establish a new session with the PTY as controlling terminal.
4. **Reader thread**: A background thread reads from the master fd in 4096-byte chunks and feeds output into the `vt100::Parser`. When the read returns 0 or errors, `alive` is set to false.
5. **Initial command**: If a `command` was specified, it is sent as text followed by an Enter keypress after a 50ms delay.

### Session Drop

When a `Session` is dropped, it kills the child process and waits for it to exit (`child.kill()` + `child.wait()`).

## Surface Resolution

Many commands accept optional `--workspace` and `--surface` flags. The `State::resolve_surface()` method resolves these to a concrete surface ID:

1. If `surface` is provided, use it directly
2. Otherwise, find the workspace (explicit or active) and use its `active_surface`

Reference strings like `workspace:1` or `surface:3` are parsed by `parse_ref()` in `main.rs`, which extracts the trailing integer after the last colon.

## Shutdown

The daemon handles shutdown in two ways:

- **Shutdown request**: Clears all surfaces and workspaces, then spawns a thread that exits the process after 100ms (allowing the response to be sent first)
- **SIGINT (ctrl-c)**: A tokio task listens for ctrl-c, removes the socket file, and exits
