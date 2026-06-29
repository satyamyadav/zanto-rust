//! Configuration IPC commands.

use super::{ConfigDto, ConfigPatch, DesktopState, ProviderDto};
use tauri::State;
use zanto_core::config::{self, ContextSource, Provider, ProviderConfig, Settings};

/// Default provider list when none are configured: registry-derived.
fn default_providers() -> Vec<ProviderConfig> {
    config::SUPPORTED
        .iter()
        .map(|k| {
            let p = Provider(*k);
            ProviderConfig {
                provider: p,
                model: p.default_model().to_string(),
                endpoint: p.default_endpoint().map(str::to_string),
                generation: config::GenerationParams::default(),
            }
        })
        .collect()
}

/// Map a provider id string to a `Provider`; returns `Err` for unknown ids.
fn parse_provider(s: &str) -> Result<Provider, String> {
    config::provider_from_id(s).ok_or_else(|| format!("unknown provider: {s}"))
}

#[tauri::command]
pub fn get_config(state: State<'_, DesktopState>) -> ConfigDto {
    let settings = Settings::load();

    let providers_cfg = if settings.providers.is_empty() {
        default_providers()
    } else {
        settings.providers.clone()
    };

    let providers: Vec<ProviderDto> = providers_cfg
        .into_iter()
        .map(|pc| {
            let has_key = config::has_api_key(pc.provider);
            ProviderDto {
                provider: pc.provider.as_str().to_string(),
                label: pc.provider.label().to_string(),
                needs_key: pc.provider.needs_key(),
                default_endpoint: pc.provider.default_endpoint().map(str::to_string),
                model: pc.model,
                endpoint: pc.endpoint,
                has_key,
                generation: pc.generation,
            }
        })
        .collect();

    let active_provider = settings.active_provider.map(|p| p.as_str().to_string());

    let model = state.model.lock().unwrap().clone();
    // The same window the per-turn Auto context policy uses (ipc/chat.rs): an
    // explicit override, else the active model's known window. Surfaced so the
    // gauge has a denominator on session load, before any chat_done arrives.
    let context_window_tokens = settings
        .context_window_tokens
        .unwrap_or_else(|| config::model_context_window(&model));

    ConfigDto {
        model,
        endpoint: state.endpoint.lock().unwrap().clone(),
        allowed_paths: settings.allowed_paths,
        project_dir: settings.project_dir,
        // Report project-layer context sources only: that's the set add/remove/
        // toggle mutate, so the UI list stays in sync with what they can affect.
        context_sources: load_project_settings().context_sources,
        selected_skill: state.selected_skill.lock().unwrap().clone(),
        max_context_turns: settings.max_context_turns,
        context_window_tokens,
        providers,
        active_provider,
        provider_registry: config::provider_registry(),
        generation: settings.generation.clone(),
    }
}

#[tauri::command]
pub fn set_config(state: State<'_, DesktopState>, patch: ConfigPatch) -> Result<(), String> {
    let mut settings = Settings::load();

    // Apply provider list patch.
    if let Some(provider_patches) = patch.providers {
        let mut new_providers: Vec<ProviderConfig> = Vec::new();
        for pp in provider_patches {
            let p = parse_provider(&pp.provider)?;
            new_providers.push(ProviderConfig {
                provider: p,
                model: pp.model,
                // Canonicalize the stored base URL (trim + trailing slash) so the
                // persisted/displayed value matches what genai needs.
                endpoint: pp
                    .endpoint
                    .map(|e| zanto_core::config::normalize_endpoint(&e))
                    .filter(|e| !e.is_empty()),
                generation: pp.generation,
            });
        }
        settings.providers = new_providers;
    }

    // Apply active provider patch.
    if let Some(ap_str) = &patch.active_provider {
        let p = parse_provider(ap_str)?;
        settings.active_provider = Some(p);
    }

    // Sync legacy model/endpoint from the active() resolution so the running
    // state reflects the provider choice.
    let (_, active_model, active_endpoint) = settings.active();

    // Explicit patch.model/endpoint always win; otherwise use the resolved
    // active-provider values (which may be None for endpoint — leave None as
    // None rather than collapsing to "" to avoid persisting an empty base URL).
    let effective_model = patch.model.clone().unwrap_or(active_model);
    let effective_endpoint = patch
        .endpoint
        .clone()
        .or(active_endpoint)
        .map(|e| zanto_core::config::normalize_endpoint(&e));

    *state.model.lock().unwrap() = effective_model.clone();
    *state.endpoint.lock().unwrap() = effective_endpoint.clone().unwrap_or_default();

    // Keep legacy fields in sync only when there is a real value to write.
    // Never write settings.model = Some("") or settings.endpoint = Some("").
    if !effective_model.is_empty() {
        settings.model = Some(effective_model);
    }
    if let Some(ep) = effective_endpoint {
        if !ep.is_empty() {
            settings.endpoint = Some(ep);
        }
    }

    if let Some(t) = patch.max_context_turns {
        // 0 means "off" — clear it so the default (no summarization) applies.
        settings.max_context_turns = if t == 0 { None } else { Some(t) };
    }

    if let Some(gen) = patch.generation {
        settings.generation = gen;
    }

    settings.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog()
        .file()
        .blocking_pick_folder()
        .map(|p| p.to_string())
}

/// Grant a folder (and children) for this session and persist it to project config.
#[tauri::command]
pub fn add_allowed_path(state: State<'_, DesktopState>, path: String) -> Result<(), String> {
    state.permissions.add_allowed(&path);
    Settings::persist_allowed_path(&path);
    Ok(())
}

/// Load only the project-layer settings (`.zanto/settings.json`), defaulting to
/// empty when absent. Mirrors `Settings::persist_allowed_path`, which edits the
/// project file directly so it never folds user-level settings into the project
/// layer (a full `Settings::load().save()` of the merged settings would).
fn load_project_settings() -> Settings {
    std::fs::read_to_string(config::PROJECT_CONFIG)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Add a context source (file or folder), enabled by default, and persist it to
/// project config. Also grants read access so the agent may actually read it.
#[tauri::command]
pub fn add_context_source(state: State<'_, DesktopState>, path: String) -> Result<(), String> {
    let mut settings = load_project_settings();
    if !settings.context_sources.iter().any(|s| s.path == path) {
        settings.context_sources.push(ContextSource {
            path: path.clone(),
            enabled: true,
        });
        settings.save().map_err(|e| e.to_string())?;
        // Inputs auto-grant read: keep the intent + security layers consistent.
        state.permissions.add_allowed(&path);
        Settings::persist_allowed_path(&path);
    }
    Ok(())
}

/// Remove a context source and persist the change to project config.
#[tauri::command]
pub fn remove_context_source(path: String) -> Result<(), String> {
    let mut settings = load_project_settings();
    let before = settings.context_sources.len();
    settings.context_sources.retain(|s| s.path != path);
    if settings.context_sources.len() != before {
        settings.save().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Toggle a context source's `enabled` flag and persist. No-op if not found.
#[tauri::command]
pub fn toggle_context_source(path: String, enabled: bool) -> Result<(), String> {
    let mut settings = load_project_settings();
    let mut changed = false;
    for s in settings.context_sources.iter_mut() {
        if s.path == path && s.enabled != enabled {
            s.enabled = enabled;
            changed = true;
        }
    }
    if changed {
        settings.save().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Set the active project directory: persist it to project config and grant
/// read access (so the agent may read the project tree). Outputs land in
/// `<dir>/.zanto/artifacts`.
#[tauri::command]
pub fn set_project_dir(state: State<'_, DesktopState>, path: String) -> Result<(), String> {
    let mut settings = load_project_settings();
    settings.project_dir = Some(path.clone());
    settings.save().map_err(|e| e.to_string())?;
    state.permissions.add_allowed(&path);
    Settings::persist_allowed_path(&path);
    Ok(())
}

/// Store an API key for a provider in the OS keychain.
#[tauri::command]
pub fn set_api_key(provider: String, key: String) -> Result<(), String> {
    let p = parse_provider(&provider)?;
    config::set_api_key(p, &key)
}

/// Remove an API key for a provider from the OS keychain.
#[tauri::command]
pub fn clear_api_key(provider: String) -> Result<(), String> {
    let p = parse_provider(&provider)?;
    config::clear_api_key(p)
}

/// List the models a provider exposes, using the saved key/endpoint.
/// Errors (missing key, offline, no list endpoint) surface to the UI as `Err`.
#[tauri::command]
pub async fn list_models(provider: String) -> Result<Vec<String>, String> {
    let p = parse_provider(&provider)?;
    config::list_models(p).await
}
