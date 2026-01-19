import { RefreshCw, Upload } from "lucide-react";
import { Button } from "@/components/ui/button";
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

function StatusBadge({ status }: { status: string }) {
    return (
        <span className="inline-flex items-center rounded-2xl bg-neutral-100 px-2 py-0.5 text-xs font-medium text-neutral-900">
            {STATUS_LABELS[status] ?? status}
        </span>
    );
}

export function DocumentLibraryPage() {
    const { documents, loading, error, summary, importFiles, refresh } = useDocuments();

    return (
        <div className="flex flex-col gap-5 p-8">
            <div className="flex items-center justify-between">
                <h1 className="text-2xl font-semibold text-neutral-950">文档库</h1>
                <div className="flex gap-3">
                    <Button type="button" variant="outline" disabled={loading} onClick={() => void refresh()}>
                        <RefreshCw aria-hidden className="h-4 w-4" />
                        刷新
                    </Button>
                    <Button type="button" disabled={loading} onClick={() => void importFiles()}>
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
            ) : (
                <table className="w-full border-collapse overflow-hidden rounded-lg border border-neutral-200 bg-neutral-50 text-left shadow-sm">
                    <thead>
                        <tr className="bg-neutral-100 text-[13px] font-normal text-neutral-500">
                            <th className="px-4 py-2.5 font-normal">标题</th>
                            <th className="w-20 px-4 py-2.5 font-normal">类型</th>
                            <th className="w-24 px-4 py-2.5 font-normal">状态</th>
                            <th className="w-24 px-4 py-2.5 font-normal">大小</th>
                            <th className="w-28 px-4 py-2.5 font-normal">导入时间</th>
                        </tr>
                    </thead>
                    <tbody>
                        {documents.map((doc) => (
                            <tr
                                key={doc.id}
                                className="border-t border-neutral-200 transition-colors hover:bg-neutral-100/60"
                            >
                                <td className="px-4 py-2.5 text-sm font-medium text-neutral-950">{doc.file_name}</td>
                                <td className="px-4 py-2.5 text-[13px] text-neutral-500">
                                    {TYPE_LABELS[doc.type] ?? doc.type}
                                </td>
                                <td className="px-4 py-2.5">
                                    <StatusBadge status={doc.status} />
                                </td>
                                <td className="px-4 py-2.5 text-[13px] text-neutral-500">{formatBytes(doc.size)}</td>
                                <td className="px-4 py-2.5 text-[13px] text-neutral-500">
                                    {formatShortDate(doc.created_at)}
                                </td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            )}
        </div>
    );
}
