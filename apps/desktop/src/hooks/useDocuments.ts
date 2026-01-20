import { useCallback, useEffect, useState } from "react";
import {
    addDocuments as addDocumentsCmd,
    compileDocuments,
    indexDocuments,
    listDocuments,
    pickFiles,
    processDocuments,
} from "@/lib/documents";
import type { AddSummary, DocumentSummary, ProcessSummary } from "@/types/document";

function toMessage(err: unknown): string {
    if (typeof err === "string") {
        return err;
    }
    if (err instanceof Error) {
        return err.message;
    }
    return String(err);
}

function summarize(result: ProcessSummary): string {
    const failures = result.compile_failures + result.index_failures;
    const base = `编译 ${result.compiled} · 索引 ${result.indexed}`;
    return failures > 0 ? `${base} · 失败 ${failures}` : base;
}

export function useDocuments() {
    const [documents, setDocuments] = useState<DocumentSummary[]>([]);
    const [loading, setLoading] = useState(true);
    const [processing, setProcessing] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [summary, setSummary] = useState<AddSummary | null>(null);
    const [processNote, setProcessNote] = useState<string | null>(null);

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

    const importFiles = useCallback(async (categoryName?: string): Promise<AddSummary | null> => {
        setError(null);
        setProcessNote(null);
        try {
            const paths = await pickFiles();
            if (paths.length === 0) {
                return null;
            }
            const result = await addDocumentsCmd(paths, categoryName);
            setSummary(result);
            if (result.added > 0) {
                // 自动 compile→index；LLM/embedding 未配置或失败时静默保持 raw，
                // 用户可稍后手动重试
                try {
                    const processed = await processDocuments();
                    setProcessNote(`自动处理完成：${summarize(processed)}`);
                } catch {
                    setProcessNote(null);
                }
            }
            setDocuments(await listDocuments());
            return result;
        } catch (err) {
            setError(toMessage(err));
            return null;
        }
    }, []);

    /// 手动全量处理（编译全部 raw → 索引全部 compiled），错误显式展示
    const processAll = useCallback(async (): Promise<ProcessSummary | null> => {
        setError(null);
        setProcessNote(null);
        setProcessing(true);
        try {
            const result = await processDocuments();
            setProcessNote(`处理完成：${summarize(result)}`);
            setDocuments(await listDocuments());
            return result;
        } catch (err) {
            setError(toMessage(err));
            return null;
        } finally {
            setProcessing(false);
        }
    }, []);

    /// 手动重试单个文档：先编译（已编译则跳过）再索引
    const processDocument = useCallback(async (id: string): Promise<boolean> => {
        setError(null);
        setProcessing(true);
        try {
            await compileDocuments([id]);
            const outcomes = await indexDocuments([id]);
            setDocuments(await listDocuments());
            return outcomes.every((o) => o.ok);
        } catch (err) {
            setError(toMessage(err));
            return false;
        } finally {
            setProcessing(false);
        }
    }, []);

    return {
        documents,
        loading,
        processing,
        error,
        summary,
        processNote,
        importFiles,
        processAll,
        processDocument,
        refresh,
    };
}
