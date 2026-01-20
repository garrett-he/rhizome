//! 编译稿 Markdown 分块纯函数（移植 deprecated core/indexing.py）。
//!
//! `split_markdown` 把 `content_compiled` 切成待向量化的文本块。编译稿由 LLM
//! 预先用 `<!-- chunk: <label> -->` 标记主题边界（见 prompts/docs_compile.txt），
//! 每个标记开启新块，标记本身不进入块文本，标签随块一并存储。
//! 本模块刻意保持纯函数：不触数据库与模型，便于单元测试。

/// 块长度上限（字符数），超出后按段落边界再切、超长段落硬切。
const MAX_CHUNK_CHARS: usize = 1200;

/// 编译稿的一个可检索单元：标记标签（无标记导语为空）与块文本。
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TextChunk {
    pub label: String,
    pub text: String,
}

/// 匹配块标记行，如 `<!-- chunk: 背景 -->`；捕获组为标签。
/// 对齐旧版正则 `^<!--\s*chunk\s*:\s*(.*?)\s*-->\s*$`：整行锚定，空白均可选。
fn marker_label(line: &str) -> Option<&str> {
    let s = line.trim();
    let s = s.strip_prefix("<!--")?;
    let s = s.trim_start();
    let s = s.strip_prefix("chunk")?;
    let s = s.trim_start();
    let s = s.strip_prefix(':')?;
    let s = s.trim_start();
    let s = s.strip_suffix("-->")?;
    let label = s.trim();
    Some(label)
}

/// 匹配一级 ATX 标题，如 `# Compiled Title`。
fn is_h1(line: &str) -> bool {
    line.starts_with("# ")
}

/// 把超长文本切成不超过 `MAX_CHUNK_CHARS` 的若干片：
/// 优先段落（空行）边界，超长段落按字符硬切，单片永不超预算。
fn split_long_text(text: &str) -> Vec<String> {
    if text.chars().count() <= MAX_CHUNK_CHARS {
        if text.is_empty() {
            return Vec::new();
        }
        return vec![text.to_string()];
    }

    let mut pieces: Vec<String> = Vec::new();
    let paragraphs: Vec<&str> = text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()).collect();
    let mut current = String::new();
    for paragraph in paragraphs {
        let mut paragraph = paragraph;
        while paragraph.chars().count() > MAX_CHUNK_CHARS {
            if !current.is_empty() {
                pieces.push(std::mem::take(&mut current));
            }
            let head: String = paragraph.chars().take(MAX_CHUNK_CHARS).collect();
            pieces.push(head);
            paragraph = &paragraph[paragraph
                .char_indices()
                .nth(MAX_CHUNK_CHARS)
                .map(|(i, _)| i)
                .unwrap_or(paragraph.len())..];
        }
        if !current.is_empty() && current.chars().count() + paragraph.chars().count() + 1 > MAX_CHUNK_CHARS {
            pieces.push(std::mem::take(&mut current));
        }
        if current.is_empty() {
            current = paragraph.to_string();
        } else {
            current.push_str("\n\n");
            current.push_str(paragraph);
        }
    }
    if !current.is_empty() {
        pieces.push(current);
    }
    pieces
}

/// 丢弃只承载文档标题（仅空白或一级标题行）的无标签首块。
fn drop_title_lead_in(chunks: &mut Vec<TextChunk>) {
    let Some(first) = chunks.first() else { return };
    if !first.label.is_empty() {
        return;
    }
    let all_title = first.text.lines().all(|line| is_h1(line) || line.trim().is_empty());
    if all_title {
        chunks.remove(0);
    }
}

/// 按标记行切分编译稿：每个 `<!-- chunk: <label> -->` 标记开启携带其标签的新块；
/// 首个标记前的内容保留为无标签导语块，除非仅由空白或一级标题构成。
/// 超过 `MAX_CHUNK_CHARS` 的块再按段落边界切分。空块被丢弃。
pub fn split_markdown(markdown: &str) -> Vec<TextChunk> {
    let mut chunks: Vec<TextChunk> = Vec::new();
    let mut pending: Vec<&str> = Vec::new();
    let mut label = String::new();

    for line in markdown.lines() {
        if let Some(found) = marker_label(line) {
            flush(&mut pending, &label, &mut chunks);
            label = found.to_string();
            continue;
        }
        pending.push(line);
    }
    flush(&mut pending, &label, &mut chunks);
    drop_title_lead_in(&mut chunks);
    chunks
}

/// 把当前积累的行合并为一个块（可再被切分）并落入 chunks。
fn flush(pending: &mut Vec<&str>, label: &str, chunks: &mut Vec<TextChunk>) {
    let text = pending.join("\n").trim().to_string();
    pending.clear();
    if text.is_empty() {
        return;
    }
    for piece in split_long_text(&text) {
        chunks.push(TextChunk {
            label: label.to_string(),
            text: piece,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(chunks: &[TextChunk]) -> Vec<&str> {
        chunks.iter().map(|c| c.label.as_str()).collect()
    }

    #[test]
    fn test_empty_markdown_yields_no_chunks() {
        assert!(split_markdown("").is_empty());
        assert!(split_markdown("   \n\n  ").is_empty());
    }

    #[test]
    fn test_markers_split_chunks_and_are_not_in_text() {
        let md = "# 标题\n\n<!-- chunk: 背景 -->\n正文甲\n\n<!-- chunk: 方法 -->\n正文乙";
        let chunks = split_markdown(md);

        assert_eq!(chunks.len(), 2);
        assert_eq!(labels(&chunks), ["背景", "方法"]);
        assert!(!chunks.iter().any(|c| c.text.contains("chunk")));
        assert_eq!(chunks[0].text, "正文甲");
        assert_eq!(chunks[1].text, "正文乙");
    }

    #[test]
    fn test_title_only_lead_in_is_dropped() {
        let md = "# 标题\n\n<!-- chunk: 正文 -->\n内容";
        let chunks = split_markdown(md);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].label, "正文");
    }

    #[test]
    fn test_lead_in_with_body_is_kept() {
        let md = "# 标题\n\n导语段落。\n\n<!-- chunk: 正文 -->\n内容";
        let chunks = split_markdown(md);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].label, "");
        assert!(chunks[0].text.contains("导语段落"));
    }

    #[test]
    fn test_h2_lead_in_is_kept() {
        // 仅含二级标题的导语不属于"只有 H1"，保留
        let md = "## 前言\n\n<!-- chunk: 正文 -->\n内容";
        let chunks = split_markdown(md);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].label, "");
    }

    #[test]
    fn test_long_chunk_splits_at_paragraph_boundaries() {
        let p = |n: usize| format!("段落{n}{}", "字".repeat(700));
        let md = format!("<!-- chunk: 大块 -->\n{}\n\n{}", p(1), p(2));
        let chunks = split_markdown(&md);

        assert!(chunks.len() >= 2);
        for chunk in &chunks {
            assert!(chunk.text.chars().count() <= MAX_CHUNK_CHARS);
        }
        assert!(chunks.iter().all(|c| c.label == "大块"));
    }

    #[test]
    fn test_overlong_paragraph_is_hard_split() {
        let text = "字".repeat(2500);
        let md = format!("<!-- chunk: 巨块 -->\n{text}");
        let chunks = split_markdown(&md);

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].text.chars().count(), MAX_CHUNK_CHARS);
        assert_eq!(chunks[2].text.chars().count(), 100);
    }

    #[test]
    fn test_marker_label_variants() {
        assert_eq!(marker_label("<!-- chunk: a b -->"), Some("a b"));
        assert_eq!(marker_label("<!--chunk:a-->"), Some("a"));
        assert_eq!(marker_label("<!-- chunk : a -->"), Some("a"));
        assert_eq!(marker_label("普通文本"), None);
        assert_eq!(marker_label("<!-- not chunk -->"), None);
    }

    #[test]
    fn test_split_long_text_short_input_passthrough() {
        assert_eq!(split_long_text(""), Vec::<String>::new());
        assert_eq!(split_long_text("短文本"), vec!["短文本".to_string()]);
    }
}
