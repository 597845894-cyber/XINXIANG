mod backup;
mod capture;
mod contracts;
mod model_resources;
pub mod observability;
pub mod security;
pub mod storage;
mod understanding;

use contracts::{
    AnalysisProgressV1, AnalysisRevisionViewV1, AppBootstrapV1, BackupSummaryV1, CandidateViewV1,
    NoticeDetailV1, NoticeRelationViewV1, NoticeStateV1, NoticeSummaryV1,
    NotificationEventV1, ReminderViewV1, SecurityStatusV1, TaskRevisionViewV1, TaskViewV1,
};
use model_resources::{
    inspect_resources, install_resources, install_root, selected_manifest, ModelResourceStatusV1,
};
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WindowEvent,
};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
static ANALYSIS_CANCELLATIONS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static ANALYSIS_QUEUE: OnceLock<Mutex<()>> = OnceLock::new();

fn analysis_cancellations() -> &'static Mutex<HashSet<String>> {
    ANALYSIS_CANCELLATIONS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn is_analysis_cancelled(notice_id: &str) -> bool {
    analysis_cancellations()
        .lock()
        .map(|pending| pending.contains(notice_id))
        .unwrap_or(false)
}

fn analysis_queue() -> &'static Mutex<()> {
    ANALYSIS_QUEUE.get_or_init(|| Mutex::new(()))
}
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
fn list_reminders(app: AppHandle, task_id: Option<String>) -> Result<Vec<ReminderViewV1>, String> {
    capture::list_reminders(&capture_data_directory(&app)?, task_id.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn upsert_reminder(
    app: AppHandle,
    task_id: String,
    reminder_id: String,
    scheduled_at: String,
    idempotency_key: String,
) -> Result<ReminderViewV1, String> {
    capture::upsert_reminder(
        &capture_data_directory(&app)?,
        &task_id,
        reminder_id,
        scheduled_at,
        idempotency_key,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn delete_reminder(app: AppHandle, reminder_id: String) -> Result<(), String> {
    capture::delete_reminder(&capture_data_directory(&app)?, &reminder_id)
        .map_err(|error| error.to_string())
}

fn scan_and_emit_reminders(app: &AppHandle) -> Result<usize, String> {
    let reminders = capture::claim_due_reminders(&capture_data_directory(app)?)
        .map_err(|error| error.to_string())?;
    let mut grouped: std::collections::BTreeMap<String, (String, String, u32)> =
        std::collections::BTreeMap::new();
    for reminder in &reminders {
        let entry = grouped
            .entry(reminder.task_id.clone())
            .or_insert_with(|| (reminder.id.clone(), reminder.scheduled_at.clone(), 0));
        entry.2 += 1;
    }
    for (task_id, (reminder_id, scheduled_at, missed_count)) in grouped {
        let _ = app.emit(
            "reminderTriggered",
            NotificationEventV1 {
                reminder_id,
                task_id,
                scheduled_at,
                missed_count,
            },
        );
    }
    Ok(reminders.len())
}

#[tauri::command(rename_all = "camelCase")]
fn run_reminder_scan(app: AppHandle) -> Result<usize, String> {
    scan_and_emit_reminders(&app)
}

#[tauri::command(rename_all = "camelCase")]
fn get_model_resource_status(_app: AppHandle) -> Result<ModelResourceStatusV1, String> {
    let manifest = selected_manifest()?;
    Ok(inspect_resources(
        &install_root(&manifest.selection_id),
        &manifest,
    ))
}

#[tauri::command(rename_all = "camelCase")]
fn install_model_resources(
    _app: AppHandle,
    source_directory: String,
) -> Result<ModelResourceStatusV1, String> {
    let manifest = selected_manifest()?;
    install_resources(
        std::path::Path::new(&source_directory),
        &install_root(&manifest.selection_id),
        &manifest,
    )
}

fn capture_data_directory(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|_| "NOTICE_STORAGE_FAILED".to_owned())
}

#[tauri::command(rename_all = "camelCase")]
fn create_backup(
    app: AppHandle,
    target_path: String,
    password: String,
) -> Result<BackupSummaryV1, String> {
    capture::create_backup(
        &capture_data_directory(&app)?,
        std::path::Path::new(&target_path),
        &password,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn inspect_backup(path: String, password: String) -> Result<BackupSummaryV1, String> {
    capture::inspect_backup(std::path::Path::new(&path), &password)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn restore_backup(
    app: AppHandle,
    path: String,
    password: String,
    confirmed: bool,
) -> Result<BackupSummaryV1, String> {
    if !confirmed {
        return Err("BACKUP_RESTORE_CONFIRMATION_REQUIRED".to_owned());
    }
    capture::restore_backup(
        &capture_data_directory(&app)?,
        std::path::Path::new(&path),
        &password,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn delete_notice_cascade(app: AppHandle, notice_id: String, confirmed: bool) -> Result<(), String> {
    if !confirmed {
        return Err("NOTICE_DELETE_CONFIRMATION_REQUIRED".to_owned());
    }
    capture::delete_notice_cascade(&capture_data_directory(&app)?, &notice_id)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn delete_notice_keep_tasks(
    app: AppHandle,
    notice_id: String,
    confirmed: bool,
) -> Result<(), String> {
    if !confirmed {
        return Err("NOTICE_DELETE_CONFIRMATION_REQUIRED".to_owned());
    }
    capture::delete_notice_keep_tasks(&capture_data_directory(&app)?, &notice_id)
        .map_err(|error| error.to_string())
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

#[tauri::command(rename_all = "camelCase")]
fn analyze_notice(
    app: AppHandle,
    notice_id: String,
) -> Result<understanding::AnalysisResultV1, String> {
    let _queue_guard = analysis_queue()
        .lock()
        .map_err(|_| "ANALYSIS_QUEUE_UNAVAILABLE".to_owned())?;
    if let Ok(mut pending) = analysis_cancellations().lock() {
        pending.remove(&notice_id);
    }
    let emit_progress = |stage: &str, progress_percent: u8| {
        let _ = app.emit(
            "analysisProgress",
            AnalysisProgressV1 {
                notice_id: notice_id.clone(),
                stage: stage.to_owned(),
                progress_percent,
            },
        );
    };
    emit_progress("读取原始内容", 10);
    if is_analysis_cancelled(&notice_id) {
        return Err("ANALYSIS_CANCELLED".to_owned());
    }
    let (input, published_at) = capture::analysis_input(&capture_data_directory(&app)?, &notice_id)
        .map_err(|error| error.to_string())?;
    emit_progress("准备本地文字分析", 35);
    if is_analysis_cancelled(&notice_id) {
        return Err("ANALYSIS_CANCELLED".to_owned());
    }
    let revision_id = format!("analysis-{}", uuid::Uuid::new_v4());
    let result = match input {
        capture::AnalysisInput::Text(text) => {
            emit_progress("本地文字分析", 65);
            understanding::analyze_text(&text, &published_at, revision_id.clone())
        }
    };
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            if error != understanding::UnderstandingError::ModelCancelled {
                let _ = capture::set_notice_state(
                    &capture_data_directory(&app)?,
                    &notice_id,
                    NoticeStateV1::Failed,
                );
            }
            return Err(error.to_string());
        }
    };
    if is_analysis_cancelled(&notice_id) {
        return Err("ANALYSIS_CANCELLED".to_owned());
    }
    emit_progress("保存分析版本", 85);

    let database = capture::open_database_for_analysis(&capture_data_directory(&app)?)
        .map_err(|error| error.to_string())?;
    let mut ids = Vec::with_capacity(result.candidates.len());
    let mut payloads = Vec::with_capacity(result.candidates.len());
    for (index, candidate) in result.candidates.iter().enumerate() {
        ids.push(format!("{}-{}", result.revision_id, index));
        payloads.push(
            serde_json::to_vec(candidate).map_err(|_| "ANALYSIS_SERIALIZE_FAILED".to_owned())?,
        );
    }
    let rows = ids
        .iter()
        .zip(payloads.iter())
        .map(|(id, payload)| storage::repository::NewCandidate { id, payload })
        .collect::<Vec<_>>();
    let analysis_text = Some(result.normalized_text.as_str());
    storage::repository::NoticeRepository::new(database.connection())
        .save_analysis_revision_full(
            &notice_id,
            &result.revision_id,
            &result.classifier_version,
            analysis_text,
            &rows,
        )
        .map_err(|error| error.to_string())?;
    emit_progress("分析完成", 100);
    let _ = app.emit("analysisCompleted", &result);
    if let Ok(mut pending) = analysis_cancellations().lock() {
        pending.remove(&notice_id);
    }
    Ok(result)
}

#[tauri::command(rename_all = "camelCase")]
fn list_analysis_revisions(
    app: AppHandle,
    notice_id: String,
) -> Result<Vec<AnalysisRevisionViewV1>, String> {
    capture::list_analysis_revisions(&capture_data_directory(&app)?, &notice_id)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn cancel_analysis(notice_id: String) -> Result<(), String> {
    analysis_cancellations()
        .lock()
        .map_err(|_| "ANALYSIS_CANCEL_STATE_UNAVAILABLE".to_owned())?
        .insert(notice_id);
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
fn list_review_candidates(app: AppHandle) -> Result<Vec<CandidateViewV1>, String> {
    capture::list_review_candidates(&capture_data_directory(&app)?)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn edit_task_candidate(
    app: AppHandle,
    candidate_id: String,
    payload: serde_json::Value,
) -> Result<(), String> {
    capture::edit_task_candidate(&capture_data_directory(&app)?, &candidate_id, payload)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn confirm_task_candidate(
    app: AppHandle,
    candidate_id: String,
    payload: serde_json::Value,
) -> Result<TaskViewV1, String> {
    capture::confirm_task_candidate(&capture_data_directory(&app)?, &candidate_id, payload)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn ignore_task_candidate(app: AppHandle, candidate_id: String) -> Result<(), String> {
    capture::ignore_task_candidate(&capture_data_directory(&app)?, &candidate_id)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn merge_task_candidates(
    app: AppHandle,
    target_id: String,
    source_ids: Vec<String>,
    payload: serde_json::Value,
) -> Result<(), String> {
    capture::merge_task_candidates(
        &capture_data_directory(&app)?,
        &target_id,
        &source_ids,
        payload,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn split_task_candidate(
    app: AppHandle,
    candidate_id: String,
    payloads: Vec<serde_json::Value>,
) -> Result<(), String> {
    capture::split_task_candidate(&capture_data_directory(&app)?, &candidate_id, payloads)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn list_tasks(app: AppHandle) -> Result<Vec<TaskViewV1>, String> {
    capture::list_tasks(&capture_data_directory(&app)?).map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn create_manual_task(app: AppHandle, payload: serde_json::Value) -> Result<TaskViewV1, String> {
    capture::create_manual_task(&capture_data_directory(&app)?, payload)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn update_task(app: AppHandle, task_id: String, payload: serde_json::Value) -> Result<(), String> {
    capture::update_task(&capture_data_directory(&app)?, &task_id, payload)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn set_task_state(
    app: AppHandle,
    task_id: String,
    state: String,
    payload: serde_json::Value,
) -> Result<(), String> {
    capture::set_task_state(&capture_data_directory(&app)?, &task_id, &state, payload)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn get_task_history(app: AppHandle, task_id: String) -> Result<Vec<TaskRevisionViewV1>, String> {
    capture::task_history(&capture_data_directory(&app)?, &task_id)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn suggest_notice_relations(
    app: AppHandle,
    notice_id: String,
) -> Result<Vec<NoticeRelationViewV1>, String> {
    capture::suggest_notice_relations(&capture_data_directory(&app)?, &notice_id)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn list_notice_relations(
    app: AppHandle,
    notice_id: Option<String>,
) -> Result<Vec<NoticeRelationViewV1>, String> {
    capture::list_notice_relations(&capture_data_directory(&app)?, notice_id.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn resolve_notice_relation(
    app: AppHandle,
    relation_id: String,
    accepted: bool,
) -> Result<(), String> {
    capture::resolve_notice_relation(&capture_data_directory(&app)?, &relation_id, accepted)
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
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                let _ = scan_and_emit_reminders(&handle);
                std::thread::sleep(std::time::Duration::from_secs(15));
            });
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
            update_notice_published_time,
            set_notice_state,
            analyze_notice,
            cancel_analysis,
            list_analysis_revisions,
            list_review_candidates,
            edit_task_candidate,
            confirm_task_candidate,
            ignore_task_candidate,
            merge_task_candidates,
            split_task_candidate,
            list_tasks,
            create_manual_task,
            update_task,
            set_task_state,
            get_task_history,
            suggest_notice_relations,
            list_notice_relations,
            resolve_notice_relation,
            open_quick_import,
            quit_app,
            list_reminders,
            upsert_reminder,
            delete_reminder,
            run_reminder_scan,
            create_backup,
            inspect_backup,
            restore_backup,
            delete_notice_cascade,
            delete_notice_keep_tasks
        ])
        .run(tauri::generate_context!())
        .expect("failed to run campus notice inbox");
}

#[cfg(test)]
mod tests {
    use super::{
        analysis_cancellations, cancel_analysis, is_analysis_cancelled, should_hide_to_tray,
        APP_VERSION, PLACEHOLDER_MODEL_MANIFEST,
    };

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

    #[test]
    fn cancellation_requests_are_scoped_to_the_notice() {
        cancel_analysis("notice-cancel".to_owned()).unwrap();
        assert!(is_analysis_cancelled("notice-cancel"));
        assert!(!is_analysis_cancelled("notice-other"));
        analysis_cancellations().lock().unwrap().clear();
    }
}
