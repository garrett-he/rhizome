export type TaskStep = "compile" | "index" | "all";

export interface TaskSummary {
    job_id: string;
    kind: string;
    total: number;
    succeeded: number;
    failed: number;
}

export interface JobRecord {
    id: string;
    kind: string;
    total: number;
    succeeded: number;
    failed: number;
    status: string;
    started_at: string;
    finished_at: string | null;
}
