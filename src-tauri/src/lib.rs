mod contracts;

use contracts::AppBootstrapV1;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[tauri::command(rename_all = "camelCase")]
fn get_app_bootstrap() -> AppBootstrapV1 {
    AppBootstrapV1::current()
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_app_bootstrap])
        .run(tauri::generate_context!())
        .expect("failed to run campus notice inbox");
}

#[cfg(test)]
mod tests {
    use super::APP_VERSION;

    #[test]
    fn app_version_is_semver_compatible() {
        let parts: Vec<_> = APP_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|part| part.parse::<u64>().is_ok()));
    }
}
