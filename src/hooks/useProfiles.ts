import { useCallback, useEffect, useState } from "react";
import {
    createVault as createVaultCmd,
    getProfiles,
    openVault as openVaultCmd,
    pickDirectory as pickDirectoryCmd,
} from "@/lib/profiles";
import type { Profiles } from "@/types/profile";

function toMessage(err: unknown): string {
    if (typeof err === "string") {
        return err;
    }
    if (err instanceof Error) {
        return err.message;
    }
    return String(err);
}

export function useProfiles() {
    const [homeDir, setHomeDir] = useState("");
    const [profiles, setProfiles] = useState<Profiles | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    const refresh = useCallback(async () => {
        try {
            const response = await getProfiles();
            setHomeDir(response.home_dir);
            setProfiles(response.profiles);
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

    const createVault = useCallback(async (path: string): Promise<boolean> => {
        setError(null);
        try {
            setProfiles(await createVaultCmd(path));
            return true;
        } catch (err) {
            setError(toMessage(err));
            return false;
        }
    }, []);

    const openVault = useCallback(async (path: string): Promise<boolean> => {
        setError(null);
        try {
            setProfiles(await openVaultCmd(path));
            return true;
        } catch (err) {
            setError(toMessage(err));
            return false;
        }
    }, []);

    const pickDirectory = useCallback(async (): Promise<string | null> => {
        setError(null);
        try {
            return await pickDirectoryCmd();
        } catch (err) {
            setError(toMessage(err));
            return null;
        }
    }, []);

    return {
        homeDir,
        profiles,
        loading,
        error,
        refresh,
        createVault,
        openVault,
        pickDirectory,
    };
}
