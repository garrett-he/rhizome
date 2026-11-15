import { FileText, Loader2, Search } from "lucide-react";
import { type FormEvent, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Tabs } from "@/components/ui/tabs";
import { searchDocuments } from "@/lib/documents";
import { formatShortDate } from "@/lib/format";
import type { SearchHit } from "@/types/document";

function toMessage(err: unknown): string {
    if (typeof err === "string") {
        return err;
    }
    if (err instanceof Error) {
        return err.message;
    }
    return String(err);
}

function formatRelevance(relevance: number): string {
    return relevance.toFixed(2);
}

/** Meta 行首段：首个标签，无标签回退「文档」 */
function primaryTag(tags: string[]): string {
    return tags[0] ?? "文档";
}

function padIndex(index: number): string {
    return String(index).padStart(2, "0");
}

export function SearchPage() {
    const navigate = useNavigate();
    const [query, setQuery] = useState("");
    const [searched, setSearched] = useState(false);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [hits, setHits] = useState<SearchHit[]>([]);
    const [vectorUsed, setVectorUsed] = useState(true);

    const runSearch = async () => {
        const trimmed = query.trim();
        if (trimmed === "") {
            setError("请输入搜索关键词");
            return;
        }
        setError(null);
        setLoading(true);
        setSearched(true);
        try {
            const outcome = await searchDocuments(trimmed);
            setHits(outcome.hits);
            setVectorUsed(outcome.vector_used);
        } catch (err) {
            setError(toMessage(err));
        } finally {
            setLoading(false);
        }
    };

    const handleSubmit = (event: FormEvent) => {
        event.preventDefault();
        void runSearch();
    };

    const openHit = (hit: SearchHit) => {
        navigate(`/documents/${hit.document_id}?chunk=${hit.chunk_id}`);
    };

    return (
        <div className="flex flex-col gap-5 px-[280px] py-10">
            <form
                onSubmit={handleSubmit}
                className="flex items-center gap-2 rounded-[10px] border border-neutral-200 bg-neutral-50 py-1.5 pl-4 shadow-sm"
            >
                <Search aria-hidden className="h-[18px] w-[18px] shrink-0 text-neutral-500" />
                <input
                    type="search"
                    value={query}
                    onChange={(event) => setQuery(event.target.value)}
                    placeholder="搜索知识库…"
                    aria-label="搜索关键词"
                    className="h-8 min-w-0 flex-1 bg-transparent text-[15px] text-neutral-950 outline-none placeholder:text-neutral-400"
                />
                <Button type="submit" disabled={loading}>
                    {loading && <Loader2 aria-hidden className="h-4 w-4 animate-spin" />}
                    搜索
                </Button>
            </form>

            <Tabs value="all" onValueChange={() => {}} items={[{ value: "all", label: "全部" }]} />

            {error && (
                <p role="alert" className="text-[13px] text-red-600">
                    {error}
                </p>
            )}

            {!searched && !error && (
                <p className="py-16 text-center text-[13px] text-neutral-500">输入关键词搜索知识库</p>
            )}

            {searched && !error && !loading && hits.length === 0 && (
                <p className="py-16 text-center text-[13px] text-neutral-500">未找到相关内容</p>
            )}

            {searched && !error && !loading && hits.length > 0 && !vectorUsed && (
                <div
                    role="status"
                    className="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-[13px] leading-5 text-amber-800"
                >
                    本次仅使用全文检索：向量检索不可用，结果可能不完整。
                </div>
            )}

            <div className="flex flex-col gap-3">
                {hits.map((hit) => (
                    <Card key={hit.chunk_id} className="gap-0 p-0">
                        <div className="flex items-center gap-2.5 border-b border-neutral-200 px-5 py-3.5">
                            <FileText aria-hidden className="h-4 w-4 shrink-0 text-neutral-500" />
                            <span className="min-w-0 flex-1 truncate text-[15px] font-medium text-neutral-950">
                                {hit.document_title}
                            </span>
                            <span className="shrink-0 text-xs text-neutral-500">
                                {padIndex(hit.chunk_index)} · {hit.chunk_label || "无标签"}
                            </span>
                            <Badge variant="secondary" className="shrink-0">
                                相关度 {formatRelevance(hit.relevance)}
                            </Badge>
                        </div>
                        <div className="flex flex-col gap-2 px-5 pb-4 pt-1">
                            <p className="text-sm leading-[1.7] text-neutral-950">{hit.snippet}</p>
                            <div className="flex items-center justify-between">
                                <span className="text-xs text-neutral-500">
                                    {primaryTag(hit.tags)} · {formatShortDate(hit.created_at)} · chunk{" "}
                                    {padIndex(hit.chunk_index)}
                                </span>
                                <Button type="button" variant="ghost" size="sm" onClick={() => openHit(hit)}>
                                    展开全文 →
                                </Button>
                            </div>
                        </div>
                    </Card>
                ))}
            </div>

            {!loading && hits.length > 0 && (
                <p className="text-center text-xs text-neutral-400">
                    共 {hits.length} 条结果 ·{" "}
                    <Link to="/documents" className="hover:underline">
                        返回文档库
                    </Link>
                </p>
            )}
        </div>
    );
}
