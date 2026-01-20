import { invoke } from "@tauri-apps/api/core";
import type { CategorySummary, DeleteCategoryOutcome } from "@/types/category";

export function listCategories(): Promise<CategorySummary[]> {
    return invoke<CategorySummary[]>("list_categories");
}

export function createCategory(name: string): Promise<CategorySummary> {
    return invoke<CategorySummary>("create_category", { name });
}

export function renameCategory(id: string, name: string): Promise<CategorySummary> {
    return invoke<CategorySummary>("rename_category", { id, name });
}

export function deleteCategory(id: string): Promise<DeleteCategoryOutcome> {
    return invoke<DeleteCategoryOutcome>("delete_category", { id });
}

export function setDocumentCategory(documentId: string, categoryId: string | null): Promise<void> {
    return invoke<void>("set_document_category", { documentId, categoryId });
}
