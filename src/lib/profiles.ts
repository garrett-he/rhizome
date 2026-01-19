import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { Profiles, ProfilesResponse } from "@/types/profile";

export async function getProfiles(): Promise<ProfilesResponse> {
    return invoke<ProfilesResponse>("get_profiles");
}

export async function createVault(path: string): Promise<Profiles> {
    return invoke<Profiles>("create_vault", { path });
}

export async function openVault(path: string): Promise<Profiles> {
    return invoke<Profiles>("open_vault", { path });
}

export async function pickDirectory(): Promise<string | null> {
    const selected = await open({ directory: true, multiple: false });
    return typeof selected === "string" ? selected : null;
}
