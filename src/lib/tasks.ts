import { invoke } from "@tauri-apps/api/core";
import type { JobRecord, TaskStep, TaskSummary } from "@/types/task";

export function runTasks(documentIds: string[], step: TaskStep): Promise<TaskSummary> {
    return invoke<TaskSummary>("run_tasks", { documentIds, step });
}

export function listJobs(): Promise<JobRecord[]> {
    return invoke<JobRecord[]>("list_jobs");
}
