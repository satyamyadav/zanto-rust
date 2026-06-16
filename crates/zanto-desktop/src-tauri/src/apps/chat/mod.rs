//! Chat — the general-purpose assistant, modeled as an app. Unlike a vertical, it
//! exposes the flexible built-in capabilities (filesystem + shell) rather than one
//! domain. This is what powers "normal" chats like "update my chrome".

use std::sync::Arc;
use serde_json::Value;
use zanto_core::chat::{AppResult, GenaiTool};
use zanto_core::data::DataStore;
use crate::app::{App, AppManifest};

pub struct ChatApp {
    manifest: AppManifest,
}

impl ChatApp {
    pub fn new() -> Arc<dyn App> {
        Arc::new(ChatApp {
            manifest: AppManifest {
                id: "chat".to_string(),
                name: "Chat".to_string(),
                description: "General assistant with filesystem and shell access.".to_string(),
                stores: Vec::new(),
                components: Vec::new(),
            },
        })
    }
}

impl App for ChatApp {
    fn manifest(&self) -> &AppManifest {
        &self.manifest
    }

    fn skill(&self) -> String {
        "You are a capable general-purpose assistant with access to the user's \
         filesystem and shell. Help with whatever the user asks — inspecting files, \
         running commands, and answering questions."
            .to_string()
    }

    fn agent_tools(&self) -> Vec<GenaiTool> {
        // None of its own — the built-in fs/shell tools are provided instead.
        Vec::new()
    }

    fn dispatch_tool(&self, _data: &DataStore, _name: &str, _args: Value) -> Option<Result<AppResult, String>> {
        None
    }

    fn query(&self, _data: &DataStore, name: &str, _args: Value) -> Result<Value, String> {
        Err(format!("unknown query: {name}"))
    }

    fn action(&self, _data: &DataStore, name: &str, _args: Value) -> Result<Value, String> {
        Err(format!("unknown action: {name}"))
    }
}
