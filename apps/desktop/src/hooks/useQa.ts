import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import { askQuestion, listConversations, listMessages } from "@/lib/qa";
import type { ConversationSummary, Message, SourceRef } from "@/types/qa";

function toMessage(err: unknown): string {
    if (typeof err === "string") {
        return err;
    }
    if (err instanceof Error) {
        return err.message;
    }
    return String(err);
}

/** 正在流式输出的 AI 消息（id 为负数占位，落库后由 qa-done 固化） */
export interface StreamingMessage {
    role: "assistant";
    content: string;
}

export type ChatEntry =
    | { kind: "message"; message: Message }
    | { kind: "streaming"; key: string; content: string }
    | { kind: "error"; key: string; content: string };

interface QaEventPayload {
    conversation_id: string;
    text?: string;
    sources?: SourceRef[];
    message_id?: number;
    message?: string;
}

export function useQa() {
    const [conversations, setConversations] = useState<ConversationSummary[]>([]);
    const [conversationId, setConversationId] = useState<string | null>(null);
    const [entries, setEntries] = useState<ChatEntry[]>([]);
    const [loading, setLoading] = useState(true);
    const [asking, setAsking] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const conversationIdRef = useRef<string | null>(null);
    conversationIdRef.current = conversationId;
    const streamingRef = useRef(false);
    const entrySeqRef = useRef(0);
    const nextEntryKey = useCallback(() => `entry-${entrySeqRef.current++}`, []);

    const refreshConversations = useCallback(async () => {
        try {
            setConversations(await listConversations());
        } catch (err) {
            setError(toMessage(err));
        }
    }, []);

    // 流式中途切换会话时立即解除输入锁：旧会话的后续事件由 conversation_id 过滤，不会污染新会话
    const resetStreaming = useCallback(() => {
        if (streamingRef.current) {
            streamingRef.current = false;
            setAsking(false);
        }
    }, []);

    const openConversation = useCallback(
        async (id: string) => {
            resetStreaming();
            setConversationId(id);
            try {
                const messages = await listMessages(id);
                setEntries(messages.map((message) => ({ kind: "message" as const, message })));
                setError(null);
            } catch (err) {
                setError(toMessage(err));
            }
        },
        [resetStreaming],
    );

    const newConversation = useCallback(() => {
        resetStreaming();
        setConversationId(null);
        setEntries([]);
        setError(null);
    }, [resetStreaming]);

    useEffect(() => {
        void refreshConversations().finally(() => setLoading(false));
    }, [refreshConversations]);

    useEffect(() => {
        // 先建立监听再提问（qa_ask 在流式结束后才 resolve，事件先行到达）
        const unlisteners: Array<() => void> = [];
        const bind = async () => {
            unlisteners.push(
                await listen<QaEventPayload>("qa-start", (event) => {
                    // 新会话场景：命令 resolve 前 events 先到，用 qa-start 同步会话 id
                    if (conversationIdRef.current === null) {
                        conversationIdRef.current = event.payload.conversation_id;
                        setConversationId(event.payload.conversation_id);
                    }
                }),
                await listen<QaEventPayload>("qa-delta", (event) => {
                    if (event.payload.conversation_id !== conversationIdRef.current) {
                        return;
                    }
                    setEntries((previous) => {
                        const last = previous[previous.length - 1];
                        if (last?.kind === "streaming") {
                            const next = [...previous];
                            next[next.length - 1] = { ...last, content: last.content + (event.payload.text ?? "") };
                            return next;
                        }
                        return [
                            ...previous,
                            { kind: "streaming", key: nextEntryKey(), content: event.payload.text ?? "" },
                        ];
                    });
                }),
                await listen<QaEventPayload>("qa-done", (event) => {
                    if (event.payload.conversation_id !== conversationIdRef.current) {
                        return;
                    }
                    setEntries((previous) => {
                        const last = previous[previous.length - 1];
                        const content = last?.kind === "streaming" ? last.content : "";
                        const next = [...previous];
                        next.pop();
                        next.push({
                            kind: "message",
                            message: {
                                id: event.payload.message_id ?? -1,
                                role: "assistant",
                                content,
                                sources: event.payload.sources ?? [],
                                created_at: new Date().toISOString(),
                            },
                        });
                        return next;
                    });
                    streamingRef.current = false;
                    setAsking(false);
                    void refreshConversations();
                }),
                await listen<QaEventPayload>("qa-error", (event) => {
                    if (event.payload.conversation_id !== conversationIdRef.current) {
                        return;
                    }
                    setEntries((previous) => {
                        const next = [...previous];
                        if (previous[previous.length - 1]?.kind === "streaming") {
                            next.pop();
                        }
                        next.push({ kind: "error", key: nextEntryKey(), content: event.payload.message ?? "回答失败" });
                        return next;
                    });
                    streamingRef.current = false;
                    setAsking(false);
                    void refreshConversations();
                }),
            );
        };
        void bind();
        return () => {
            unlisteners.forEach((unbind) => {
                unbind();
            });
        };
    }, [refreshConversations, nextEntryKey]);

    const ask = useCallback(
        async (question: string) => {
            const trimmed = question.trim();
            if (trimmed === "" || streamingRef.current) {
                return;
            }
            streamingRef.current = true;
            setError(null);
            setAsking(true);
            setEntries((previous) => [
                ...previous,
                {
                    kind: "message",
                    message: {
                        id: -Date.now(),
                        role: "user",
                        content: trimmed,
                        sources: [],
                        created_at: new Date().toISOString(),
                    },
                },
            ]);
            try {
                const outcome = await askQuestion(trimmed, conversationIdRef.current ?? undefined);
                // 命令 resolve 晚于 qa-done：同步会话 id（新建会话场景）。
                // 仅在流式仍活跃且用户未切换会话时同步，避免切换后被旧会话 id 拉回。
                if (streamingRef.current && conversationIdRef.current === null) {
                    conversationIdRef.current = outcome.conversation_id;
                    setConversationId(outcome.conversation_id);
                }
            } catch (err) {
                streamingRef.current = false;
                setAsking(false);
                setEntries((previous) => {
                    // qa-error 事件已展示错误（事件先于命令 reject 到达），避免重复
                    if (previous[previous.length - 1]?.kind === "error") {
                        return previous;
                    }
                    const next = [...previous];
                    if (previous[previous.length - 1]?.kind === "streaming") {
                        next.pop();
                    }
                    next.push({ kind: "error", key: nextEntryKey(), content: toMessage(err) });
                    return next;
                });
            }
        },
        [nextEntryKey],
    );

    return {
        conversations,
        conversationId,
        entries,
        loading,
        asking,
        error,
        ask,
        openConversation,
        newConversation,
    };
}
