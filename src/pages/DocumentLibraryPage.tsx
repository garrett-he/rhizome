import { Loader2, RefreshCw, RotateCcw, Upload } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { useCategories } from "@/hooks/useCategories";
import { useDocuments } from "@/hooks/useDocuments";
import { formatBytes, formatShortDate } from "@/lib/format";

const STATUS_LABELS: Record<string, string> = {
    raw: "已导入",
    compiled: "已编译",
    indexed: "已索引",
};

const TYPE_LABELS: Record<string, string> = {
    plaintext: "文本",
};

function StatusBadge({ status, hasError }: { status: string; hasError: boolean }) {
    if (hasError) {
        return (
            <span className="inline-flex items-center rounded-2xl bg-red-100 px-2 py-0.5 text-xs font-medium text-red-700">
                {STATUS_LABELS[status] ?? status}
            </span>
        );
    }
    return (
        <span className="inline-flex items-center rounded-2xl bg-neutral-100 px-2 py-0.5 text-xs font-medium text-neutral-900">
            {STATUS_LABELS[status] ?? status}
        </span>
    );
}

export function DocumentLibraryPage() {
    const {
        documents,
        loading,
        processing,
        error,
        summary,
        processNote,
        importFiles,
        processAll,
        processDocument,
        refresh,
    } = useDocuments();
    const { categories } = useCategories();
    const [categoryFilter, setCategoryFilter] = useState("");
    const [importCategory, setImportCategory] = useState("");

    const filtered = categoryFilter === "" ? documents : documents.filter((doc) => doc.category_id === categoryFilter);

    return (
        <div className="flex flex-col gap-5 p-8">
            <div className="flex items-center justify-between">
                <h1 className="text-2xl font-semibold text-neutral-950">文档库</h1>
                <div className="flex gap-3">
                    <select
                        aria-label="分类筛选"
                        value={categoryFilter}
                        onChange={(event) => setCategoryFilter(event.target.value)}
                        className="h-9 rounded-md border border-neutral-200 bg-neutral-50 px-2 text-sm text-neutral-950 outline-none transition-colors focus:border-neutral-400"
                    >
                        <option value="">全部分类</option>
                        {categories.map((category) => (
                            <option key={category.id} value={category.id}>
                                {category.name}
                            </option>
                        ))}
                    </select>
                    <Button type="button" variant="outline" disabled={loading} onClick={() => void refresh()}>
                        <RefreshCw aria-hidden className="h-4 w-4" />
                        刷新
                    </Button>
                    <Button
                        type="button"
                        variant="outline"
                        disabled={loading || processing || documents.length === 0}
                        onClick={() => void processAll()}
                    >
                        {processing ? (
                            <Loader2 aria-hidden className="h-4 w-4 animate-spin" />
                        ) : (
                            <RotateCcw aria-hidden className="h-4 w-4" />
                        )}
                        编译并索引
                    </Button>
                    <select
                        aria-label="导入分类"
                        value={importCategory}
                        onChange={(event) => setImportCategory(event.target.value)}
                        disabled={loading || processing}
                        className="h-9 rounded-md border border-neutral-200 bg-neutral-50 px-2 text-sm text-neutral-950 outline-none transition-colors focus:border-neutral-400"
                    >
                        <option value="">导入时不设分类</option>
                        {categories.map((category) => (
                            <option key={category.id} value={category.name}>
                                导入到「{category.name}」
                            </option>
                        ))}
                    </select>
                    <Button
                        type="button"
                        disabled={loading || processing}
                        onClick={() => void importFiles(importCategory === "" ? undefined : importCategory)}
                    >
                        <Upload aria-hidden className="h-4 w-4" />
                        导入文档
                    </Button>
                </div>
            </div>

            {error && (
                <p role="alert" className="text-[13px] text-red-600">
                    {error}
                </p>
            )}

            {processNote && !error && <p className="text-[13px] text-neutral-500">{processNote}</p>}

            {summary && (
                <p className="text-[13px] text-neutral-500">
                    新增 {summary.added} · 跳过 {summary.skipped} · 失败 {summary.failures.length}
                    {summary.failures.length > 0 && (
                        <span className="ml-2 text-red-600">
                            {summary.failures.map((f) => `${f.path}：${f.error}`).join("；")}
                        </span>
                    )}
                </p>
            )}

            {documents.length === 0 ? (
                <div className="flex items-center justify-center rounded-lg border border-neutral-200 bg-neutral-50 py-16 text-[13px] text-neutral-500">
                    {loading ? "加载中…" : "暂无文档，点击右上角导入文档添加"}
                </div>
            ) : filtered.length === 0 ? (
                <div className="flex items-center justify-center rounded-lg border border-neutral-200 bg-neutral-50 py-16 text-[13px] text-neutral-500">
                    该分类下暂无文档
                </div>
            ) : (
                <table className="w-full border-collapse overflow-hidden rounded-lg border border-neutral-200 bg-neutral-50 text-left shadow-sm">
                    <thead>
                        <tr className="bg-neutral-100 text-[13px] font-normal text-neutral-500">
                            <th className="px-4 py-2.5 font-normal">标题</th>
                            <th className="w-20 px-4 py-2.5 font-normal">类型</th>
                            <th className="w-28 px-4 py-2.5 font-normal">分类</th>
                            <th className="w-24 px-4 py-2.5 font-normal">状态</th>
                            <th className="w-24 px-4 py-2.5 font-normal">大小</th>
                            <th className="w-28 px-4 py-2.5 font-normal">导入时间</th>
                            <th className="w-24 px-4 py-2.5 font-normal">操作</th>
                        </tr>
                    </thead>
                    <tbody>
                        {filtered.map((doc) => {
                            const hasError = doc.last_error !== null && doc.last_error !== "";
                            const needsAction = doc.status !== "indexed" || hasError;
                            return (
                                <tr
                                    key={doc.id}
                                    className="border-t border-neutral-200 transition-colors hover:bg-neutral-100/60"
                                >
                                    <td className="px-4 py-2.5">
                                        <Link
                                            to={`/documents/${doc.id}`}
                                            className="text-sm font-medium text-neutral-950 hover:underline"
                                        >
                                            {doc.file_name}
                                        </Link>
                                        {hasError && (
                                            <span title={doc.last_error ?? ""} className="ml-2 text-xs text-red-600">
                                                {doc.last_error}
                                            </span>
                                        )}
                                    </td>
                                    <td className="px-4 py-2.5 text-[13px] text-neutral-500">
                                        {TYPE_LABELS[doc.type] ?? doc.type}
                                    </td>
                                    <td className="px-4 py-2.5">
                                        {doc.category_name !== null ? (
                                            <span className="inline-flex items-center rounded-2xl bg-neutral-900 px-2 py-0.5 text-xs font-medium text-neutral-50">
                                                {doc.category_name}
                                            </span>
                                        ) : (
                                            <span className="text-[13px] text-neutral-400">未分类</span>
                                        )}
                                    </td>
                                    <td className="px-4 py-2.5">
                                        <StatusBadge status={doc.status} hasError={hasError} />
                                    </td>
                                    <td className="px-4 py-2.5 text-[13px] text-neutral-500">
                                        {formatBytes(doc.size)}
                                    </td>
                                    <td className="px-4 py-2.5 text-[13px] text-neutral-500">
                                        {formatShortDate(doc.created_at)}
                                    </td>
                                    <td className="px-4 py-2.5">
                                        {needsAction && (
                                            <Button
                                                type="button"
                                                variant="ghost"
                                                size="sm"
                                                disabled={processing}
                                                onClick={() => void processDocument(doc.id)}
                                            >
                                                <RotateCcw aria-hidden className="h-3.5 w-3.5" />
                                                重试
                                            </Button>
                                        )}
                                    </td>
                                </tr>
                            );
                        })}
                    </tbody>
                </table>
            )}
        </div>
    );
}
