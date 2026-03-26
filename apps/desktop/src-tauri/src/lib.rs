mod categories;
mod compile;
mod db;
mod documents;
mod extract;
mod embedding;
mod index_db;
mod indexer;
mod indexing;
mod jobs;
mod llm;
mod logging;
mod profiles;
mod qa;
mod search;
mod settings;
mod tasks;

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
async fn add_documents(paths: Vec<String>, category_name: Option<String>) -> Result<documents::AddSummary, String> {
    let vault = current_vault()?;
    let category_id = match category_name.as_deref() {
        // 分类名非空时解析（不存在自动创建）；空串视为未指定
        Some(name) if !name.trim().is_empty() => Some(categories::ensure_category(Path::new(&vault), name)?),
        _ => None,
    };
    // 导入要调 any2txt 子进程（媒体类超时上限 600s），放到阻塞线程池，
    // 避免同步命令跑在主线程时冻结整个窗口
    tauri::async_runtime::spawn_blocking(move || {
        documents::add_batch(
            Path::new(&vault),
            &paths,
            category_id.as_deref(),
            &extract::Any2txtExtractor,
        )
    })
    .await
    .map_err(|e| format!("导入任务异常退出: {e}"))
}

#[tauri::command]
fn create_category(name: String) -> Result<categories::CategorySummary, String> {
    categories::create_category(Path::new(&current_vault()?), &name)
}

#[tauri::command]
fn list_categories() -> Result<Vec<categories::CategorySummary>, String> {
    categories::list_categories(Path::new(&current_vault()?))
}

#[tauri::command]
fn rename_category(id: String, name: String) -> Result<categories::CategorySummary, String> {
    categories::rename_category(Path::new(&current_vault()?), &id, &name)
}

#[tauri::command]
fn delete_category(id: String) -> Result<categories::DeleteOutcome, String> {
    categories::delete_category(Path::new(&current_vault()?), &id)
}

#[tauri::command]
fn set_document_category(document_id: String, category_id: Option<String>) -> Result<(), String> {
    documents::set_category(Path::new(&current_vault()?), &document_id, category_id.as_deref())
}

#[tauri::command]
fn list_documents() -> Result<Vec<documents::DocumentSummary>, String> {
    documents::list(Path::new(&current_vault()?))
}

#[tauri::command]
fn get_document(id: String) -> Result<documents::DocumentDetail, String> {
    documents::get_detail(Path::new(&current_vault()?), &id)
}

#[tauri::command]
fn list_chunks(document_id: String) -> Result<Vec<indexer::ChunkInfo>, String> {
    indexer::list_chunks(Path::new(&current_vault()?), &document_id)
}

#[tauri::command]
fn save_document_file(id: String, target: String) -> Result<(), String> {
    documents::export_original(Path::new(&current_vault()?), &id, &target)
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

/// 用表单当前值测试 LLM 连接（不依赖是否已保存）。
#[tauri::command]
async fn test_llm_connection(base_url: String, api_key: String, model: String) -> Result<(), String> {
    let config = llm::LlmConfig {
        base_url: base_url.trim().to_string(),
        api_key: api_key.trim().to_string(),
        model: model.trim().to_string(),
    };
    if config.base_url.is_empty() || config.api_key.is_empty() || config.model.is_empty() {
        return Err("请先填写 Base URL、API Key 与模型".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || llm::OpenAiClient::new(config).test_connection())
        .await
        .map_err(|e| format!("任务执行失败: {e}"))?
}

/// 问答事件的 Tauri 实现：转发到前端监听（payload serde_json::Value）。
struct TauriQaEvents {
    app: tauri::AppHandle,
}

impl TauriQaEvents {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        use tauri::Emitter;
        if let Err(e) = self.app.emit(event, payload) {
            logging::log("ERROR", &format!("qa 事件发送失败 {event}: {e}"));
        }
    }
}

impl qa::QaEvents for TauriQaEvents {
    fn start(&self, conversation_id: &str) {
        self.emit("qa-start", serde_json::json!({ "conversation_id": conversation_id }));
    }

    fn delta(&self, conversation_id: &str, text: &str) {
        self.emit(
            "qa-delta",
            serde_json::json!({ "conversation_id": conversation_id, "text": text }),
        );
    }

    fn done(&self, conversation_id: &str, sources: &[qa::SourceRef], message_id: i64) {
        self.emit(
            "qa-done",
            serde_json::json!({ "conversation_id": conversation_id, "sources": sources, "message_id": message_id }),
        );
    }

    fn error(&self, conversation_id: &str, message: &str) {
        self.emit(
            "qa-error",
            serde_json::json!({ "conversation_id": conversation_id, "message": message }),
        );
    }
}

/// 提问：检索 top-5 组装上下文 → LLM 流式回答（qa-delta 事件推送）→ 落库。
#[tauri::command]
async fn qa_ask(
    app: tauri::AppHandle,
    question: String,
    conversation_id: Option<String>,
) -> Result<qa::AskOutcome, String> {
    let vault = current_vault()?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let client = compile::client_from_settings(&vault)?;
        let model_name = settings::get(&vault, "embedding_model")?
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| embedding::SUPPORTED_MODEL.to_string());
        // 模型加载失败不报错：检索降级仅 FTS
        let model = embedding::default_embedding_model(&model_name).ok();
        let events = TauriQaEvents { app };
        qa::qa_ask_service(
            &vault,
            &events,
            &question,
            conversation_id.as_deref(),
            &client,
            model.as_deref(),
        )
    })
    .await
    .map_err(|e| format!("任务执行失败: {e}"))?;
    log_command_result("qa_ask", &result);
    result
}

#[tauri::command]
fn list_conversations() -> Result<Vec<qa::ConversationSummary>, String> {
    qa::list_conversations(Path::new(&current_vault()?))
}

#[tauri::command]
fn list_messages(conversation_id: String) -> Result<Vec<qa::Message>, String> {
    qa::list_messages(Path::new(&current_vault()?), &conversation_id)
}

#[tauri::command]
fn rename_conversation(id: String, title: String) -> Result<(), String> {
    qa::rename_conversation(Path::new(&current_vault()?), &id, &title)
}

#[tauri::command]
fn delete_conversation(id: String) -> Result<(), String> {
    qa::delete_conversation(Path::new(&current_vault()?), &id)
}

/// 任务进度事件的 Tauri 实现：转发到前端监听（payload serde_json::Value）。
struct TauriTaskEvents {
    app: tauri::AppHandle,
}

impl TauriTaskEvents {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        use tauri::Emitter;
        if let Err(e) = self.app.emit(event, payload) {
            logging::log("ERROR", &format!("task 事件发送失败 {event}: {e}"));
        }
    }
}

impl tasks::TaskEvents for TauriTaskEvents {
    fn start(&self, job_id: &str, kind: &str, total: u64) {
        self.emit(
            "task-start",
            serde_json::json!({ "job_id": job_id, "kind": kind, "total": total }),
        );
    }

    fn item(&self, job_id: &str, document_id: &str, step: &str, ok: bool, error: Option<&str>) {
        self.emit(
            "task-item",
            serde_json::json!({ "job_id": job_id, "document_id": document_id, "step": step, "ok": ok, "error": error }),
        );
    }

    fn done(&self, job_id: &str, succeeded: u64, failed: u64) {
        self.emit(
            "task-done",
            serde_json::json!({ "job_id": job_id, "succeeded": succeeded, "failed": failed }),
        );
    }
}

/// 批量执行任务（step：compile/index/all）；事件先于命令 resolve 到达。
#[tauri::command]
async fn run_tasks(
    app: tauri::AppHandle,
    document_ids: Vec<String>,
    step: String,
) -> Result<tasks::TaskSummary, String> {
    let vault = current_vault()?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let events = TauriTaskEvents { app };
        tasks::run_batch_from_settings(&vault, &events, &document_ids, &step)
    })
    .await
    .map_err(|e| format!("任务执行失败: {e}"))?;
    log_command_result("run_tasks", &result);
    result
}

#[tauri::command]
fn list_jobs() -> Result<Vec<jobs::JobRecord>, String> {
    jobs::list_jobs(Path::new(&current_vault()?), 10)
}

/// 混合检索（向量 KNN + FTS trigram RRF 融合）；向量腿失败降级仅 FTS。
#[tauri::command]
async fn search_documents(query: String, limit: Option<u32>) -> Result<Vec<search::SearchHit>, String> {
    let limit = limit.unwrap_or(10).clamp(1, 50) as usize;
    let vault = current_vault()?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let model_name = settings::get(&vault, "embedding_model")?
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| embedding::SUPPORTED_MODEL.to_string());
        // 模型加载失败不报错：降级仅 FTS
        let model = embedding::default_embedding_model(&model_name).ok();
        search::search_documents(&vault, &query, limit, model.as_deref())
    })
    .await
    .map_err(|e| format!("任务执行失败: {e}"))?;
    log_command_result("search_documents", &result);
    result
}

/// process_documents 的汇总结果。
#[derive(serde::Serialize, Debug, PartialEq)]
pub struct ProcessSummary {
    pub compiled: u64,
    pub compile_failures: u64,
    pub indexed: u64,
    pub index_failures: u64,
}

/// 编译指定文档（空列表 = 全部 raw）。force 时重置已编译文档并清空旧切片再重编译。
#[tauri::command]
async fn compile_documents(document_ids: Vec<String>, force: Option<bool>) -> Result<Vec<llm::CompileOutcome>, String> {
    let force = force.unwrap_or(false);
    let vault = current_vault()?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let client = compile::client_from_settings(&vault)?;
        compile::compile_documents(&vault, &document_ids, &client, force)
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
        let compile_outcomes = compile::compile_documents(&vault, &[], &client, false)?;
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
        .invoke_handler(tauri::generate_handler![
            get_profiles,
            create_vault,
            open_vault,
            get_overview,
            add_documents,
            list_documents,
            get_document,
            list_chunks,
            save_document_file,
            create_category,
            list_categories,
            rename_category,
            delete_category,
            set_document_category,
            run_tasks,
            list_jobs,
            search_documents,
            qa_ask,
            list_conversations,
            list_messages,
            rename_conversation,
            delete_conversation,
            get_settings,
            set_setting,
            get_default_prompt,
            test_llm_connection,
            compile_documents,
            index_documents,
            process_documents
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
