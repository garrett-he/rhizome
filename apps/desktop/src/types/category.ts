export interface CategorySummary {
    id: string;
    name: string;
    document_count: number;
}

export interface DeleteCategoryOutcome {
    detached: number;
}
