# Commands

Complete reference for all xaku CLI commands.

## Daemon Management

### `xaku daemon run`

Run the daemon in the foreground. Normally you do not need this -- the daemon starts automatically when you run any command.

### `xaku daemon stop`

Stop the background daemon. All workspaces and surfaces are closed.

### `xaku daemon status`

Check if the daemon is running. Prints "Daemon running" or "Daemon not running".

## Workspace Commands

### `xaku new-workspace`

Create a new workspace with a terminal session.

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--cwd` | string | current directory | Working directory for the shell |
| `--command` | string | none | Command to run immediately after spawn |
| `--name` | string | `workspace-{id}` | Display name for the workspace |

Returns: `workspace:{id}` (e.g., `workspace:1`)

### `xaku list-workspaces`

List all workspaces. Returns a JSON array with each workspace's ref, name, surface count, and active status.

### `xaku tree`

Show the workspace/surface hierarchy as a tree.

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--workspace` | string | all | Show only this workspace (e.g., `workspace:1`) |
| `--all` | flag | false | (reserved) |

### `xaku close-workspace`

Close a workspace and all its surfaces.

| Flag | Type | Required | Description |
|------|------|----------|-------------|
| `--workspace` | string | yes | Workspace to close (e.g., `workspace:1`) |

### `xaku rename-workspace`

Rename a workspace.

| Flag | Type | Required | Description |
|------|------|----------|-------------|
| `--workspace` | string | no | Workspace to rename (defaults to active) |

| Argument | Description |
|----------|-------------|
| `TITLE` | New name for the workspace |

### `xaku select-workspace`

Set the active workspace.

| Flag | Type | Required | Description |
|------|------|----------|-------------|
| `--workspace` | string | yes | Workspace to focus (e.g., `workspace:1`) |

### `xaku current-workspace`

Show the currently active workspace. Returns JSON with `ref` and `name`.

## Surface Commands

### `xaku new-surface`

Create a new terminal surface (tab) in a workspace.

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--type` | string | `terminal` | Surface type. Only `terminal` is supported; `browser` returns an error directing to khora. |
| `--pane` | string | none | (reserved) |
| `--workspace` | string | active workspace | Workspace to add the surface to |

Returns: `surface:{id}`

### `xaku new-pane`

Alias for `new-surface`.

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--type` | string | `terminal` | Surface type |
| `--direction` | string | none | (reserved) |
| `--workspace` | string | active workspace | Workspace to add the pane to |

### `xaku close-surface`

Close a single surface.

| Flag | Type | Required | Description |
|------|------|----------|-------------|
| `--surface` | string | yes | Surface to close (e.g., `surface:1`) |

## Input Commands

### `xaku send`

Send text to a terminal. **Does NOT press Enter.** Use `send-key enter` after this command to execute.

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--workspace` | string | active workspace | Target workspace |
| `--surface` | string | active surface | Target surface |

| Argument | Description |
|----------|-------------|
| `TEXT` | Text to send to the terminal |

### `xaku send-key`

Send a special key to a terminal.

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--workspace` | string | active workspace | Target workspace |
| `--surface` | string | active surface | Target surface |

| Argument | Description |
|----------|-------------|
| `KEY` | Key name (see table below) |

#### Supported Keys

| Key | Name(s) | Escape Sequence |
|-----|---------|-----------------|
| Enter | `enter`, `return` | `\r` |
| Tab | `tab` | `\t` |
| Escape | `escape`, `esc` | `\x1b` |
| Backspace | `backspace` | `\x7f` |
| Space | `space` | ` ` |
| Arrow Up | `up` | `\x1b[A` |
| Arrow Down | `down` | `\x1b[B` |
| Arrow Right | `right` | `\x1b[C` |
| Arrow Left | `left` | `\x1b[D` |
| Ctrl-C | `ctrl-c` | `\x03` |
| Ctrl-D | `ctrl-d` | `\x04` |
| Ctrl-Z | `ctrl-z` | `\x1a` |
| Ctrl-L | `ctrl-l` | `\x0c` |
| Ctrl-A | `ctrl-a` | `\x01` |
| Ctrl-E | `ctrl-e` | `\x05` |
| Ctrl-U | `ctrl-u` | `\x15` |
| Ctrl-K | `ctrl-k` | `\x0b` |
| Ctrl-W | `ctrl-w` | `\x17` |
| Ctrl-R | `ctrl-r` | `\x12` |

Key names are case-insensitive.

## Output Commands

### `xaku read-screen`

Read terminal screen content.

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--workspace` | string | active workspace | Target workspace |
| `--surface` | string | active surface | Target surface |
| `--lines` | integer | all | Number of lines to return (from bottom of output) |
| `--scrollback` | flag | false | Include scrollback buffer (reserved, not yet implemented) |

Returns the visible terminal content as a string. Trailing empty lines are stripped.

### `xaku capture-pane`

Alias for `read-screen` (tmux compatibility). Accepts the same flags.

## Utility Commands

### `xaku identify`

Show the current context (active workspace and surface). Returns JSON:

```json
{
  "workspace": "workspace:1",
  "workspace_name": "dev",
  "surface": "surface:1"
}
```

### `xaku ping`

Ping the daemon. Returns `"pong"` if the daemon is running.

## Reference Resolution

Commands that accept `--workspace` and `--surface` use reference strings like `workspace:1` or `surface:3`. The integer after the colon is the ID. If neither flag is provided, the active workspace and its active surface are used.
