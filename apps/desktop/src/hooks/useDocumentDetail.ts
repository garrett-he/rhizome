import { useCallback, useEffect, useRef, useState } from "react";
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
    // 时序保护：快速切换文档 id 时，旧请求的响应乱序到达会被丢弃，避免覆盖新数据
    const seqRef = useRef(0);

    const refresh = useCallback(async () => {
        const seq = ++seqRef.current;
        try {
            const [detail, chunks] = await Promise.all([getDocument(id), listChunks(id)]);
            if (seq !== seqRef.current) {
                return;
            }
            setDetail(detail);
            setChunks(chunks);
            setError(null);
        } catch (err) {
            if (seq !== seqRef.current) {
                return;
            }
            setError(toMessage(err));
        } finally {
            if (seq === seqRef.current) {
                setLoading(false);
            }
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
