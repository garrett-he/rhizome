import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { AddSummary, CompileOutcome, DocumentSummary, IndexOutcome, ProcessSummary } from "@/types/document";

const TEXT_FILTERS = [{ name: "纯文本", extensions: ["md", "txt", "srt"] }];

export async function listDocuments(): Promise<DocumentSummary[]> {
    return invoke<DocumentSummary[]>("list_documents");
}

export async function addDocuments(paths: string[]): Promise<AddSummary> {
    return invoke<AddSummary>("add_documents", { paths });
}

export async function compileDocuments(documentIds: string[]): Promise<CompileOutcome[]> {
    return invoke<CompileOutcome[]>("compile_documents", { documentIds });
}

export async function indexDocuments(documentIds: string[]): Promise<IndexOutcome[]> {
    return invoke<IndexOutcome[]>("index_documents", { documentIds });
}

export async function processDocuments(): Promise<ProcessSummary> {
    return invoke<ProcessSummary>("process_documents");
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
