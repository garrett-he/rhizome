export interface SourceRef {
    chunk_id: number;
    document_id: string;
    title: string;
    chunk_index: number;
}

export interface ConversationSummary {
    id: string;
    title: string;
    updated_at: string;
}

export interface Message {
    id: number;
    role: "user" | "assistant";
    content: string;
    sources: SourceRef[];
    created_at: string;
}

export interface AskOutcome {
    conversation_id: string;
    sources: SourceRef[];
}
