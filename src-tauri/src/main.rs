#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{path::PathBuf, sync::Arc};

use context_menu_core::{
    ApplyResult, ContextMenuService, CreateActionRequest, JsonLogStore, MenuEntry, ProposedChange,
    WindowsRegistryProvider,
};
use rfd::FileDialog;
use tauri::{Manager, State};

struct AppState {
    service: Arc<ContextMenuService>,
}

#[tauri::command]
fn scan_entries(state: State<'_, AppState>) -> Result<Vec<MenuEntry>, String> {
    state.service.scan_entries().map_err(|err| err.to_string())
}

#[tauri::command]
fn suggest_actions(state: State<'_, AppState>) -> Result<Vec<ProposedChange>, String> {
    state
        .service
        .suggest_actions()
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn create_action(
    request: CreateActionRequest,
    state: State<'_, AppState>,
) -> Result<Vec<ProposedChange>, String> {
    state
        .service
        .create_action(request)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn create_custom_entry(
    payload: CreateActionRequest,
    state: State<'_, AppState>,
) -> Result<Vec<ProposedChange>, String> {
    state
        .service
        .create_action(payload)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn toggle_entry(
    id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<ApplyResult, String> {
    state
        .service
        .toggle_entry(&id, enabled)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn apply_changes(
    changes: Vec<ProposedChange>,
    state: State<'_, AppState>,
) -> Result<ApplyResult, String> {
    state
        .service
        .apply_changes(changes)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn rollback(change_set_id: String, state: State<'_, AppState>) -> Result<ApplyResult, String> {
    state
        .service
        .rollback(&change_set_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn list_change_sets(
    state: State<'_, AppState>,
) -> Result<Vec<context_menu_core::ChangeSetSummary>, String> {
    state
        .service
        .list_change_sets()
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn pick_path(kind: String) -> Result<Option<String>, String> {
    let mut dialog = FileDialog::new();
    match kind.as_str() {
        "icon" => {
            dialog = dialog.add_filter("Icon files", &["ico", "png", "bmp", "exe"]);
        }
        _ => {
            dialog = dialog.add_filter("Executable files", &["exe", "cmd", "bat", "com", "ps1"]);
        }
    }

    Ok(dialog
        .pick_file()
        .map(|path| path.to_string_lossy().to_string()))
}

fn build_service(app: &tauri::AppHandle) -> anyhow::Result<ContextMenuService> {
    let base: PathBuf = app
        .path()
        .app_local_data_dir()
        .map_err(|err| anyhow::anyhow!(err.to_string()))?;
    std::fs::create_dir_all(&base)?;

    let log_store = Arc::new(JsonLogStore::new(base.join("change_sets")));
    let provider = Arc::new(WindowsRegistryProvider::new());

    Ok(ContextMenuService::new(provider, log_store))
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let service = build_service(app.handle())?;
            app.manage(AppState {
                service: Arc::new(service),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_entries,
            suggest_actions,
            create_action,
            create_custom_entry,
            toggle_entry,
            apply_changes,
            rollback,
            list_change_sets,
            pick_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running context menu helper");
}
