mod profiles;

use std::path::PathBuf;

use dirs::home_dir;

fn resolve_home() -> Result<PathBuf, String> {
    home_dir().ok_or_else(|| "无法获取用户主目录".to_string())
}

#[tauri::command]
fn get_profiles() -> Result<profiles::ProfilesResponse, String> {
    profiles::get_profiles(&resolve_home()?)
}

#[tauri::command]
fn create_vault(path: String) -> Result<profiles::Profiles, String> {
    profiles::create_vault(&resolve_home()?, &path)
}

#[tauri::command]
fn open_vault(path: String) -> Result<profiles::Profiles, String> {
    profiles::open_vault(&resolve_home()?, &path)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_profiles, create_vault, open_vault])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
