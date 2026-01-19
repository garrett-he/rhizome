mod db;
mod documents;
mod profiles;
mod settings;

use std::path::{Path, PathBuf};

use dirs::home_dir;

fn resolve_home() -> Result<PathBuf, String> {
    home_dir().ok_or_else(|| "无法获取用户主目录".to_string())
}

fn current_vault() -> Result<PathBuf, String> {
    let profiles = profiles::load(&resolve_home()?);
    profiles
        .current
        .map(PathBuf::from)
        .ok_or_else(|| "未选择知识库".to_string())
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

#[tauri::command]
fn get_overview() -> Result<db::Overview, String> {
    db::get_overview(Path::new(&current_vault()?))
}

#[tauri::command]
fn add_documents(paths: Vec<String>) -> Result<documents::AddSummary, String> {
    Ok(documents::add_batch(Path::new(&current_vault()?), &paths))
}

#[tauri::command]
fn list_documents() -> Result<Vec<documents::DocumentSummary>, String> {
    documents::list(Path::new(&current_vault()?))
}

#[tauri::command]
fn get_settings() -> Result<Vec<settings::SettingEntry>, String> {
    settings::get_all(Path::new(&current_vault()?))
}

#[tauri::command]
fn set_setting(key: String, value: Option<String>) -> Result<(), String> {
    settings::set(Path::new(&current_vault()?), &key, value)
}

#[tauri::command]
fn get_default_prompt(name: String) -> Result<String, String> {
    settings::default_prompt(&name)
        .map(str::to_string)
        .ok_or_else(|| format!("未知提示词: {name}"))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_profiles,
            create_vault,
            open_vault,
            get_overview,
            add_documents,
            list_documents,
            get_settings,
            set_setting,
            get_default_prompt
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
