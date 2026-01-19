export interface Vault {
    path: string;
    added_at: string;
    last_opened_at: string;
}

export interface Profiles {
    current: string | null;
    vaults: Vault[];
}

export interface ProfilesResponse {
    home_dir: string;
    profiles: Profiles;
}
