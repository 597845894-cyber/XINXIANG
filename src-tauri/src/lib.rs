mod capture;
mod contracts;
mod model_resources;
pub mod observability;
pub mod security;
pub mod storage;

use contracts::{
    AppBootstrapV1, ImagePreviewV1, NoticeDetailV1, NoticeStateV1, NoticeSummaryV1,
    SecurityStatusV1,
};
use model_resources::{
    inspect_resources, install_resources, install_root, selected_manifest, ModelResourceStatusV1,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WindowEvent,
};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
#[cfg(test)]
const PLACEHOLDER_MODEL_MANIFEST: &str =
    include_str!("../resources/model-placeholder/manifest.json");

#[tauri::command(rename_all = "camelCase")]
fn get_app_bootstrap() -> AppBootstrapV1 {
    AppBootstrapV1::current()
}

#[tauri::command(rename_all = "camelCase")]
fn get_security_status(app: AppHandle) -> Result<SecurityStatusV1, String> {
    let app_data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "LOCAL_SECURITY_CHECK_FAILED".to_owned())?;
    security::status::verify_local_security(&app_data_dir)
        .map_err(|_| "LOCAL_SECURITY_CHECK_FAILED".to_owned())
}

#[tauri::command]
fn open_quick_import(app: AppHandle) {
    show_main_window(&app);
    let _ = app.emit("quickImportRequested", ());
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command(rename_all = "camelCase")]
fn get_model_resource_status(app: AppHandle) -> Result<ModelResourceStatusV1, String> {
    let manifest = selected_manifest()?;
    let app_data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "MODEL_RESOURCE_DIRECTORY_UNAVAILABLE".to_owned())?;
    Ok(inspect_resources(
        &install_root(&app_data_dir, &manifest.selection_id),
        &manifest,
    ))
}

#[tauri::command(rename_all = "camelCase")]
fn install_model_resources(
    app: AppHandle,
    source_directory: String,
) -> Result<ModelResourceStatusV1, String> {
    let manifest = selected_manifest()?;
    let app_data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "MODEL_RESOURCE_DIRECTORY_UNAVAILABLE".to_owned())?;
    install_resources(
        std::path::Path::new(&source_directory),
        &install_root(&app_data_dir, &manifest.selection_id),
        &manifest,
    )
}

fn capture_data_directory(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|_| "NOTICE_STORAGE_FAILED".to_owned())
}

#[tauri::command(rename_all = "camelCase")]
fn import_text_notice(
    app: AppHandle,
    original_text: String,
    published_at: String,
) -> Result<NoticeSummaryV1, String> {
    capture::import_text(&capture_data_directory(&app)?, original_text, published_at)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn import_image_notice(
    app: AppHandle,
    bytes: Vec<u8>,
    declared_media_type: Option<String>,
    published_at: String,
) -> Result<NoticeSummaryV1, String> {
    capture::import_image(
        &capture_data_directory(&app)?,
        bytes,
        declared_media_type,
        published_at,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn list_notices(
    app: AppHandle,
    state: Option<NoticeStateV1>,
) -> Result<Vec<NoticeSummaryV1>, String> {
    capture::list_notices(&capture_data_directory(&app)?, state).map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn get_notice_detail(app: AppHandle, notice_id: String) -> Result<NoticeDetailV1, String> {
    capture::notice_detail(&capture_data_directory(&app)?, &notice_id)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn get_notice_image_preview(app: AppHandle, notice_id: String) -> Result<ImagePreviewV1, String> {
    capture::image_preview(&capture_data_directory(&app)?, &notice_id)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn update_notice_published_time(
    app: AppHandle,
    notice_id: String,
    published_at: String,
) -> Result<(), String> {
    capture::update_published_time(&capture_data_directory(&app)?, &notice_id, published_at)
        .map_err(|error| error.to_string())?;
    let _ = app.emit("relativeDateRecalculationRequested", &notice_id);
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
fn set_notice_state(app: AppHandle, notice_id: String, state: NoticeStateV1) -> Result<(), String> {
    capture::set_notice_state(&capture_data_directory(&app)?, &notice_id, state)
        .map_err(|error| error.to_string())
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn should_hide_to_tray(window_label: &str) -> bool {
    window_label == "main"
}

fn configure_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "打开校园信箱", true, None::<&str>)?;
    let quick_import = MenuItem::with_id(app, "quick-import", "快速导入", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "彻底退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quick_import, &quit])?;

    TrayIconBuilder::new()
        .icon(
            app.default_window_icon()
                .expect("application icon missing")
                .clone(),
        )
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("校园信箱 - 通知仅在本机处理")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quick-import" => {
                show_main_window(app);
                let _ = app.emit("quickImportRequested", ());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            configure_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if should_hide_to_tray(window.label()) {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                    let _ = window.emit("windowHiddenToTray", ());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_app_bootstrap,
            get_security_status,
            get_model_resource_status,
            install_model_resources,
            import_text_notice,
            import_image_notice,
            list_notices,
            get_notice_detail,
            get_notice_image_preview,
            update_notice_published_time,
            set_notice_state,
            open_quick_import,
            quit_app
        ])
        .run(tauri::generate_context!())
        .expect("failed to run campus notice inbox");
}

#[cfg(test)]
mod tests {
    use super::{should_hide_to_tray, APP_VERSION, PLACEHOLDER_MODEL_MANIFEST};

    #[test]
    fn app_version_is_semver_compatible() {
        let parts: Vec<_> = APP_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|part| part.parse::<u64>().is_ok()));
    }

    #[test]
    fn only_the_main_window_closes_to_tray() {
        assert!(should_hide_to_tray("main"));
        assert!(!should_hide_to_tray("quick-import"));
    }

    #[test]
    fn placeholder_model_manifest_is_versioned_and_explicit() {
        let manifest: serde_json::Value =
            serde_json::from_str(PLACEHOLDER_MODEL_MANIFEST).expect("placeholder manifest is JSON");

        assert_eq!(manifest["schemaVersion"], 1);
        assert_eq!(manifest["placeholder"], true);
        assert_eq!(manifest["file"], "placeholder-model.txt");
    }
}
