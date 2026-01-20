import { invoke } from "@tauri-apps/api/core";
import type { AskOutcome, ConversationSummary, Message } from "@/types/qa";

export async function askQuestion(question: string, conversationId?: string): Promise<AskOutcome> {
    return invoke<AskOutcome>("qa_ask", { question, conversationId });
}

export async function listConversations(): Promise<ConversationSummary[]> {
    return invoke<ConversationSummary[]>("list_conversations");
}

export async function listMessages(conversationId: string): Promise<Message[]> {
    return invoke<Message[]>("list_messages", { conversationId });
}
