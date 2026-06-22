//! Contract test: every fixture's `response` must deserialize into the real DTO.
//!
//! Guards against drift between the golden fixtures (UI mocks) and the Rust
//! backend types. Add a test here whenever a new fixture is added.

use std::fs;
use std::path::Path;

use zanto_desktop_lib::app::AppManifest;
use zanto_desktop_lib::catalogue::ArtifactDef;
use zanto_desktop_lib::ipc::ConfigDto;
use zanto_core::chat::ChatTurn;
use zanto_core::session::SessionMeta;

fn fixture(name: &str) -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../contract/fixtures")
        .join(format!("{name}.json"));
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw).expect("fixture is valid JSON")
}

#[test]
fn get_config_response_matches_dto() {
    let fx = fixture("get_config");
    let _dto: ConfigDto = serde_json::from_value(fx["response"].clone())
        .expect("get_config response deserializes into ConfigDto");
}

#[test]
fn list_apps_response_matches_dto() {
    let fx = fixture("list_apps");
    let _dto: Vec<AppManifest> = serde_json::from_value(fx["response"].clone())
        .expect("list_apps response deserializes into Vec<AppManifest>");
}

#[test]
fn get_catalogue_response_matches_dto() {
    let fx = fixture("get_catalogue");
    let _dto: Vec<ArtifactDef> = serde_json::from_value(fx["response"].clone())
        .expect("get_catalogue response deserializes into Vec<ArtifactDef>");
}

#[test]
fn list_sessions_response_matches_dto() {
    let fx = fixture("list_sessions");
    let _dto: Vec<SessionMeta> = serde_json::from_value(fx["response"].clone())
        .expect("list_sessions response deserializes into Vec<SessionMeta>");
}

#[test]
fn new_session_response_matches_dto() {
    let fx = fixture("new_session");
    let _id: String = serde_json::from_value(fx["response"].clone())
        .expect("new_session response deserializes into String");
}

#[test]
fn send_message_response_matches_dto() {
    let fx = fixture("send_message");
    let _dto: ChatTurn = serde_json::from_value(fx["response"].clone())
        .expect("send_message response deserializes into ChatTurn");
}
