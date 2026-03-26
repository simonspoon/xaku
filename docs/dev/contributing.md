# Contributing

## Build

xaku is a single Rust binary. Rust edition 2024.

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Install locally
cargo install --path .
```

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `tokio` | 1 (full features) | Async runtime for the daemon's Unix socket listener |
| `clap` | 4 (derive feature) | CLI argument parsing via derive macros |
| `serde` | 1 (derive feature) | Serialization/deserialization for protocol types |
| `serde_json` | 1 | JSON encoding/decoding for the wire protocol |
| `nix` | 0.29 (term, signal) | PTY allocation (`openpty`), terminal ioctls, signal handling |
| `vt100` | 0.15 | Virtual terminal emulator -- parses ANSI escape sequences |
| `anyhow` | 1 | Error handling with context |
| `libc` | 0.2 | Low-level system calls (`getuid`, `winsize`, `setsid`, `ioctl`) |

## Project Structure

```
src/
  main.rs       CLI client (clap parsing, request dispatch)
  daemon.rs     Background daemon (state management, request handling)
  protocol.rs   Shared types (Request enum, Response struct)
  session.rs    PTY terminal session (spawn, input, screen reading)
```

## Adding a New CLI Command

Adding a command touches three files. Here is the complete path:

### 1. Add the CLI variant (`src/main.rs`)

Add a new variant to the `Cmd` enum with clap attributes:

```rust
#[derive(Subcommand)]
enum Cmd {
    // ... existing commands ...

    /// Description for --help
    #[command(name = "my-command")]
    MyCommand {
        #[arg(long)]
        some_flag: Option<String>,
    },
}
```

### 2. Add the protocol variant (`src/protocol.rs`)

Add a matching variant to the `Request` enum:

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    // ... existing variants ...
    MyCommand {
        some_flag: Option<String>,
    },
}
```

### 3. Add the handler (`src/daemon.rs`)

Add a match arm in `State::handle()`:

```rust
Request::MyCommand { some_flag } => {
    // Implement the logic
    Response::ok(json!("result"))
}
```

### 4. Wire up the match (`src/main.rs`)

Add a match arm in the `main()` function to convert the `Cmd` variant to a `Request` and call `exec()`:

```rust
Cmd::MyCommand { some_flag } => {
    exec(Request::MyCommand { some_flag }, true)
}
```

The second argument to `exec()` controls whether `data` is printed on success (`true` = print, `false` = silent).

## Adding a New Key Binding

Add a match arm in `Session::send_key()` in `src/session.rs`:

```rust
fn send_key(&mut self, key: &str) -> Result<()> {
    let bytes: &[u8] = match key.to_lowercase().as_str() {
        // ... existing keys ...
        "my-key" => b"\x1b[...",  // the escape sequence
        other => anyhow::bail!("Unknown key: {}", other),
    };
    // ...
}
```

Currently supported keys: `enter`, `tab`, `escape`, `backspace`, `space`, `up`, `down`, `right`, `left`, `ctrl-c`, `ctrl-d`, `ctrl-z`, `ctrl-l`, `ctrl-a`, `ctrl-e`, `ctrl-u`, `ctrl-k`, `ctrl-w`, `ctrl-r`.

## Testing

There is no test suite yet. To manually test:

```bash
# Start fresh (kill any existing daemon)
xaku daemon stop

# Create a workspace and interact with it
xaku new-workspace --name test
xaku send "echo hello"
xaku send-key enter
sleep 0.5
xaku read-screen --lines 5
xaku close-workspace --workspace workspace:1
```

## Release Process

Releases are automated via GitHub Actions (`.github/workflows/release.yml`):

1. Push a version tag: `git tag v0.2.0 && git push --tags`
2. The workflow builds binaries for 4 targets:
   - `x86_64-unknown-linux-gnu` (linux-amd64)
   - `aarch64-unknown-linux-gnu` (linux-arm64)
   - `x86_64-apple-darwin` (darwin-amd64)
   - `aarch64-apple-darwin` (darwin-arm64)
3. A GitHub Release is created with the binaries and SHA-256 checksums
4. The workflow dispatches a repository event to `simonspoon/homebrew-tap` to update the Homebrew formula
