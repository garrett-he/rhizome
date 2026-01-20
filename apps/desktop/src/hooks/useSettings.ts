import { useCallback, useEffect, useState } from "react";
import { getSettings, setSetting as setSettingCmd } from "@/lib/settings";

function toMessage(err: unknown): string {
    if (typeof err === "string") {
        return err;
    }
    if (err instanceof Error) {
        return err.message;
    }
    return String(err);
}

export function useSettings() {
    const [settings, setSettings] = useState<Record<string, string | null>>({});
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    const refresh = useCallback(async () => {
        try {
            const entries = await getSettings();
            const record: Record<string, string | null> = {};
            for (const entry of entries) {
                record[entry.key] = entry.value;
            }
            setSettings(record);
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

    const save = useCallback(async (key: string, value: string | null): Promise<boolean> => {
        setError(null);
        try {
            await setSettingCmd(key, value);
            setSettings((prev) => ({ ...prev, [key]: value }));
            return true;
        } catch (err) {
            setError(toMessage(err));
            return false;
        }
    }, []);

    return { settings, loading, error, save, refresh };
}
