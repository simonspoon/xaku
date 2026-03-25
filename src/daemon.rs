use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;

use crate::protocol::{Request, Response};
use crate::session::Session;

struct Workspace {
    id: u32,
    name: String,
    surfaces: Vec<u32>,
    active_surface: u32,
}

struct State {
    workspaces: HashMap<u32, Workspace>,
    surfaces: HashMap<u32, Session>,
    next_workspace_id: u32,
    next_surface_id: u32,
    active_workspace: Option<u32>,
}

impl State {
    fn new() -> Self {
        Self {
            workspaces: HashMap::new(),
            surfaces: HashMap::new(),
            next_workspace_id: 1,
            next_surface_id: 1,
            active_workspace: None,
        }
    }

    fn resolve_surface(&self, workspace: Option<u32>, surface: Option<u32>) -> Option<u32> {
        if let Some(sid) = surface {
            return Some(sid);
        }
        let wid = workspace.or(self.active_workspace)?;
        let ws = self.workspaces.get(&wid)?;
        Some(ws.active_surface)
    }

    fn handle(&mut self, req: Request) -> Response {
        match req {
            Request::Ping => Response::ok(json!("pong")),

            Request::Shutdown => {
                // Clean up all sessions
                self.surfaces.clear();
                self.workspaces.clear();
                // Signal shutdown via exit (daemon will clean up socket)
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    std::process::exit(0);
                });
                Response::ok_empty()
            }

            Request::NewWorkspace { cwd, command, name } => {
                let wid = self.next_workspace_id;
                let sid = self.next_surface_id;
                self.next_workspace_id += 1;
                self.next_surface_id += 1;

                let cwd = cwd.unwrap_or_else(|| {
                    std::env::current_dir()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| "/tmp".to_string())
                });

                let ws_name = name.unwrap_or_else(|| format!("workspace-{}", wid));

                match Session::spawn(sid, &cwd, command.as_deref()) {
                    Ok(session) => {
                        self.surfaces.insert(sid, session);
                        self.workspaces.insert(
                            wid,
                            Workspace {
                                id: wid,
                                name: ws_name,
                                surfaces: vec![sid],
                                active_surface: sid,
                            },
                        );
                        self.active_workspace = Some(wid);
                        Response::ok(json!(format!("workspace:{}", wid)))
                    }
                    Err(e) => Response::err(format!("Failed to spawn session: {}", e)),
                }
            }

            Request::ListWorkspaces => {
                let list: Vec<_> = self
                    .workspaces
                    .values()
                    .map(|ws| {
                        json!({
                            "ref": format!("workspace:{}", ws.id),
                            "name": ws.name,
                            "surfaces": ws.surfaces.len(),
                            "active": self.active_workspace == Some(ws.id),
                        })
                    })
                    .collect();
                Response::ok(json!(list))
            }

            Request::Tree { workspace } => {
                let wids: Vec<u32> = if let Some(wid) = workspace {
                    vec![wid]
                } else {
                    self.workspaces.keys().copied().collect()
                };

                let mut lines = Vec::new();
                for wid in &wids {
                    if let Some(ws) = self.workspaces.get(wid) {
                        let active = if self.active_workspace == Some(*wid) {
                            " *"
                        } else {
                            ""
                        };
                        lines.push(format!("workspace:{} \"{}\"{}", ws.id, ws.name, active));
                        for (i, sid) in ws.surfaces.iter().enumerate() {
                            let prefix = if i == ws.surfaces.len() - 1 {
                                "  └─"
                            } else {
                                "  ├─"
                            };
                            let alive = self
                                .surfaces
                                .get(sid)
                                .map(|s| if s.is_alive() { "alive" } else { "exited" })
                                .unwrap_or("unknown");
                            let active_s = if ws.active_surface == *sid { " *" } else { "" };
                            lines.push(format!(
                                "{} surface:{} (terminal, {}){}",
                                prefix, sid, alive, active_s
                            ));
                        }
                    }
                }
                Response::ok(json!(lines.join("\n")))
            }

            Request::Send {
                workspace,
                surface,
                text,
            } => {
                let sid = match self.resolve_surface(workspace, surface) {
                    Some(s) => s,
                    None => return Response::err("No workspace or surface specified"),
                };
                match self.surfaces.get_mut(&sid) {
                    Some(session) => match session.send_text(&text) {
                        Ok(()) => Response::ok_empty(),
                        Err(e) => Response::err(e.to_string()),
                    },
                    None => Response::err(format!("Surface {} not found", sid)),
                }
            }

            Request::SendKey {
                workspace,
                surface,
                key,
            } => {
                let sid = match self.resolve_surface(workspace, surface) {
                    Some(s) => s,
                    None => return Response::err("No workspace or surface specified"),
                };
                match self.surfaces.get_mut(&sid) {
                    Some(session) => match session.send_key(&key) {
                        Ok(()) => Response::ok_empty(),
                        Err(e) => Response::err(e.to_string()),
                    },
                    None => Response::err(format!("Surface {} not found", sid)),
                }
            }

            Request::ReadScreen {
                workspace,
                surface,
                lines,
                scrollback,
            } => {
                let sid = match self.resolve_surface(workspace, surface) {
                    Some(s) => s,
                    None => return Response::err("No workspace or surface specified"),
                };
                match self.surfaces.get(&sid) {
                    Some(session) => {
                        let content = session.read_screen(lines, scrollback);
                        Response::ok(json!(content))
                    }
                    None => Response::err(format!("Surface {} not found", sid)),
                }
            }

            Request::CloseWorkspace { workspace } => {
                if let Some(ws) = self.workspaces.remove(&workspace) {
                    for sid in &ws.surfaces {
                        self.surfaces.remove(sid);
                    }
                    if self.active_workspace == Some(workspace) {
                        self.active_workspace = self.workspaces.keys().next().copied();
                    }
                    Response::ok_empty()
                } else {
                    Response::err(format!("Workspace {} not found", workspace))
                }
            }

            Request::CloseSurface { surface } => {
                if self.surfaces.remove(&surface).is_some() {
                    // Remove from parent workspace
                    for ws in self.workspaces.values_mut() {
                        ws.surfaces.retain(|s| *s != surface);
                        if ws.active_surface == surface {
                            ws.active_surface = ws.surfaces.first().copied().unwrap_or(0);
                        }
                    }
                    // Remove empty workspaces
                    self.workspaces.retain(|_, ws| !ws.surfaces.is_empty());
                    if let Some(awid) = self.active_workspace
                        && !self.workspaces.contains_key(&awid)
                    {
                        self.active_workspace = self.workspaces.keys().next().copied();
                    }
                    Response::ok_empty()
                } else {
                    Response::err(format!("Surface {} not found", surface))
                }
            }

            Request::NewSurface {
                workspace,
                surface_type,
            } => {
                if surface_type.as_deref() == Some("browser") {
                    return Response::err(
                        "Browser surfaces not supported — use khora for browser automation",
                    );
                }
                let wid = match workspace.or(self.active_workspace) {
                    Some(w) => w,
                    None => return Response::err("No workspace specified or active"),
                };
                let ws = match self.workspaces.get(&wid) {
                    Some(ws) => ws,
                    None => return Response::err(format!("Workspace {} not found", wid)),
                };
                let cwd = self
                    .surfaces
                    .get(&ws.active_surface)
                    .map(|s| s.cwd.clone())
                    .unwrap_or_else(|| "/tmp".to_string());

                let sid = self.next_surface_id;
                self.next_surface_id += 1;

                match Session::spawn(sid, &cwd, None) {
                    Ok(session) => {
                        self.surfaces.insert(sid, session);
                        let ws = self.workspaces.get_mut(&wid).unwrap();
                        ws.surfaces.push(sid);
                        ws.active_surface = sid;
                        Response::ok(json!(format!("surface:{}", sid)))
                    }
                    Err(e) => Response::err(e.to_string()),
                }
            }

            Request::Identify => {
                let info = if let Some(wid) = self.active_workspace {
                    let ws = self.workspaces.get(&wid);
                    json!({
                        "workspace": format!("workspace:{}", wid),
                        "workspace_name": ws.map(|w| w.name.as_str()).unwrap_or(""),
                        "surface": ws.map(|w| format!("surface:{}", w.active_surface)).unwrap_or_default(),
                    })
                } else {
                    json!({ "workspace": null, "surface": null })
                };
                Response::ok(info)
            }

            Request::RenameWorkspace { workspace, name } => {
                match self.workspaces.get_mut(&workspace) {
                    Some(ws) => {
                        ws.name = name;
                        Response::ok_empty()
                    }
                    None => Response::err(format!("Workspace {} not found", workspace)),
                }
            }

            Request::SelectWorkspace { workspace } => {
                if self.workspaces.contains_key(&workspace) {
                    self.active_workspace = Some(workspace);
                    Response::ok_empty()
                } else {
                    Response::err(format!("Workspace {} not found", workspace))
                }
            }

            Request::CurrentWorkspace => match self.active_workspace {
                Some(wid) => {
                    let name = self
                        .workspaces
                        .get(&wid)
                        .map(|w| w.name.as_str())
                        .unwrap_or("");
                    Response::ok(json!({
                        "ref": format!("workspace:{}", wid),
                        "name": name,
                    }))
                }
                None => Response::ok(json!(null)),
            },
        }
    }
}

pub fn run(socket_path: PathBuf) -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        // Clean up stale socket
        if socket_path.exists() {
            if std::os::unix::net::UnixStream::connect(&socket_path).is_ok() {
                anyhow::bail!("Daemon already running");
            }
            std::fs::remove_file(&socket_path)?;
        }

        // Ensure parent directory exists
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let listener = UnixListener::bind(&socket_path)?;
        let state = Arc::new(Mutex::new(State::new()));

        // Clean up socket on shutdown signals
        let sp = socket_path.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            let _ = std::fs::remove_file(&sp);
            std::process::exit(0);
        });

        loop {
            let (stream, _) = listener.accept().await?;
            let state = state.clone();
            tokio::spawn(async move {
                let (reader, mut writer) = stream.into_split();
                let mut reader = BufReader::new(reader);
                let mut line = String::new();
                if reader.read_line(&mut line).await.is_ok()
                    && !line.is_empty()
                    && let Ok(request) = serde_json::from_str::<Request>(&line)
                {
                    let response = {
                        let state_clone = state.clone();
                        tokio::task::spawn_blocking(move || {
                            let mut s = state_clone.lock().unwrap();
                            s.handle(request)
                        })
                        .await
                        .unwrap_or_else(|e| Response::err(e.to_string()))
                    };
                    if let Ok(json) = serde_json::to_string(&response) {
                        let _ = writer.write_all(json.as_bytes()).await;
                        let _ = writer.write_all(b"\n").await;
                        let _ = writer.flush().await;
                    }
                }
            });
        }
    })
}
