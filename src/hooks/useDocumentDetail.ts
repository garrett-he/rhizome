import { useCallback, useEffect, useState } from "react";
import { getDocument, listChunks } from "@/lib/documents";
import type { ChunkInfo, DocumentDetail } from "@/types/document";

function toMessage(err: unknown): string {
    if (typeof err === "string") {
        return err;
    }
    if (err instanceof Error) {
        return err.message;
    }
    return String(err);
}

export function useDocumentDetail(id: string) {
    const [detail, setDetail] = useState<DocumentDetail | null>(null);
    const [chunks, setChunks] = useState<ChunkInfo[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    const refresh = useCallback(async () => {
        try {
            const [detail, chunks] = await Promise.all([getDocument(id), listChunks(id)]);
            setDetail(detail);
            setChunks(chunks);
            setError(null);
        } catch (err) {
            setError(toMessage(err));
        } finally {
            setLoading(false);
        }
    }, [id]);

    useEffect(() => {
        setLoading(true);
        setDetail(null);
        setChunks([]);
        void refresh();
    }, [refresh]);

    return { detail, chunks, loading, error, refresh };
}
