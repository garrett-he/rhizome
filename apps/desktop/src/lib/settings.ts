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

/** 用当前表单值测试 LLM 连通性，成功 resolve，失败 reject 带错误信息。 */
export async function testLlmConnection(baseUrl: string, apiKey: string, model: string): Promise<void> {
    await invoke("test_llm_connection", { baseUrl, apiKey, model });
}
