import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
    AddSummary,
    ChunkInfo,
    CompileOutcome,
    DocumentDetail,
    DocumentSummary,
    IndexOutcome,
    ProcessSummary,
    SearchHit,
} from "@/types/document";

export async function listDocuments(): Promise<DocumentSummary[]> {
    return invoke<DocumentSummary[]>("list_documents");
}

export async function addDocuments(paths: string[], categoryName?: string): Promise<AddSummary> {
    return invoke<AddSummary>("add_documents", { paths, categoryName: categoryName ?? null });
}

export async function getDocument(id: string): Promise<DocumentDetail> {
    return invoke<DocumentDetail>("get_document", { id });
}

export async function listChunks(id: string): Promise<ChunkInfo[]> {
    return invoke<ChunkInfo[]>("list_chunks", { documentId: id });
}

/** 弹保存对话框（默认文件名为原始文件名），将 blob 中的原始文件另存为到所选路径。 */
export async function saveDocumentFile(id: string, fileName: string): Promise<void> {
    const target = await save({ defaultPath: fileName });
    if (target === null) {
        return;
    }
    await invoke("save_document_file", { id, target });
}

export async function compileDocuments(documentIds: string[], force = false): Promise<CompileOutcome[]> {
    return invoke<CompileOutcome[]>("compile_documents", { documentIds, force });
}

export async function indexDocuments(documentIds: string[]): Promise<IndexOutcome[]> {
    return invoke<IndexOutcome[]>("index_documents", { documentIds });
}

export async function processDocuments(): Promise<ProcessSummary> {
    return invoke<ProcessSummary>("process_documents");
}

export async function searchDocuments(query: string, limit?: number): Promise<SearchHit[]> {
    return invoke<SearchHit[]>("search_documents", { query, limit });
}

export async function pickFiles(): Promise<string[]> {
    const selected = await open({ multiple: true });
    if (typeof selected === "string") {
        return [selected];
    }
    if (Array.isArray(selected)) {
        return selected;
    }
    return [];
}
