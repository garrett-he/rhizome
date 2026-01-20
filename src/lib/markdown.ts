/// 按索引分块标记切分编译稿（详情页渲染视图与切片联动共用）。
///
/// 与后端 indexing.rs 的 split_markdown 同源（`<!-- chunk: label -->` 标记行
/// 切段），但**保留全部文本**（含仅含 H1 标题的无标记导语，渲染视图不能丢标
/// 题）。导语段 label 为空字符串；有 label 的段与后端索引切片按标记出现顺序
/// 一一对应。

/** 匹配一个分块标记行，如 `<!-- chunk: 背景 -->`。 */
const MARKER_RE = /^<!--\s*chunk\s*:\s*(.*?)\s*-->\s*$/;

export interface MarkdownSection {
    /** 分块标记 label；无标记导语段为空字符串 */
    label: string;
    /** 该段 Markdown 文本（不含标记行，首尾空白已裁剪） */
    text: string;
}

/**
 * 切分编译稿：每个标记行开启新段（标记本身不进文本），首标记前的内容作为
 * 无标记导语段保留。空段被丢弃。
 */
export function splitByChunkMarkers(markdown: string): MarkdownSection[] {
    const sections: MarkdownSection[] = [];
    let pending: string[] = [];
    let label = "";

    const flush = () => {
        const text = pending.join("\n").trim();
        pending = [];
        if (!text) {
            return;
        }
        sections.push({ label, text });
    };

    for (const line of markdown.split(/\r?\n/)) {
        const match = line.trim().match(MARKER_RE);
        if (match === null) {
            pending.push(line);
            continue;
        }
        flush();
        label = match[1] ?? "";
    }
    flush();
    return sections;
}

/** 有 label 的段数（应与后端索引切片数一致）。 */
export function labeledSectionCount(sections: MarkdownSection[]): number {
    return sections.filter((s) => s.label !== "").length;
}

/** 截断 sha1 用于展示：前 6 位 + … + 后 4 位（长度不足时原样返回）。 */
export function shortSha1(sha1: string): string {
    if (sha1.length <= 10) {
        return sha1;
    }
    return `${sha1.slice(0, 6)}…${sha1.slice(-4)}`;
}

/** 中文字数（按字符计，与设计稿「380 字」口径一致）。 */
export function charCount(text: string): string {
    return `${text.length.toLocaleString()} 字`;
}
