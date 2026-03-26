# Getting Started

xaku is a headless terminal multiplexer designed for agent automation. It lets programs create, control, and read terminal sessions without a visible UI.

## Installation

### Homebrew (macOS)

```bash
brew install simonspoon/tap/xaku
```

### GitHub Releases

Download the binary for your platform from the [releases page](https://github.com/simonspoon/xaku/releases):

| Platform | Binary |
|----------|--------|
| Linux x86_64 | `xaku-linux-amd64` |
| Linux ARM64 | `xaku-linux-arm64` |
| macOS x86_64 | `xaku-darwin-amd64` |
| macOS ARM64 | `xaku-darwin-arm64` |

```bash
chmod +x xaku-*
mv xaku-* /usr/local/bin/xaku
```

### From Source

Requires Rust (edition 2024):

```bash
git clone https://github.com/simonspoon/xaku.git
cd xaku
cargo install --path .
```

## Quick Start

### 1. Create a Workspace

```bash
xaku new-workspace --name myproject --cwd ~/projects/myapp
```

This creates a workspace with a terminal session. The daemon starts automatically if it is not already running. Output: `workspace:1`.

### 2. Run a Command

xaku's `send` command types text into the terminal but does **not** press Enter. Use `send-key enter` to execute:

```bash
xaku send "echo hello world"
xaku send-key enter
```

### 3. Read the Output

Wait a moment for the command to execute, then read the screen:

```bash
xaku read-screen --lines 5
```

This returns the last 5 lines of terminal output.

### 4. Clean Up

```bash
xaku close-workspace --workspace workspace:1
```

To stop the daemon entirely:

```bash
xaku daemon stop
```

## Key Concepts

### Workspaces

A workspace is a named container for one or more terminal surfaces. When you create a workspace, it automatically gets one terminal surface. You can add more surfaces with `xaku new-surface`.

### Surfaces

A surface is an individual terminal session. Each surface runs a shell process (your `$SHELL`, defaulting to `/bin/zsh`) in a virtual terminal (50 rows x 200 columns). You can send input to a surface and read its screen content.

### The Daemon

xaku runs a background daemon that manages all workspaces and surfaces. The daemon starts automatically when you run any xaku command. It communicates with the CLI over a Unix domain socket at `/tmp/xaku-{uid}.sock`.

You can check its status or manage it directly:

```bash
xaku daemon status   # Check if daemon is running
xaku daemon stop     # Stop the daemon
xaku daemon run      # Run daemon in foreground (for debugging)
```

## Example: Automated Build Check

```bash
# Create a workspace for the build
xaku new-workspace --name build --cwd ~/projects/myapp

# Run the build
xaku send "cargo build 2>&1"
xaku send-key enter

# Wait for build to finish, then check output
sleep 10
xaku read-screen

# Clean up
xaku close-workspace --workspace workspace:1
```
