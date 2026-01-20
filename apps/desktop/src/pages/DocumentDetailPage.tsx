import { Boxes, ExternalLink, Loader2, RefreshCw, WandSparkles } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import { Link, useParams, useSearchParams } from "react-router-dom";
import remarkGfm from "remark-gfm";
import { Alert, AlertDescription, AlertIcon, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs } from "@/components/ui/tabs";
import { useCategories } from "@/hooks/useCategories";
import { useDocumentDetail } from "@/hooks/useDocumentDetail";
import { setDocumentCategory } from "@/lib/categories";
import { confirmAction } from "@/lib/dialog";
import { compileDocuments, indexDocuments, revealBlob } from "@/lib/documents";
import { formatBytes, formatShortDate } from "@/lib/format";
import { charCount, shortSha1, splitByChunkMarkers } from "@/lib/markdown";
import { cn } from "@/lib/utils";

const STATUS_LABELS: Record<string, string> = {
    raw: "已导入",
    compiled: "已编译",
    indexed: "已索引",
};

const TYPE_LABELS: Record<string, string> = {
    plaintext: "文本",
};

interface CompiledMetadata {
    title?: unknown;
    tags?: unknown;
    language?: unknown;
    summary?: unknown;
}

function asText(value: unknown): string | null {
    return typeof value === "string" && value.trim() !== "" ? value : null;
}

function asTags(value: unknown): string[] {
    if (!Array.isArray(value)) {
        return [];
    }
    return value.filter((tag): tag is string => typeof tag === "string" && tag.trim() !== "");
}

function parseMetadata(raw: string | null): CompiledMetadata {
    if (!raw) {
        return {};
    }
    try {
        const parsed: unknown = JSON.parse(raw);
        return parsed !== null && typeof parsed === "object" ? (parsed as CompiledMetadata) : {};
    } catch {
        return {};
    }
}

/** 视图模式：渲染视图 / 原始文本 */
type ViewMode = "rendered" | "raw";

export function DocumentDetailPage() {
    const { id = "" } = useParams();
    const { detail, chunks, loading, error, refresh } = useDocumentDetail(id);
    const { categories } = useCategories();

    const [viewMode, setViewMode] = useState<ViewMode>("rendered");
    const [busy, setBusy] = useState<string | null>(null);
    const [actionError, setActionError] = useState<string | null>(null);
    const [selectedChunkId, setSelectedChunkId] = useState<number | null>(null);
    const sectionRefs = useRef<Map<number, HTMLElement>>(new Map());
    const [searchParams, setSearchParams] = useSearchParams();

    // 搜索页「展开全文」通过 ?chunk=<id> 初始定位：数据加载完成后选中并滚动，
    // 随后一次性消费该参数（replace 不留历史）
    useEffect(() => {
        if (detail === null || chunks.length === 0) {
            return;
        }
        const chunkParam = searchParams.get("chunk");
        if (chunkParam === null) {
            return;
        }
        const chunkId = Number(chunkParam);
        const index = chunks.findIndex((chunk) => chunk.id === chunkId);
        if (Number.isFinite(chunkId) && index >= 0) {
            setSelectedChunkId(chunkId);
            // 默认渲染视图已随本次提交渲染，refs 已填充
            sectionRefs.current.get(index)?.scrollIntoView({ behavior: "smooth", block: "start" });
        }
        setSearchParams({}, { replace: true });
    }, [detail, chunks, searchParams, setSearchParams]);

    if (loading) {
        return <div className="flex flex-1 items-center justify-center p-8 text-[13px] text-neutral-500">加载中…</div>;
    }

    if (error !== null || detail === null) {
        return (
            <div className="flex flex-col gap-4 p-8">
                <p role="alert" className="text-[13px] text-red-600">
                    {error ?? "文档不存在"}
                </p>
                <Link to="/documents" className="text-[13px] text-primary hover:underline">
                    返回文档库
                </Link>
            </div>
        );
    }

    const metadata = parseMetadata(detail.metadata_compiled);
    const title = asText(metadata.title) ?? detail.file_name;
    const tags = asTags(metadata.tags);
    const language = asText(metadata.language);
    const summary = asText(metadata.summary);
    const hasError = detail.last_error !== null && detail.last_error !== "";

    const sections = splitByChunkMarkers(detail.content_compiled ?? "");
    // 切片联动对齐：无标记导语段不参与匹配，其余段按标记出现顺序与 chunks 一一对应
    let labeledCounter = 0;
    const sectionMeta = sections.map((section) => {
        if (section.label === "") {
            return { section, labeledIndex: null as number | null };
        }
        const labeledIndex = labeledCounter;
        labeledCounter += 1;
        return { section, labeledIndex };
    });

    const scrollToChunk = (chunkId: number, index: number) => {
        setSelectedChunkId(chunkId);
        const target = sectionRefs.current.get(index);
        if (target) {
            target.scrollIntoView({ behavior: "smooth", block: "start" });
        }
    };

    const runAction = async (name: string, action: () => Promise<void>) => {
        setActionError(null);
        setBusy(name);
        try {
            await action();
            await refresh();
        } catch (err) {
            setActionError(err instanceof Error ? err.message : String(err));
        } finally {
            setBusy(null);
        }
    };

    const handleRecompile = async () => {
        // 强制重编译会覆盖现有编译稿并清除旧切片
        if (!(await confirmAction(`确定重新编译「${title}」吗？现有编译稿与切片将被覆盖。`))) {
            return;
        }
        void runAction("recompile", async () => {
            await compileDocuments([detail.id], true);
        });
    };

    const handleReindex = () => {
        void runAction("reindex", async () => {
            await indexDocuments([detail.id]);
        });
    };

    const handleRetry = () => {
        void runAction("retry", async () => {
            await compileDocuments([detail.id]);
            await indexDocuments([detail.id]);
        });
    };

    const handleReveal = () => {
        void runAction("reveal", async () => {
            await revealBlob(detail.blob_path);
        });
    };

    const handleCategoryChange = (value: string) => {
        void runAction("category", async () => {
            await setDocumentCategory(detail.id, value === "" ? null : value);
        });
    };

    return (
        <div className="flex h-full flex-col gap-4 overflow-y-auto p-8">
            <nav aria-label="面包屑" className="flex items-center gap-1 text-[13px]">
                <Link to="/documents" className="text-muted-foreground hover:text-foreground hover:underline">
                    文档库
                </Link>
                <span aria-hidden className="text-muted-foreground">
                    /
                </span>
                <span className="text-foreground">{title}</span>
            </nav>

            <Card className="gap-2.5 p-5">
                <div className="flex items-center justify-between gap-3">
                    <div className="flex min-w-0 items-center gap-2.5">
                        <h1 className="truncate text-xl font-semibold text-foreground">{title}</h1>
                        <Badge variant="secondary">{TYPE_LABELS[detail.type] ?? detail.type}</Badge>
                        <Badge variant={hasError ? "destructive" : "outline"}>
                            {STATUS_LABELS[detail.status] ?? detail.status}
                        </Badge>
                    </div>
                    <div className="flex shrink-0 items-center gap-2">
                        <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            disabled={busy !== null}
                            onClick={handleReveal}
                        >
                            {busy === "reveal" ? (
                                <Loader2 aria-hidden className="h-4 w-4 animate-spin" />
                            ) : (
                                <ExternalLink aria-hidden className="h-4 w-4" />
                            )}
                            打开原始文件
                        </Button>
                        <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            disabled={busy !== null}
                            onClick={handleRecompile}
                        >
                            {busy === "recompile" ? (
                                <Loader2 aria-hidden className="h-4 w-4 animate-spin" />
                            ) : (
                                <WandSparkles aria-hidden className="h-4 w-4" />
                            )}
                            重新编译
                        </Button>
                        <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            disabled={busy !== null}
                            onClick={handleReindex}
                        >
                            {busy === "reindex" ? (
                                <Loader2 aria-hidden className="h-4 w-4 animate-spin" />
                            ) : (
                                <Boxes aria-hidden className="h-4 w-4" />
                            )}
                            重新索引
                        </Button>
                    </div>
                </div>
                <p className="text-[13px] text-muted-foreground">
                    {formatBytes(detail.size)} · sha1 {shortSha1(detail.sha1)} · 导入于{" "}
                    {formatShortDate(detail.created_at)}
                    {detail.chunk_count !== null ? ` · ${detail.chunk_count} chunks` : ""}
                    {detail.embedding_model !== null ? ` · ${detail.embedding_model}` : ""}
                </p>
                <div className="flex items-center gap-2">
                    <span className="text-[13px] text-muted-foreground">分类</span>
                    <select
                        aria-label="所属分类"
                        value={detail.category_id ?? ""}
                        disabled={busy !== null}
                        onChange={(event) => handleCategoryChange(event.target.value)}
                        className="h-8 rounded-md border border-neutral-200 bg-neutral-50 px-2 text-[13px] text-neutral-950 outline-none transition-colors focus:border-neutral-400 disabled:opacity-50"
                    >
                        <option value="">未分类</option>
                        {categories.map((category) => (
                            <option key={category.id} value={category.id}>
                                {category.name}
                            </option>
                        ))}
                    </select>
                </div>
            </Card>

            {(actionError !== null || hasError) && (
                <div className="flex items-center gap-3">
                    <Alert className="flex-1">
                        <AlertIcon />
                        <div>
                            <AlertTitle>{actionError !== null ? "操作失败" : "处理失败"}</AlertTitle>
                            <AlertDescription>{actionError ?? detail.last_error}</AlertDescription>
                        </div>
                    </Alert>
                    {actionError === null && (
                        <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            disabled={busy !== null}
                            onClick={handleRetry}
                        >
                            {busy === "retry" ? (
                                <Loader2 aria-hidden className="h-4 w-4 animate-spin" />
                            ) : (
                                <RefreshCw aria-hidden className="h-4 w-4" />
                            )}
                            重试
                        </Button>
                    )}
                </div>
            )}

            <div className="flex min-h-0 flex-1 items-stretch gap-5">
                <Card className="min-w-0 flex-1 self-start">
                    <CardHeader className="flex-row items-center justify-between space-y-0 px-4 py-3">
                        <CardTitle className="text-base font-medium">正文</CardTitle>
                        <Tabs
                            value={viewMode}
                            onValueChange={(value) => setViewMode(value as ViewMode)}
                            items={[
                                { value: "rendered", label: "渲染视图" },
                                { value: "raw", label: "原始文本" },
                            ]}
                        />
                    </CardHeader>
                    <CardContent className="p-5 pt-0">
                        {viewMode === "rendered" ? (
                            detail.content_compiled === null ? (
                                <p className="text-[13px] text-muted-foreground">
                                    尚未编译，点击右上角「重新编译」生成编译稿。
                                </p>
                            ) : (
                                <div className="flex flex-col gap-3">
                                    {sectionMeta.map(({ section, labeledIndex }, sectionIndex) => {
                                        const isSelected =
                                            labeledIndex !== null && chunks[labeledIndex]?.id === selectedChunkId;
                                        return (
                                            <section
                                                key={
                                                    section.label === ""
                                                        ? `lead-${sectionIndex}`
                                                        : `chunk-${labeledIndex}`
                                                }
                                                data-chunk-label={section.label}
                                                ref={(node) => {
                                                    if (node === null || labeledIndex === null) {
                                                        return;
                                                    }
                                                    sectionRefs.current.set(labeledIndex, node);
                                                }}
                                                className={cn(
                                                    "rounded-md p-2 transition-colors",
                                                    isSelected ? "bg-accent" : "",
                                                )}
                                            >
                                                <ReactMarkdown
                                                    remarkPlugins={[remarkGfm]}
                                                    components={{
                                                        h1: ({ children }) => (
                                                            <h1 className="mb-2 text-lg font-semibold text-foreground">
                                                                {children}
                                                            </h1>
                                                        ),
                                                        h2: ({ children }) => (
                                                            <h2 className="mb-2 text-[15px] font-semibold text-foreground">
                                                                {children}
                                                            </h2>
                                                        ),
                                                        h3: ({ children }) => (
                                                            <h3 className="mb-2 text-sm font-semibold text-foreground">
                                                                {children}
                                                            </h3>
                                                        ),
                                                        p: ({ children }) => (
                                                            <p className="text-sm leading-[1.7] text-foreground">
                                                                {children}
                                                            </p>
                                                        ),
                                                        ul: ({ children }) => (
                                                            <ul className="list-disc pl-5 text-sm leading-[1.7] text-foreground">
                                                                {children}
                                                            </ul>
                                                        ),
                                                        ol: ({ children }) => (
                                                            <ol className="list-decimal pl-5 text-sm leading-[1.7] text-foreground">
                                                                {children}
                                                            </ol>
                                                        ),
                                                        code: ({ children, className }) =>
                                                            className?.includes("language-") ? (
                                                                <code className="block whitespace-pre-wrap rounded-md bg-muted p-3 text-[13px] leading-6">
                                                                    {children}
                                                                </code>
                                                            ) : (
                                                                <code className="rounded bg-muted px-1 py-0.5 text-[13px]">
                                                                    {children}
                                                                </code>
                                                            ),
                                                        pre: ({ children }) => (
                                                            <pre className="overflow-x-auto">{children}</pre>
                                                        ),
                                                        blockquote: ({ children }) => (
                                                            <blockquote className="border-l-2 border-border pl-3 text-sm text-muted-foreground">
                                                                {children}
                                                            </blockquote>
                                                        ),
                                                        a: ({ children, href }) => (
                                                            <a
                                                                href={href}
                                                                className="text-primary underline underline-offset-2"
                                                            >
                                                                {children}
                                                            </a>
                                                        ),
                                                    }}
                                                >
                                                    {section.text}
                                                </ReactMarkdown>
                                            </section>
                                        );
                                    })}
                                </div>
                            )
                        ) : (
                            <pre className="max-h-[60vh] overflow-auto whitespace-pre-wrap text-[13px] leading-6 text-foreground">
                                {detail.content}
                            </pre>
                        )}
                    </CardContent>
                </Card>

                <div className="flex w-[340px] shrink-0 flex-col gap-5">
                    <Card>
                        <CardHeader className="px-4 py-3">
                            <CardTitle className="text-base font-medium">元数据</CardTitle>
                        </CardHeader>
                        <CardContent className="flex flex-col gap-3 px-4 pb-4">
                            <div className="flex flex-col gap-1.5">
                                <span className="text-xs text-muted-foreground">标签</span>
                                <div className="flex flex-wrap gap-1.5">
                                    {tags.length > 0 ? (
                                        tags.map((tag) => (
                                            <Badge key={tag} variant="secondary">
                                                {tag}
                                            </Badge>
                                        ))
                                    ) : (
                                        <span className="text-[13px] text-muted-foreground">—</span>
                                    )}
                                </div>
                            </div>
                            <div className="flex flex-col gap-1.5">
                                <span className="text-xs text-muted-foreground">语言</span>
                                <span className="text-sm text-foreground">{language ?? "—"}</span>
                            </div>
                            <div className="flex flex-col gap-1.5">
                                <span className="text-xs text-muted-foreground">摘要</span>
                                <span className="text-[13px] leading-6 text-foreground">{summary ?? "—"}</span>
                            </div>
                        </CardContent>
                    </Card>

                    <Card>
                        <CardHeader className="px-4 py-3">
                            <CardTitle className="text-base font-medium">切片（{chunks.length}）</CardTitle>
                        </CardHeader>
                        <CardContent className="flex flex-col gap-1 p-2 pt-0">
                            {chunks.length === 0 ? (
                                <p className="px-3 py-2 text-[13px] text-muted-foreground">
                                    {detail.status === "raw"
                                        ? "尚未编译，编译后可索引生成切片。"
                                        : "尚未索引，点击「重新索引」生成切片。"}
                                </p>
                            ) : (
                                chunks.map((chunk, index) => (
                                    <button
                                        key={chunk.id}
                                        type="button"
                                        onClick={() => scrollToChunk(chunk.id, index)}
                                        className={cn(
                                            "flex w-full items-center justify-between rounded-md px-3 py-2 text-left text-[13px] transition-colors",
                                            selectedChunkId === chunk.id
                                                ? "bg-accent text-accent-foreground"
                                                : "text-foreground hover:bg-accent/50",
                                        )}
                                    >
                                        <span className="truncate">
                                            {String(index + 1).padStart(2, "0")} · {chunk.label || "无标签"}
                                        </span>
                                        <span className="ml-3 shrink-0 text-xs text-muted-foreground">
                                            {charCount(chunk.text)}
                                        </span>
                                    </button>
                                ))
                            )}
                        </CardContent>
                    </Card>
                </div>
            </div>
        </div>
    );
}
