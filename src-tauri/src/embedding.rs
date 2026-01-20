//! 本地 embedding 服务（对齐 deprecated core/embedding.py 的 fastembed 实现）。
//!
//! 用 ort（ONNX Runtime）+ tokenizers 跑 `BAAI/bge-small-zh-v1.5`（384 维，
//! CLS pooling + L2 归一化），与旧版 fastembed 输出一致。模型文件（model.onnx
//! 与 tokenizer.json）首次使用时从 HuggingFace 下载缓存到
//! `{cache_dir}/rhizome/models/bge-small-zh-v1.5/`，`HF_ENDPOINT` 环境变量
//! 可切换镜像站（如 https://hf-mirror.com）。
//!
//! `EmbeddingModel` trait 保持向量化可替换：测试注入确定性 fake，不下载模型。

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

/// 本期唯一内置的本地嵌入模型（与前端设置页默认值一致）。
pub const SUPPORTED_MODEL: &str = "BAAI/bge-small-zh-v1.5";
/// ONNX 权重所在的 HuggingFace 仓库（fastembed 同源）。
const MODEL_REPO: &str = "Qdrant/bge-small-zh-v1.5";
/// 模型输出维度（bge-small 系列 = 384）。
const DIMENSION: usize = 384;

/// 批量文本向量化接口。
pub trait EmbeddingModel: Send + Sync {
    fn dimension(&self) -> usize;
    /// 返回与 `texts` 一一对应的归一化向量。
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String>;
}

/// L2 归一化（零向量原样返回，避免除零）。
pub fn normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for x in vector.iter_mut() {
            *x /= norm;
        }
    }
}

/// 返回内置本地模型；非内置名称报明确错误（本期不做远程 embedding API）。
pub fn default_embedding_model(model_name: &str) -> Result<Arc<dyn EmbeddingModel>, String> {
    if model_name != SUPPORTED_MODEL {
        return Err(format!(
            "不支持的本地嵌入模型: {model_name}（当前仅支持 {SUPPORTED_MODEL}）"
        ));
    }
    static CACHED: OnceLock<Result<Arc<FastembedModel>, String>> = OnceLock::new();
    match CACHED.get_or_init(|| FastembedModel::load().map(Arc::new)) {
        Ok(model) => Ok(model.clone() as Arc<dyn EmbeddingModel>),
        Err(error) => Err(error.clone()),
    }
}

impl FastembedModel {
    /// 加载（必要时下载）模型文件并构建推理会话。
    pub fn load() -> Result<Self, String> {
        crate::logging::log("INFO", &format!("开始加载嵌入模型 {SUPPORTED_MODEL}"));
        let dir = model_cache_dir();
        let tokenizer_path = ensure_file(&dir, "tokenizer.json", &tokenizer_url())?;
        let onnx_path = ensure_file(&dir, onnx_file_name(), &onnx_url())?;

        let tokenizer = match tokenizers::Tokenizer::from_file(&tokenizer_path) {
            Ok(tokenizer) => tokenizer,
            Err(e) => {
                crate::logging::log("ERROR", &format!("加载 tokenizer 失败: {e}"));
                return Err(format!("加载 tokenizer 失败: {e}"));
            }
        };
        let environment = ort::init()
            .with_name("rhizome")
            .build()
            .map_err(|e| format!("初始化 ONNX 环境失败: {e}"))?;
        let session = match ort::session::Session::builder(&environment)
            .map_err(|e| format!("创建会话构建器失败: {e}"))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| format!("配置 ONNX 会话失败: {e}"))?
            .with_intra_threads(num_threads())
            .map_err(|e| format!("配置 ONNX 会话失败: {e}"))?
            .commit_from_file(&onnx_path)
        {
            Ok(session) => session,
            Err(e) => {
                crate::logging::log("ERROR", &format!("加载 ONNX 模型失败: {e}"));
                return Err(format!("加载 ONNX 模型失败: {e}"));
            }
        };
        crate::logging::log("INFO", &format!("嵌入模型加载完成: {}", onnx_path.display()));
        Ok(Self {
            session: std::sync::Mutex::new(session),
            tokenizer,
        })
    }
}

/// ort + tokenizers 实现的 fastembed 等价模型。
pub struct FastembedModel {
    session: std::sync::Mutex<ort::session::Session>,
    tokenizer: tokenizers::Tokenizer,
}

/// 单条文本前向推理，返回 CLS 向量（未归一化）。
fn forward(
    session: &mut ort::session::Session,
    tokenizer: &tokenizers::Tokenizer,
    text: &str,
) -> Result<Vec<f32>, String> {
    let encoding = tokenizer.encode(text, true).map_err(|e| format!("分词失败: {e}"))?;
    // ONNX 输入 dtype 为 int64
    let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| i64::from(x)).collect();
    let attention: Vec<i64> = encoding.get_attention_mask().iter().map(|&x| i64::from(x)).collect();
    let types: Vec<i64> = encoding.get_type_ids().iter().map(|&x| i64::from(x)).collect();
    let seq = ids.len();
    if seq == 0 {
        return Err("分词结果为空".to_string());
    }
    let input_err = |e: ort::Error| format!("构建输入失败: {e}");

    let inputs = ort::inputs![
        "input_ids" => ort::value::Value::from_array(([1usize, seq], ids)).map_err(input_err)?,
        "attention_mask" => ort::value::Value::from_array(([1usize, seq], attention)).map_err(input_err)?,
        "token_type_ids" => ort::value::Value::from_array(([1usize, seq], types)).map_err(input_err)?,
    ];

    let outputs = session.run(inputs).map_err(|e| format!("模型推理失败: {e}"))?;
    let extracted = outputs["last_hidden_state"]
        .try_extract_array::<f32>()
        .map_err(|e| format!("读取模型输出失败: {e}"))?;
    // CLS pooling：取 [1, seq, hidden] 的首个 token 向量
    Ok(extracted.iter().take(DIMENSION).copied().collect())
}

impl EmbeddingModel for FastembedModel {
    fn dimension(&self) -> usize {
        DIMENSION
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        let mut session = self.session.lock().map_err(|e| format!("模型会话锁失败: {e}"))?;
        let mut vectors = Vec::with_capacity(texts.len());
        for text in texts {
            let mut vector = forward(&mut session, &self.tokenizer, text)?;
            normalize(&mut vector);
            vectors.push(vector);
        }
        Ok(vectors)
    }
}

/// 模型缓存目录：{cache_dir}/rhizome/models/bge-small-zh-v1.5/。
pub fn model_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("rhizome")
        .join("models")
        .join("bge-small-zh-v1.5")
}

/// HuggingFace 端点：默认 hf-mirror.com（国内可直连），`HF_ENDPOINT` 可覆盖
/// 为官方端点或其它镜像。
fn hf_endpoint() -> String {
    std::env::var("HF_ENDPOINT").unwrap_or_else(|_| "https://hf-mirror.com".to_string())
}

fn tokenizer_url() -> String {
    format!("{hf}/{MODEL_REPO}/resolve/main/tokenizer.json", hf = hf_endpoint())
}

/// ONNX 权重文件名（Qdrant/bge-small-zh-v1.5 仓库根目录，无 onnx/ 子目录）。
fn onnx_file_name() -> &'static str {
    "model_optimized.onnx"
}

fn onnx_url() -> String {
    format!(
        "{hf}/{MODEL_REPO}/resolve/main/{file}",
        hf = hf_endpoint(),
        file = onnx_file_name()
    )
}

/// 文件存在且非空则跳过下载，否则流式下载到 .part 再原子改名。
fn ensure_file(dir: &std::path::Path, name: &str, url: &str) -> Result<PathBuf, String> {
    let path = dir.join(name);
    if std::fs::metadata(&path).map(|m| m.len() > 0).unwrap_or(false) {
        return Ok(path);
    }
    crate::logging::log("INFO", &format!("开始下载模型文件: {url}"));
    std::fs::create_dir_all(dir).map_err(|e| format!("创建模型目录失败: {e}"))?;
    let tmp = dir.join(format!("{name}.part"));
    let mut response = match reqwest::blocking::get(url) {
        Ok(response) => response,
        Err(e) => {
            crate::logging::log("ERROR", &format!("下载模型文件失败: {url}: {e}"));
            return Err(format!("下载模型文件失败: {e}"));
        }
    };
    if !response.status().is_success() {
        let status = response.status();
        crate::logging::log("ERROR", &format!("下载模型文件失败: {url}: HTTP {status}"));
        return Err(format!("下载模型文件失败: HTTP {status}"));
    }
    let mut file = match std::fs::File::create(&tmp) {
        Ok(file) => file,
        Err(e) => {
            crate::logging::log("ERROR", &format!("创建临时文件失败: {}: {e}", tmp.display()));
            return Err(format!("创建临时文件失败: {e}"));
        }
    };
    if let Err(e) = response.copy_to(&mut file) {
        crate::logging::log("ERROR", &format!("保存模型文件失败: {url}: {e}"));
        return Err(format!("保存模型文件失败: {e}"));
    }
    drop(file);
    std::fs::rename(&tmp, &path).map_err(|e| format!("就位模型文件失败: {e}"))?;
    crate::logging::log("INFO", &format!("模型文件就位: {}", path.display()));
    Ok(path)
}

fn num_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(4)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn test_normalize_unit_vector() {
        let mut v = vec![3.0, 4.0];
        normalize(&mut v);
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_zero_vector_is_noop() {
        let mut v = vec![0.0; 4];
        normalize(&mut v);
        assert!(v.iter().all(|x| *x == 0.0));
    }

    #[test]
    fn test_default_embedding_model_rejects_unknown() {
        let err = match default_embedding_model("text-embedding-3-small") {
            Err(e) => e,
            Ok(_) => panic!("未知模型应当报错"),
        };
        assert!(err.contains("不支持的本地嵌入模型"));
        assert!(err.contains(SUPPORTED_MODEL));
    }

    #[test]
    fn test_urls_default_to_mirror_endpoint() {
        // 默认使用国内可直连的 hf-mirror.com；HF_ENDPOINT 可覆盖为官方端点
        std::env::remove_var("HF_ENDPOINT");
        assert!(tokenizer_url().starts_with("https://hf-mirror.com/"));
        assert!(tokenizer_url().contains(MODEL_REPO));
        assert!(onnx_url().ends_with(&format!("/resolve/main/{}", onnx_file_name())));
        assert_eq!(onnx_file_name(), "model_optimized.onnx");
        std::env::set_var("HF_ENDPOINT", "https://huggingface.co");
        assert!(tokenizer_url().starts_with("https://huggingface.co/"));
        std::env::remove_var("HF_ENDPOINT");
    }

    #[test]
    fn test_model_cache_dir_layout() {
        let dir = model_cache_dir();
        assert!(dir.ends_with("rhizome/models/bge-small-zh-v1.5"));
    }

    /// fake：确定性向量（对齐旧版 EmbeddingModel protocol 用法），不触真实模型。
    pub(crate) struct FakeEmbedding {
        pub dimension: usize,
    }

    impl EmbeddingModel for FakeEmbedding {
        fn dimension(&self) -> usize {
            self.dimension
        }

        fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
            Ok(texts
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    let mut v = vec![0.0; self.dimension];
                    v[i % self.dimension] = 1.0;
                    v
                })
                .collect())
        }
    }
}
