export interface DocumentSummary {
    id: string;
    file_name: string;
    mime_type: string;
    type: string;
    status: string;
    size: number;
    created_at: string;
    last_error: string | null;
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

export interface CompileOutcome {
    document_id: string;
    title: string | null;
    tags: string[] | null;
    ok: boolean;
    error: string | null;
}

export interface IndexOutcome {
    document_id: string;
    title: string | null;
    chunks: number;
    ok: boolean;
    error: string | null;
}

export interface ProcessSummary {
    compiled: number;
    compile_failures: number;
    indexed: number;
    index_failures: number;
}
