import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import { listDocuments } from "@/lib/documents";
import { listJobs, runTasks } from "@/lib/tasks";
import type { DocumentSummary } from "@/types/document";
import type { JobRecord, TaskStep } from "@/types/task";

function toMessage(err: unknown): string {
    if (typeof err === "string") {
        return err;
    }
    if (err instanceof Error) {
        return err.message;
    }
    return String(err);
}

/** 单个文档单步执行结果（逐项进度） */
export interface TaskItemState {
    documentId: string;
    step: string;
    ok: boolean;
    error: string | null;
}

/** 进行中批次的进度（成败计数以 task-done 为准：all 步骤单文档产生两个 item） */
export interface TaskProgress {
    jobId: string;
    kind: string;
    total: number;
    items: TaskItemState[];
    succeeded: number | null;
    failed: number | null;
}

interface TaskEventPayload {
    job_id: string;
    kind?: string;
    total?: number;
    document_id?: string;
    step?: string;
    ok?: boolean;
    error?: string | null;
    succeeded?: number;
    failed?: number;
}

export function useTasks() {
    const [documents, setDocuments] = useState<DocumentSummary[]>([]);
    const [jobs, setJobs] = useState<JobRecord[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [running, setRunning] = useState(false);
    const [progress, setProgress] = useState<TaskProgress | null>(null);

    const runningRef = useRef(false);

    const refresh = useCallback(async () => {
        try {
            const [docs, jobList] = await Promise.all([listDocuments(), listJobs()]);
            setDocuments(docs);
            setJobs(jobList);
            setError(null);
        } catch (err) {
            setError(toMessage(err));
        } finally {
            setLoading(false);
        }
    }, []);

    useEffect(() => {
        void refresh();
    }, [refresh]);

    useEffect(() => {
        // 先建立监听再执行任务（run_tasks 在批次结束后才 resolve，事件先行到达）
        const unlisteners: Array<() => void> = [];
        const bind = async () => {
            unlisteners.push(
                await listen<TaskEventPayload>("task-start", (event) => {
                    setProgress({
                        jobId: event.payload.job_id,
                        kind: event.payload.kind ?? "",
                        total: event.payload.total ?? 0,
                        items: [],
                        succeeded: null,
                        failed: null,
                    });
                }),
                await listen<TaskEventPayload>("task-item", (event) => {
                    setProgress((previous) => {
                        if (previous === null || previous.jobId !== event.payload.job_id) {
                            return previous;
                        }
                        return {
                            ...previous,
                            items: [
                                ...previous.items,
                                {
                                    documentId: event.payload.document_id ?? "",
                                    step: event.payload.step ?? "",
                                    ok: event.payload.ok ?? false,
                                    error: event.payload.error ?? null,
                                },
                            ],
                        };
                    });
                }),
                await listen<TaskEventPayload>("task-done", (event) => {
                    setProgress((previous) => {
                        if (previous === null || previous.jobId !== event.payload.job_id) {
                            return previous;
                        }
                        return {
                            ...previous,
                            succeeded: event.payload.succeeded ?? 0,
                            failed: event.payload.failed ?? 0,
                        };
                    });
                }),
            );
        };
        void bind();
        return () => {
            unlisteners.forEach((unbind) => {
                unbind();
            });
        };
    }, []);

    const run = useCallback(
        async (documentIds: string[], step: TaskStep) => {
            if (runningRef.current) {
                return;
            }
            runningRef.current = true;
            setRunning(true);
            setError(null);
            setProgress(null);
            try {
                await runTasks(documentIds, step);
                await refresh();
            } catch (err) {
                setError(toMessage(err));
            } finally {
                runningRef.current = false;
                setRunning(false);
            }
        },
        [refresh],
    );

    return { documents, jobs, loading, error, running, progress, run, refresh };
}
