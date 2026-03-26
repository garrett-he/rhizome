import { Check, Ellipsis, MessageCircle, Pencil, SendHorizontal, Trash2, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import { useNavigate } from "react-router-dom";
import remarkGfm from "remark-gfm";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useQa } from "@/hooks/useQa";
import { confirmAction } from "@/lib/dialog";
import { formatRelativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { ConversationSummary } from "@/types/qa";

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

function RenameConversationForm({
    title,
    busy,
    onConfirm,
    onCancel,
}: {
    title: string;
    busy: boolean;
    onConfirm: (title: string) => void;
    onCancel: () => void;
}) {
    const [value, setValue] = useState(title);
    return (
        <form
            className="flex items-center gap-1 rounded-lg border border-neutral-300 bg-neutral-50 px-2 py-1.5"
            onSubmit={(event) => {
                event.preventDefault();
                onConfirm(value);
            }}
        >
            <Input
                aria-label="会话标题"
                value={value}
                onChange={(event) => setValue(event.target.value)}
                disabled={busy}
                autoFocus
                className="h-7 min-w-0 flex-1 text-[13px]"
            />
            <Button
                type="submit"
                variant="ghost"
                size="sm"
                aria-label="确认重命名"
                disabled={busy || value.trim() === ""}
            >
                <Check aria-hidden className="h-3.5 w-3.5" />
            </Button>
            <Button type="button" variant="ghost" size="sm" aria-label="取消重命名" disabled={busy} onClick={onCancel}>
                <X aria-hidden className="h-3.5 w-3.5" />
            </Button>
        </form>
    );
}

export function QaPage() {
    const navigate = useNavigate();
    const {
        conversations,
        conversationId,
        entries,
        loading,
        asking,
        error,
        ask,
        openConversation,
        newConversation,
        renameConversation,
        removeConversation,
    } = useQa();
    const [question, setQuestion] = useState("");
    const [menuId, setMenuId] = useState<string | null>(null);
    const [renamingId, setRenamingId] = useState<string | null>(null);
    const [renaming, setRenaming] = useState(false);
    const [deleting, setDeleting] = useState(false);
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

    const submitRename = (id: string, title: string) => {
        setRenaming(true);
        void renameConversation(id, title).then(() => {
            setRenaming(false);
            setRenamingId(null);
        });
    };

    const deleteConversationItem = async (conversation: ConversationSummary) => {
        setMenuId(null);
        if (!(await confirmAction(`确定删除会话「${conversation.title}」吗？`))) {
            return;
        }
        setDeleting(true);
        await removeConversation(conversation.id);
        setDeleting(false);
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
                    {conversations.map((conversation, index) => (
                        <div key={conversation.id} className="group relative">
                            {renamingId === conversation.id ? (
                                <RenameConversationForm
                                    title={conversation.title}
                                    busy={renaming}
                                    onConfirm={(title) => submitRename(conversation.id, title)}
                                    onCancel={() => setRenamingId(null)}
                                />
                            ) : (
                                <>
                                    <button
                                        type="button"
                                        onClick={() => void openConversation(conversation.id)}
                                        className={cn(
                                            "flex w-full flex-col gap-0.5 rounded-lg border py-2.5 pl-3 pr-9 text-left transition-colors",
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
                                    <button
                                        type="button"
                                        aria-label="会话操作"
                                        aria-haspopup="menu"
                                        aria-expanded={menuId === conversation.id}
                                        disabled={deleting}
                                        onClick={() => setMenuId(menuId === conversation.id ? null : conversation.id)}
                                        className={cn(
                                            "absolute right-1.5 top-1/2 -translate-y-1/2 rounded-md p-1 text-neutral-500 opacity-0 transition-opacity hover:bg-neutral-200 hover:text-neutral-950 focus-visible:opacity-100 group-hover:opacity-100",
                                            menuId === conversation.id && "opacity-100",
                                        )}
                                    >
                                        <Ellipsis aria-hidden className="h-4 w-4" />
                                    </button>
                                    {menuId === conversation.id && (
                                        <>
                                            <button
                                                type="button"
                                                aria-label="关闭菜单"
                                                className="fixed inset-0 z-10 cursor-default"
                                                onClick={() => setMenuId(null)}
                                            />
                                            <div
                                                role="menu"
                                                className={cn(
                                                    "absolute right-0 z-20 flex w-28 flex-col rounded-md border border-neutral-200 bg-white p-1 shadow-md",
                                                    // 末项向下弹出会被滚动容器裁切，改为向上展开
                                                    index === conversations.length - 1
                                                        ? "bottom-full mb-1"
                                                        : "top-full mt-1",
                                                )}
                                            >
                                                <button
                                                    type="button"
                                                    role="menuitem"
                                                    onClick={() => {
                                                        setMenuId(null);
                                                        setRenamingId(conversation.id);
                                                    }}
                                                    className="flex items-center gap-2 rounded px-2 py-1.5 text-left text-[13px] text-neutral-950 hover:bg-neutral-100"
                                                >
                                                    <Pencil aria-hidden className="h-3.5 w-3.5" />
                                                    重命名
                                                </button>
                                                <button
                                                    type="button"
                                                    role="menuitem"
                                                    onClick={() => void deleteConversationItem(conversation)}
                                                    className="flex items-center gap-2 rounded px-2 py-1.5 text-left text-[13px] text-red-600 hover:bg-red-50"
                                                >
                                                    <Trash2 aria-hidden className="h-3.5 w-3.5" />
                                                    删除
                                                </button>
                                            </div>
                                        </>
                                    )}
                                </>
                            )}
                        </div>
                    ))}
                </div>
            </div>
        </div>
    );
}
