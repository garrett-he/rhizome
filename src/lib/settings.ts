import { invoke } from "@tauri-apps/api/core";
import type { SettingEntry } from "@/types/settings";

export async function getSettings(): Promise<SettingEntry[]> {
    return invoke<SettingEntry[]>("get_settings");
}

/** value 为 null 时删除该键 */
export async function setSetting(key: string, value: string | null): Promise<void> {
    return invoke("set_setting", { key, value });
}

export async function getDefaultPrompt(name: string): Promise<string> {
    return invoke<string>("get_default_prompt", { name });
}
