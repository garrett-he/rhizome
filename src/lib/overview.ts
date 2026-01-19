import { invoke } from "@tauri-apps/api/core";
import type { Overview } from "@/types/overview";

export async function getOverview(): Promise<Overview> {
    return invoke<Overview>("get_overview");
}
