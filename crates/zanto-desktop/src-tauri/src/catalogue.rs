//! The shared artifact catalogue: the single source of truth (catalogue.json) for
//! the LLM wiring (list/get/render tools) and shell mounting. Each artifact has a
//! `dataSchema`; `render_artifact` validates data against it in Rust so the model
//! gets errors to retry (the shell re-checks with AJV before mounting).

use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use zanto_core::chat::{AppDispatcher, AppResult, GenaiTool, Target};
use zanto_core::data::DataStore;
use crate::app::App;
use crate::interaction::TauriInteractor;

const CATALOGUE_JSON: &str = include_str!("../catalogue.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactDef {
    pub id: String,
    pub description: String,
    #[serde(default)]
    pub when_to_use: String,
    /// Storage class: "view" (ephemeral, render-only) or "file" (durable file
    /// document). Defaults to "view" so any artifact lacking the field renders.
    #[serde(default = "default_storage")]
    pub storage: String,
    #[serde(rename = "data_schema", alias = "dataSchema")]
    pub data_schema: Value,
}

fn default_storage() -> String {
    "view".to_string()
}

pub struct Catalogue {
    defs: Vec<ArtifactDef>,
}

impl Catalogue {
    pub fn load() -> Self {
        let defs: Vec<ArtifactDef> =
            serde_json::from_str(CATALOGUE_JSON).expect("parse catalogue.json");
        Catalogue { defs }
    }

    pub fn all(&self) -> Vec<ArtifactDef> {
        self.defs.clone()
    }

    pub fn get(&self, id: &str) -> Option<&ArtifactDef> {
        self.defs.iter().find(|d| d.id == id)
    }

    /// Summaries for `list_artifacts` (no schemas).
    fn list(&self) -> Value {
        Value::Array(
            self.defs
                .iter()
                .map(|d| json!({ "id": d.id, "description": d.description, "when_to_use": d.when_to_use, "storage": d.storage }))
                .collect(),
        )
    }

    /// Validate `data` against the artifact's dataSchema. `Err` carries readable messages.
    pub fn validate(&self, id: &str, data: &Value) -> Result<(), Vec<String>> {
        let def = self.get(id).ok_or_else(|| vec![format!("unknown artifact: {id}")])?;
        let compiled = jsonschema::JSONSchema::compile(&def.data_schema)
            .map_err(|e| vec![format!("bad schema for {id}: {e}")])?;
        if let Err(errors) = compiled.validate(data) {
            return Err(errors.map(|e| e.to_string()).collect());
        }
        Ok(())
    }
}

/// Tool schemas shared by every app: the artifact tools + the `ask` HITL form tool.
pub fn shared_tools() -> Vec<GenaiTool> {
    vec![
        GenaiTool::new("ask")
            .with_description("Ask the user one or more questions (a small form shown above the composer) and get their answers back. Use for intent clarification or follow-ups before acting.")
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "fields": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "label": { "type": "string" },
                                "type": { "type": "string", "enum": ["text", "select", "confirm"] },
                                "options": { "type": "array", "items": { "type": "string" } }
                            },
                            "required": ["name", "label", "type"]
                        }
                    }
                },
                "required": ["fields"]
            })),
        GenaiTool::new("list_artifacts")
            .with_description("List available UI artifacts (id, description, when to use). Call this to discover what you can render.")
            .with_schema(json!({ "type": "object", "properties": {} })),
        GenaiTool::new("get_artifact")
            .with_description("Get one artifact's full definition, including its dataSchema, before rendering it.")
            .with_schema(json!({
                "type": "object",
                "properties": { "id": { "type": "string" } },
                "required": ["id"]
            })),
        GenaiTool::new("render_artifact")
            .with_description("Display an artifact to the user. This tool call is the ONLY way to show a table, chart, metric, list, or document — describing it in your reply renders nothing. Call get_artifact(id) first to read its dataSchema, then call this with `data` matching that schema. target=inline shows it in the chat, target=canvas in the side panel. Never claim a chart/table will appear without calling this in the same turn.")
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "data": { "type": "object" },
                    "target": { "type": "string", "enum": ["inline", "canvas"] }
                },
                "required": ["id", "data"]
            })),
    ]
}

/// Dispatcher layered over each app: handles the shared artifact tools, then
/// delegates anything else to the app's own domain tools.
pub struct SharedDispatcher {
    catalogue: Arc<Catalogue>,
    app: Arc<dyn App>,
    data: Arc<DataStore>,
    interactor: TauriInteractor,
}

impl SharedDispatcher {
    pub fn new(
        catalogue: Arc<Catalogue>,
        app: Arc<dyn App>,
        data: Arc<DataStore>,
        interactor: TauriInteractor,
    ) -> Self {
        Self { catalogue, app, data, interactor }
    }
}

#[async_trait]
impl AppDispatcher for SharedDispatcher {
    async fn dispatch(&self, name: &str, args: Value) -> Option<Result<AppResult, String>> {
        match name {
            "ask" => {
                let title = args.get("title").cloned().unwrap_or(Value::Null);
                let fields = args.get("fields").cloned().unwrap_or(json!([]));
                let answers = self
                    .interactor
                    .request("form", json!({ "title": title, "steps": [{ "fields": fields }] }))
                    .await;
                Some(Ok(AppResult::Data(answers)))
            }
            "list_artifacts" => Some(Ok(AppResult::Data(self.catalogue.list()))),
            "get_artifact" => {
                let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let result = match self.catalogue.get(id) {
                    Some(def) => serde_json::to_value(def).unwrap_or(Value::Null),
                    None => json!({ "error": format!("unknown artifact: {id}") }),
                };
                Some(Ok(AppResult::Data(result)))
            }
            "render_artifact" => {
                let id = args.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let data = args.get("data").cloned().unwrap_or(Value::Null);
                let target = match args.get("target").and_then(|v| v.as_str()) {
                    Some("canvas") => Target::Canvas,
                    _ => Target::Inline,
                };
                match self.catalogue.get(&id) {
                    None => Some(Ok(AppResult::Data(json!({
                        "error": format!("unknown artifact id '{id}'. render_artifact only displays artifacts from the catalogue — it does not create them, and store_artifact does not display anything. Call list_artifacts for valid ids (e.g. \"chart\", \"table\", \"metric\"), then render with that id."),
                    })))),
                    Some(def) => match self.catalogue.validate(&id, &data) {
                        Ok(()) => Some(Ok(AppResult::Block { component_id: id, data, target })),
                        Err(details) => Some(Ok(AppResult::Data(json!({
                            "error": "data did not match the artifact's dataSchema. Fix `data` to match the `dataSchema` below, then call render_artifact again.",
                            "details": details,
                            "dataSchema": def.data_schema.clone(),
                        })))),
                    },
                }
            }
            _ => self.app.dispatch_tool(&self.data, name, args),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_class_parses_from_catalogue() {
        let cat = Catalogue::load();
        assert_eq!(cat.get("chart").unwrap().storage, "view");
        assert_eq!(cat.get("markdown").unwrap().storage, "file");
    }

    #[test]
    fn missing_storage_defaults_to_view() {
        let def: ArtifactDef =
            serde_json::from_str(r#"{ "id": "x", "description": "d", "data_schema": {} }"#)
                .expect("parse def without storage");
        assert_eq!(def.storage, "view");
    }
}
