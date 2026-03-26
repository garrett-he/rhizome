import { useCallback, useEffect, useState } from "react";
import { getOverview } from "@/lib/overview";
import type { Overview } from "@/types/overview";

function toMessage(err: unknown): string {
    if (typeof err === "string") {
        return err;
    }
    if (err instanceof Error) {
        return err.message;
    }
    return String(err);
}

export function useOverview() {
    const [overview, setOverview] = useState<Overview | null>(null);
    const [error, setError] = useState<string | null>(null);

    const refresh = useCallback(async () => {
        try {
            setOverview(await getOverview());
            setError(null);
        } catch (err) {
            setError(toMessage(err));
        }
    }, []);

    useEffect(() => {
        void refresh();
    }, [refresh]);

    return { overview, error };
}
