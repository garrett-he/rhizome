import { useCallback, useEffect, useState } from "react";
import { addDocuments as addDocumentsCmd, listDocuments, pickFiles } from "@/lib/documents";
import type { AddSummary, DocumentSummary } from "@/types/document";

function toMessage(err: unknown): string {
    if (typeof err === "string") {
        return err;
    }
    if (err instanceof Error) {
        return err.message;
    }
    return String(err);
}

export function useDocuments() {
    const [documents, setDocuments] = useState<DocumentSummary[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [summary, setSummary] = useState<AddSummary | null>(null);

    const refresh = useCallback(async () => {
        try {
            setDocuments(await listDocuments());
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

    const importFiles = useCallback(async (): Promise<AddSummary | null> => {
        setError(null);
        try {
            const paths = await pickFiles();
            if (paths.length === 0) {
                return null;
            }
            const result = await addDocumentsCmd(paths);
            setSummary(result);
            setDocuments(await listDocuments());
            return result;
        } catch (err) {
            setError(toMessage(err));
            return null;
        }
    }, []);

    return { documents, loading, error, summary, importFiles, refresh };
}
