export interface DocumentSummary {
    id: string;
    file_name: string;
    mime_type: string;
    type: string;
    status: string;
    size: number;
    created_at: string;
}

export interface AddFailure {
    path: string;
    error: string;
}

export interface AddSummary {
    added: number;
    skipped: number;
    failures: AddFailure[];
}
