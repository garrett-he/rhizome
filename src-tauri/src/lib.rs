mod compile;
mod db;
mod documents;
mod embedding;
mod index_db;
mod indexer;
mod indexing;
mod llm;
mod logging;
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

/// process_documents 的汇总结果。
#[derive(serde::Serialize, Debug, PartialEq)]
pub struct ProcessSummary {
    pub compiled: u64,
    pub compile_failures: u64,
    pub indexed: u64,
    pub index_failures: u64,
}

/// 编译指定文档（空列表 = 全部 raw）。LLM 调用与写库在阻塞线程执行。
#[tauri::command]
async fn compile_documents(document_ids: Vec<String>) -> Result<Vec<llm::CompileOutcome>, String> {
    let vault = current_vault()?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let client = compile::client_from_settings(&vault)?;
        compile::compile_documents(&vault, &document_ids, &client)
    })
    .await
    .map_err(|e| format!("任务执行失败: {e}"))?;
    log_command_result("compile_documents", &result);
    result
}

/// 索引指定文档（空列表 = 全部 compiled）。模型加载与推理在阻塞线程执行。
#[tauri::command]
async fn index_documents(document_ids: Vec<String>) -> Result<Vec<indexer::IndexOutcome>, String> {
    let vault = current_vault()?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let model_name = settings::get(&vault, "embedding_model")?
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| embedding::SUPPORTED_MODEL.to_string());
        let model = embedding::default_embedding_model(&model_name)?;
        indexer::index_documents(&vault, &document_ids, model.as_ref(), &model_name)
    })
    .await
    .map_err(|e| format!("任务执行失败: {e}"))?;
    log_command_result("index_documents", &result);
    result
}

/// 导入后自动处理：编译全部 raw → 索引全部 compiled。
/// LLM 未配置时返回 Err（前端静默保持 raw，等待手动重试）。
#[tauri::command]
async fn process_documents() -> Result<ProcessSummary, String> {
    let vault = current_vault()?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let client = compile::client_from_settings(&vault)?;
        let compile_outcomes = compile::compile_documents(&vault, &[], &client)?;
        let compiled = compile_outcomes.iter().filter(|o| o.ok).count() as u64;
        let compile_failures = compile_outcomes.iter().filter(|o| !o.ok).count() as u64;

        let model_name = settings::get(&vault, "embedding_model")?
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| embedding::SUPPORTED_MODEL.to_string());
        let model = embedding::default_embedding_model(&model_name)?;
        let index_outcomes = indexer::index_documents(&vault, &[], model.as_ref(), &model_name)?;
        let indexed = index_outcomes.iter().filter(|o| o.ok).count() as u64;
        let index_failures = index_outcomes.iter().filter(|o| !o.ok).count() as u64;

        Ok(ProcessSummary {
            compiled,
            compile_failures,
            indexed,
            index_failures,
        })
    })
    .await
    .map_err(|e| format!("任务执行失败: {e}"))?;
    log_command_result("process_documents", &result);
    result
}

/// 命令出错时写 app.log（成功摘要也记录，便于定位时序）。
fn log_command_result<T>(command: &str, result: &Result<T, String>) {
    match result {
        Ok(_) => logging::log("INFO", &format!("{command} 完成")),
        Err(error) => logging::log("ERROR", &format!("{command} 失败: {error}")),
    }
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
            get_default_prompt,
            compile_documents,
            index_documents,
            process_documents
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
