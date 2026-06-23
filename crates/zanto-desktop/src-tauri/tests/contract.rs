//! Contract test: every fixture's `response` must deserialize into the real DTO.
//!
//! Guards against drift between the golden fixtures (UI mocks) and the Rust
//! backend types. Add a test here whenever a new fixture is added.

use std::fs;
use std::path::Path;

use zanto_core::chat::ChatTurn;
use zanto_core::session::SessionMeta;
use zanto_desktop_lib::app::AppManifest;
use zanto_desktop_lib::catalogue::ArtifactDef;
use zanto_desktop_lib::ipc::ConfigDto;

fn fixture(name: &str) -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../contract/fixtures")
        .join(format!("{name}.json"));
    let raw = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
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

#[test]
fn list_pinned_artifacts_response_matches_dto() {
    let fx = fixture("list_pinned_artifacts");
    let _v: Vec<zanto_desktop_lib::ipc::artifacts::PinnedArtifact> =
        serde_json::from_value(fx["response"].clone())
            .expect("list_pinned_artifacts → Vec<PinnedArtifact>");
}

#[test]
fn read_pinned_artifact_response_matches_dto() {
    let fx = fixture("read_pinned_artifact");
    let _v: zanto_desktop_lib::ipc::artifacts::PinnedArtifact =
        serde_json::from_value(fx["response"].clone())
            .expect("read_pinned_artifact → PinnedArtifact");
}

#[test]
fn pin_artifact_cmd_response_matches_dto() {
    let fx = fixture("pin_artifact_cmd");
    let _v: i64 = serde_json::from_value(fx["response"].clone()).expect("pin_artifact_cmd → i64");
}

#[test]
fn load_session_response_matches_dto() {
    let fx = fixture("load_session");
    let msgs: Vec<zanto_desktop_lib::ipc::RenderMsg> =
        serde_json::from_value(fx["response"].clone()).expect("load_session → Vec<RenderMsg>");
    // The fixture's first message carries an attachment; assert it is preserved.
    assert_eq!(msgs[0].attachments.len(), 1);
    assert_eq!(msgs[0].attachments[0].path, "/home/user/photo.png");
    assert_eq!(msgs[0].attachments[0].name, "photo.png");
    assert!(msgs[0].attachments[0].is_image);
    // The second message (assistant) has no attachments (default empty vec).
    assert!(msgs[1].attachments.is_empty());
}

#[test]
fn read_image_data_url_response_matches_dto() {
    let fx = fixture("read_image_data_url");
    let data_url: String = serde_json::from_value(fx["response"].clone())
        .expect("read_image_data_url response deserializes into String");
    assert!(data_url.starts_with("data:"), "response must be a data-URL");
}

#[test]
fn open_path_response_matches_dto() {
    let fx = fixture("open_path");
    // open_path returns () — the fixture response is null (JSON null → unit).
    assert!(
        fx["response"].is_null(),
        "open_path response must be null (unit)"
    );
}
