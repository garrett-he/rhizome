import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { AddSummary, DocumentSummary } from "@/types/document";

const TEXT_FILTERS = [{ name: "纯文本", extensions: ["md", "txt", "srt"] }];

export async function listDocuments(): Promise<DocumentSummary[]> {
    return invoke<DocumentSummary[]>("list_documents");
}

export async function addDocuments(paths: string[]): Promise<AddSummary> {
    return invoke<AddSummary>("add_documents", { paths });
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
