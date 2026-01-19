export interface CategoryStat {
    name: string;
    count: number;
}

export interface Overview {
    total_documents: number;
    indexed_documents: number;
    pending_documents: number;
    failed_documents: number;
    storage_bytes: number;
    categories: CategoryStat[];
}
