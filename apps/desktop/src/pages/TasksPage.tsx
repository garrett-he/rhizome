import { CheckCircle2, Loader2, Play, RefreshCw, XCircle } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { type TaskProgress, useTasks } from "@/hooks/useTasks";
import { formatShortDate } from "@/lib/format";
import type { DocumentSummary } from "@/types/document";
import type { TaskStep } from "@/types/task";

const KIND_LABELS: Record<string, string> = {
    compile: "编译",
    index: "索引",
    all: "编译并索引",
};

const STEP_LABELS: Record<string, string> = {
    compile: "编译",
    index: "索引",
};

function hasError(doc: DocumentSummary): boolean {
    return doc.last_error !== null && doc.last_error !== "";
}

interface TaskGroupProps {
    title: string;
    docs: DocumentSummary[];
    actionLabel: string;
    step: TaskStep;
    selected: Set<string>;
    running: boolean;
    onToggle: (id: string) => void;
    onRun: (ids: string[], step: TaskStep) => void;
}

function TaskGroup({ title, docs, actionLabel, step, selected, running, onToggle, onRun }: TaskGroupProps) {
    const selectedIds = docs.filter((doc) => selected.has(doc.id)).map((doc) => doc.id);
    return (
        <div className="flex flex-col rounded-lg border border-neutral-200 bg-neutral-50 shadow-sm">
            <div className="flex items-center justify-between px-5 pt-4 pb-2">
                <h3 className="flex items-center gap-2 text-base font-medium text-neutral-950">
                    {title}
                    <span className="rounded-2xl bg-neutral-100 px-2 py-0.5 text-xs font-medium text-neutral-900">
                        {docs.length}
                    </span>
                </h3>
                <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={running || selectedIds.length === 0}
                    onClick={() => onRun(selectedIds, step)}
                >
                    <Play aria-hidden className="h-3.5 w-3.5" />
                    {actionLabel}
                </Button>
            </div>
            <div className="flex flex-col gap-1 p-2.5">
                {docs.length === 0 ? (
                    <div className="flex items-center justify-center rounded-md p-4 text-[13px] text-neutral-500">
                        暂无待处理文档
                    </div>
                ) : (
                    docs.map((doc) => (
                        <label
                            key={doc.id}
                            className="flex cursor-pointer items-center gap-2.5 rounded-md px-3 py-2 transition-colors hover:bg-neutral-100"
                        >
                            <input
                                type="checkbox"
                                checked={selected.has(doc.id)}
                                disabled={running}
                                onChange={() => onToggle(doc.id)}
                                className="h-3.5 w-3.5 accent-neutral-900"
                            />
                            <Link
                                to={`/documents/${doc.id}`}
                                className="min-w-0 flex-1 truncate text-sm text-neutral-950 hover:underline"
                            >
                                {doc.file_name}
                            </Link>
                            {hasError(doc) && (
                                <span
                                    title={doc.last_error ?? ""}
                                    className="max-w-[320px] truncate text-xs text-red-600"
                                >
                                    {doc.last_error}
                                </span>
                            )}
                        </label>
                    ))
                )}
            </div>
        </div>
    );
}

function ProgressCard({ progress, documents }: { progress: TaskProgress; documents: DocumentSummary[] }) {
    const titleOf = (id: string) => documents.find((doc) => doc.id === id)?.file_name ?? id.slice(0, 8);
    const finished = progress.succeeded !== null;
    return (
        <div className="flex flex-col gap-2 rounded-lg border border-neutral-200 bg-neutral-50 p-4 shadow-sm">
            <div className="flex items-center gap-2 text-sm font-medium text-neutral-950">
                {finished ? null : <Loader2 aria-hidden className="h-4 w-4 animate-spin" />}
                {KIND_LABELS[progress.kind] ?? progress.kind} · {progress.items.length}/{progress.total}
                {finished && (
                    <span className="ml-2 text-[13px] font-normal text-neutral-500">
                        成功 {progress.succeeded} · 失败 {progress.failed}
                    </span>
                )}
            </div>
            <div className="flex flex-col gap-1">
                {progress.items.map((item) => (
                    <div key={`${item.documentId}-${item.step}`} className="flex items-center gap-2 text-[13px]">
                        {item.ok ? (
                            <CheckCircle2 aria-hidden className="h-3.5 w-3.5 shrink-0 text-green-600" />
                        ) : (
                            <XCircle aria-hidden className="h-3.5 w-3.5 shrink-0 text-red-600" />
                        )}
                        <span className="min-w-0 flex-1 truncate text-neutral-950">{titleOf(item.documentId)}</span>
                        <span className="shrink-0 text-neutral-500">{STEP_LABELS[item.step] ?? item.step}</span>
                        {!item.ok && item.error !== null && (
                            <span className="max-w-[320px] truncate text-red-600">{item.error}</span>
                        )}
                    </div>
                ))}
            </div>
        </div>
    );
}

export function TasksPage() {
    const { documents, jobs, loading, error, running, progress, run, refresh } = useTasks();
    const [selected, setSelected] = useState<Set<string>>(new Set());

    const pendingCompile = documents.filter((doc) => doc.status === "raw" && !hasError(doc));
    const pendingIndex = documents.filter((doc) => doc.status === "compiled" && !hasError(doc));
    const failedDocs = documents.filter((doc) => hasError(doc));
    const hasTargets = pendingCompile.length + pendingIndex.length + failedDocs.length > 0;

    const toggle = (id: string) => {
        setSelected((previous) => {
            const next = new Set(previous);
            if (next.has(id)) {
                next.delete(id);
            } else {
                next.add(id);
            }
            return next;
        });
    };

    const runStep = (ids: string[], step: TaskStep) => {
        if (ids.length === 0) {
            return;
        }
        setSelected(new Set());
        void run(ids, step);
    };

    return (
        <div className="flex flex-col gap-5 p-8">
            <div className="flex items-center justify-end">
                <div className="flex gap-3">
                    <Button
                        type="button"
                        variant="outline"
                        size="icon"
                        aria-label="刷新"
                        title="刷新"
                        disabled={loading || running}
                        onClick={() => void refresh()}
                    >
                        <RefreshCw aria-hidden className="h-4 w-4" />
                    </Button>
                    <Button
                        type="button"
                        disabled={loading || running || !hasTargets}
                        onClick={() => void run([], "all")}
                    >
                        {running ? (
                            <Loader2 aria-hidden className="h-4 w-4 animate-spin" />
                        ) : (
                            <Play aria-hidden className="h-4 w-4" />
                        )}
                        一键处理全部
                    </Button>
                </div>
            </div>

            {error && (
                <p role="alert" className="text-[13px] text-red-600">
                    {error}
                </p>
            )}

            {progress && <ProgressCard progress={progress} documents={documents} />}

            <div className="grid grid-cols-3 gap-5">
                <TaskGroup
                    title="待编译"
                    docs={pendingCompile}
                    actionLabel="编译所选"
                    step="compile"
                    selected={selected}
                    running={running}
                    onToggle={toggle}
                    onRun={runStep}
                />
                <TaskGroup
                    title="待索引"
                    docs={pendingIndex}
                    actionLabel="索引所选"
                    step="index"
                    selected={selected}
                    running={running}
                    onToggle={toggle}
                    onRun={runStep}
                />
                <TaskGroup
                    title="失败"
                    docs={failedDocs}
                    actionLabel="重试所选"
                    step="all"
                    selected={selected}
                    running={running}
                    onToggle={toggle}
                    onRun={runStep}
                />
            </div>

            <div className="flex flex-col rounded-lg border border-neutral-200 bg-neutral-50 shadow-sm">
                <div className="px-5 pt-4 pb-2">
                    <h3 className="text-base font-medium text-neutral-950">历史任务</h3>
                </div>
                <div className="flex flex-col gap-1 p-2.5">
                    {jobs.length === 0 ? (
                        <div className="flex items-center justify-center rounded-md p-4 text-[13px] text-neutral-500">
                            暂无历史任务
                        </div>
                    ) : (
                        jobs.map((job) => (
                            <div key={job.id} className="flex items-center gap-3 rounded-md px-3 py-2 text-[13px]">
                                <span className="w-16 shrink-0 text-neutral-500">
                                    {formatShortDate(job.started_at)}
                                </span>
                                <span className="w-20 shrink-0 text-neutral-950">
                                    {KIND_LABELS[job.kind] ?? job.kind}
                                </span>
                                <span className="flex-1 text-neutral-500">
                                    共 {job.total} 篇 · 成功 {job.succeeded} · 失败 {job.failed}
                                </span>
                                {job.status !== "completed" && (
                                    <span className="shrink-0 rounded-2xl bg-neutral-100 px-2 py-0.5 text-xs text-neutral-900">
                                        进行中
                                    </span>
                                )}
                            </div>
                        ))
                    )}
                </div>
            </div>
        </div>
    );
}
