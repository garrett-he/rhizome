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
    /** 本次回答是否用上向量腿；历史消息无该字段，视为未知 */
    vector_used?: boolean;
    created_at: string;
}

export interface AskOutcome {
    conversation_id: string;
    sources: SourceRef[];
    vector_used: boolean;
}
