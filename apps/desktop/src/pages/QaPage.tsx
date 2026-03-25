import { MessageCircle, SendHorizontal } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import { useNavigate } from "react-router-dom";
import remarkGfm from "remark-gfm";
import { Button } from "@/components/ui/button";
import { useQa } from "@/hooks/useQa";
import { formatRelativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";

const CIRCLED = ["①", "②", "③", "④", "⑤", "⑥", "⑦", "⑧", "⑨", "⑩"];

function sourceLabel(index: number): string {
    return CIRCLED[index] ?? `(${index + 1})`;
}

/** AI 回答按 Markdown 渲染；streaming 时在末尾追加光标。 */
function MarkdownContent({ content, streaming = false }: { content: string; streaming?: boolean }) {
    return (
        <div className="flex min-w-0 flex-col gap-2 text-sm leading-[1.7] text-neutral-950">
            <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                components={{
                    h1: ({ children }) => <h1 className="text-lg font-semibold">{children}</h1>,
                    h2: ({ children }) => <h2 className="text-[15px] font-semibold">{children}</h2>,
                    h3: ({ children }) => <h3 className="text-sm font-semibold">{children}</h3>,
                    p: ({ children }) => <p className="whitespace-pre-wrap">{children}</p>,
                    ul: ({ children }) => <ul className="list-disc pl-5">{children}</ul>,
                    ol: ({ children }) => <ol className="list-decimal pl-5">{children}</ol>,
                    code: ({ children, className }) =>
                        className?.includes("language-") ? (
                            <code className="block whitespace-pre-wrap rounded-md bg-muted p-3 text-[13px] leading-6">
                                {children}
                            </code>
                        ) : (
                            <code className="rounded bg-muted px-1 py-0.5 text-[13px]">{children}</code>
                        ),
                    pre: ({ children }) => <pre className="overflow-x-auto">{children}</pre>,
                    blockquote: ({ children }) => (
                        <blockquote className="border-l-2 border-border pl-3 text-muted-foreground">
                            {children}
                        </blockquote>
                    ),
                    a: ({ children, href }) => (
                        <a href={href} className="text-primary underline underline-offset-2">
                            {children}
                        </a>
                    ),
                }}
            >
                {content}
            </ReactMarkdown>
            {streaming && (
                <span aria-hidden className="animate-pulse">
                    ▍
                </span>
            )}
        </div>
    );
}

export function QaPage() {
    const navigate = useNavigate();
    const { conversations, conversationId, entries, loading, asking, error, ask, openConversation, newConversation } =
        useQa();
    const [question, setQuestion] = useState("");
    const scrollRef = useRef<HTMLDivElement>(null);

    // 新消息/流式追加时自动滚到底
    useEffect(() => {
        if (entries.length === 0) {
            return;
        }
        scrollRef.current?.scrollTo?.({ top: scrollRef.current.scrollHeight });
    }, [entries]);

    const submit = () => {
        const trimmed = question.trim();
        if (trimmed === "" || asking) {
            return;
        }
        setQuestion("");
        void ask(trimmed);
    };

    return (
        <div className="flex h-full gap-5 p-8">
            <div className="flex min-w-0 flex-1 flex-col gap-4">
                <div ref={scrollRef} className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto pr-1">
                    {entries.length === 0 && !loading && (
                        <div className="flex flex-1 items-center justify-center text-[13px] text-neutral-500">
                            {conversationId === null ? "输入问题，从知识库中寻找答案" : "该会话暂无消息"}
                        </div>
                    )}
                    {entries.map((entry) => {
                        if (entry.kind === "message" && entry.message.role === "user") {
                            return (
                                <div key={`m-${entry.message.id}`} className="flex justify-end">
                                    <div className="max-w-[320px] rounded-xl bg-secondary px-3.5 py-2.5 text-sm text-secondary-foreground">
                                        {entry.message.content}
                                    </div>
                                </div>
                            );
                        }
                        if (entry.kind === "message") {
                            return (
                                <div key={`m-${entry.message.id}`} className="flex gap-2.5">
                                    <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-zinc-200 text-xs font-medium text-zinc-700">
                                        AI
                                    </div>
                                    <div className="flex min-w-0 flex-1 flex-col gap-2.5 rounded-xl border border-neutral-200 bg-neutral-50 px-4 py-3.5">
                                        <MarkdownContent content={entry.message.content} />
                                        {entry.message.sources.length > 0 && (
                                            <div className="flex flex-col gap-1.5">
                                                <span className="text-xs text-neutral-500">引用来源</span>
                                                <div className="flex flex-wrap gap-1.5">
                                                    {entry.message.sources.map((source, sourceIndex) => (
                                                        <button
                                                            key={source.chunk_id}
                                                            type="button"
                                                            onClick={() =>
                                                                navigate(
                                                                    `/documents/${source.document_id}?chunk=${source.chunk_id}`,
                                                                )
                                                            }
                                                            className="rounded-md border border-neutral-200 bg-neutral-50 px-2 py-0.5 text-xs text-neutral-700 transition-colors hover:bg-neutral-100"
                                                        >
                                                            {sourceLabel(sourceIndex)} {source.title}
                                                        </button>
                                                    ))}
                                                </div>
                                            </div>
                                        )}
                                    </div>
                                </div>
                            );
                        }
                        if (entry.kind === "streaming") {
                            return (
                                <div key={entry.key} className="flex gap-2.5">
                                    <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-zinc-200 text-xs font-medium text-zinc-700">
                                        AI
                                    </div>
                                    <div className="flex min-w-0 flex-1 flex-col rounded-xl border border-neutral-200 bg-neutral-50 px-4 py-3.5">
                                        <MarkdownContent content={entry.content} streaming />
                                    </div>
                                </div>
                            );
                        }
                        return (
                            <div key={entry.key} className="flex justify-center">
                                <span role="alert" className="text-[13px] text-red-600">
                                    {entry.content}
                                </span>
                            </div>
                        );
                    })}
                </div>

                {error && (
                    <p role="alert" className="text-[13px] text-red-600">
                        {error}
                    </p>
                )}

                <form
                    className="flex items-center gap-2"
                    onSubmit={(event) => {
                        event.preventDefault();
                        submit();
                    }}
                >
                    <input
                        type="text"
                        value={question}
                        onChange={(event) => setQuestion(event.target.value)}
                        placeholder="输入问题…（检索范围：全部知识库）"
                        aria-label="问题"
                        disabled={asking}
                        className="h-9 min-w-0 flex-1 rounded-md border border-neutral-200 bg-neutral-50 px-3 text-sm text-neutral-950 outline-none placeholder:text-neutral-400 focus:border-neutral-400"
                    />
                    <Button type="submit" disabled={asking || question.trim() === ""} aria-label="发送">
                        <SendHorizontal aria-hidden className="h-4 w-4" />
                    </Button>
                </form>
            </div>

            <div className="flex w-[260px] shrink-0 flex-col gap-3">
                <div className="flex items-center justify-between">
                    <span className="text-sm font-semibold text-neutral-950">历史对话</span>
                    <Button type="button" variant="outline" size="sm" onClick={newConversation}>
                        <MessageCircle aria-hidden className="h-3.5 w-3.5" />
                        新会话
                    </Button>
                </div>
                <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto">
                    {conversations.length === 0 && <p className="text-[13px] text-neutral-500">暂无历史会话</p>}
                    {conversations.map((conversation) => (
                        <button
                            key={conversation.id}
                            type="button"
                            onClick={() => void openConversation(conversation.id)}
                            className={cn(
                                "flex w-full flex-col gap-0.5 rounded-lg border px-3 py-2.5 text-left transition-colors",
                                conversation.id === conversationId
                                    ? "border-neutral-300 bg-neutral-100"
                                    : "border-neutral-200 bg-neutral-50 hover:bg-neutral-100",
                            )}
                        >
                            <span className="truncate text-[13px] font-medium text-neutral-950">
                                {conversation.title}
                            </span>
                            <span className="text-xs text-neutral-500">
                                {formatRelativeTime(conversation.updated_at)}
                            </span>
                        </button>
                    ))}
                </div>
            </div>
        </div>
    );
}
