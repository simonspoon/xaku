# Protocol

xaku uses a JSON-over-newline protocol for communication between the CLI client and the background daemon over a Unix domain socket.

## Wire Format

Each message (request or response) is a single line of JSON terminated by a newline character (`\n`). The client sends one request per connection and reads one response.

```
Client → Daemon:  {"cmd":"ping"}\n
Daemon → Client:  {"ok":true,"data":"pong"}\n
```

The socket is located at `/tmp/xaku-{uid}.sock` where `uid` is the current user's UID.

## Request

Defined in `src/protocol.rs` as an enum with serde attributes:

```rust
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request { ... }
```

The `tag = "cmd"` attribute means the JSON object uses a `"cmd"` field to identify the variant. The `rename_all = "snake_case"` converts Rust variant names (e.g., `NewWorkspace`) to snake_case in JSON (e.g., `"new_workspace"`).

### Request Variants

| Variant | JSON `cmd` | Fields | Description |
|---------|-----------|--------|-------------|
| `Ping` | `"ping"` | none | Health check |
| `Shutdown` | `"shutdown"` | none | Stop the daemon |
| `NewWorkspace` | `"new_workspace"` | `cwd?: string`, `command?: string`, `name?: string` | Create a workspace with a terminal session |
| `ListWorkspaces` | `"list_workspaces"` | none | List all workspaces |
| `Tree` | `"tree"` | `workspace?: u32` | Show workspace/surface hierarchy |
| `Send` | `"send"` | `workspace?: u32`, `surface?: u32`, `text: string` | Send text to a terminal (no Enter) |
| `SendKey` | `"send_key"` | `workspace?: u32`, `surface?: u32`, `key: string` | Send a special key |
| `ReadScreen` | `"read_screen"` | `workspace?: u32`, `surface?: u32`, `lines?: usize`, `scrollback: bool` | Read terminal screen content |
| `CloseWorkspace` | `"close_workspace"` | `workspace: u32` | Close a workspace and all its surfaces |
| `CloseSurface` | `"close_surface"` | `surface: u32` | Close a single surface |
| `NewSurface` | `"new_surface"` | `workspace?: u32`, `surface_type?: string` | Create a new surface in a workspace |
| `Identify` | `"identify"` | none | Show current workspace/surface context |
| `RenameWorkspace` | `"rename_workspace"` | `workspace: u32`, `name: string` | Rename a workspace |
| `SelectWorkspace` | `"select_workspace"` | `workspace: u32` | Set active workspace |
| `CurrentWorkspace` | `"current_workspace"` | none | Get the active workspace |

Fields marked with `?` are optional (serialized as `Option<T>` in Rust; omitted from JSON when absent).

## Response

Defined in `src/protocol.rs`:

```rust
pub struct Response {
    pub ok: bool,
    pub data: Option<serde_json::Value>,   // skipped if None
    pub error: Option<String>,             // skipped if None
}
```

| Field | Type | Present | Description |
|-------|------|---------|-------------|
| `ok` | `bool` | always | Whether the request succeeded |
| `data` | `Value` | on success (when data exists) | Result payload (type varies by request) |
| `error` | `string` | on failure | Error message |

### Constructors

| Constructor | `ok` | `data` | `error` | Used for |
|-------------|------|--------|---------|----------|
| `Response::ok(value)` | `true` | `Some(value)` | `None` | Success with data |
| `Response::ok_empty()` | `true` | `None` | `None` | Success, no data to return |
| `Response::err(msg)` | `false` | `None` | `Some(msg)` | Failure |

## Example Exchanges

### Ping

```json
→ {"cmd":"ping"}
← {"ok":true,"data":"pong"}
```

### Create a Workspace

```json
→ {"cmd":"new_workspace","cwd":"/home/user/project","name":"dev"}
← {"ok":true,"data":"workspace:1"}
```

### Send Text

```json
→ {"cmd":"send","text":"echo hello"}
← {"ok":true}
```

Note: `send` does NOT append a newline. Use `send_key` with `"enter"` to press Enter.

### Send Key

```json
→ {"cmd":"send_key","key":"enter"}
← {"ok":true}
```

### Read Screen

```json
→ {"cmd":"read_screen","lines":5}
← {"ok":true,"data":"$ echo hello\nhello\n$"}
```

### List Workspaces

```json
→ {"cmd":"list_workspaces"}
← {"ok":true,"data":[{"ref":"workspace:1","name":"dev","surfaces":1,"active":true}]}
```

### Error Response

```json
→ {"cmd":"close_workspace","workspace":99}
← {"ok":false,"error":"Workspace 99 not found"}
```

### Identify

```json
→ {"cmd":"identify"}
← {"ok":true,"data":{"workspace":"workspace:1","workspace_name":"dev","surface":"surface:1"}}
```

### New Surface (Browser -- Unsupported)

```json
→ {"cmd":"new_surface","surface_type":"browser"}
← {"ok":false,"error":"Browser surfaces not supported — use khora for browser automation"}
```

## Workspace and Surface Resolution

For requests that accept optional `workspace` and `surface` fields:

- If `surface` is provided, it is used directly
- If only `workspace` is provided, the workspace's `active_surface` is used
- If neither is provided, the daemon's `active_workspace` is used, then its `active_surface`
- If no workspace is active, the request fails with an error

## Reference Format

Workspaces and surfaces are identified by integer IDs. The CLI uses reference strings like `workspace:1` and `surface:3`, but the protocol uses bare integers. The CLI's `parse_ref()` function extracts the integer from the reference string.
