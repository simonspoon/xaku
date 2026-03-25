use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Ping,
    Shutdown,
    NewWorkspace {
        cwd: Option<String>,
        command: Option<String>,
        name: Option<String>,
    },
    ListWorkspaces,
    Tree {
        workspace: Option<u32>,
    },
    Send {
        workspace: Option<u32>,
        surface: Option<u32>,
        text: String,
    },
    SendKey {
        workspace: Option<u32>,
        surface: Option<u32>,
        key: String,
    },
    ReadScreen {
        workspace: Option<u32>,
        surface: Option<u32>,
        lines: Option<usize>,
        scrollback: bool,
    },
    CloseWorkspace {
        workspace: u32,
    },
    CloseSurface {
        surface: u32,
    },
    NewSurface {
        workspace: Option<u32>,
        surface_type: Option<String>,
    },
    Identify,
    RenameWorkspace {
        workspace: u32,
        name: String,
    },
    SelectWorkspace {
        workspace: u32,
    },
    CurrentWorkspace,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    pub fn ok(data: serde_json::Value) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn ok_empty() -> Self {
        Self {
            ok: true,
            data: None,
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}
