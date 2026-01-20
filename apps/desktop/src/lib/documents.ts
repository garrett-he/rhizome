import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
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

const TEXT_FILTERS = [{ name: "纯文本", extensions: ["md", "txt", "srt"] }];

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

export async function revealBlob(path: string): Promise<void> {
    await revealItemInDir(path);
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
    const selected = await open({ multiple: true, filters: TEXT_FILTERS });
    if (typeof selected === "string") {
        return [selected];
    }
    if (Array.isArray(selected)) {
        return selected;
    }
    return [];
}
