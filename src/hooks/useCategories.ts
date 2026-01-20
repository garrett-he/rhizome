import { useCallback, useEffect, useState } from "react";
import { createCategory, deleteCategory, listCategories, renameCategory } from "@/lib/categories";
import type { CategorySummary } from "@/types/category";

function toMessage(err: unknown): string {
    if (typeof err === "string") {
        return err;
    }
    if (err instanceof Error) {
        return err.message;
    }
    return String(err);
}

export function useCategories() {
    const [categories, setCategories] = useState<CategorySummary[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [note, setNote] = useState<string | null>(null);

    const refresh = useCallback(async () => {
        try {
            setCategories(await listCategories());
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

    const create = useCallback(
        async (name: string) => {
            const trimmed = name.trim();
            if (trimmed === "") {
                setError("分类名不能为空");
                return false;
            }
            try {
                await createCategory(trimmed);
                setError(null);
                setNote(`已创建分类「${trimmed}」`);
                await refresh();
                return true;
            } catch (err) {
                setError(toMessage(err));
                return false;
            }
        },
        [refresh],
    );

    const rename = useCallback(
        async (id: string, name: string) => {
            const trimmed = name.trim();
            if (trimmed === "") {
                setError("分类名不能为空");
                return false;
            }
            try {
                await renameCategory(id, trimmed);
                setError(null);
                setNote(`已重命名为「${trimmed}」`);
                await refresh();
                return true;
            } catch (err) {
                setError(toMessage(err));
                return false;
            }
        },
        [refresh],
    );

    const remove = useCallback(
        async (category: CategorySummary) => {
            const message =
                category.document_count > 0
                    ? `删除分类「${category.name}」将把 ${category.document_count} 个文档变为未分类，确定删除？`
                    : `确定删除分类「${category.name}」？`;
            if (!window.confirm(message)) {
                return;
            }
            try {
                const outcome = await deleteCategory(category.id);
                setError(null);
                setNote(
                    outcome.detached > 0
                        ? `已删除分类「${category.name}」，${outcome.detached} 个文档已变为未分类`
                        : `已删除分类「${category.name}」`,
                );
                await refresh();
            } catch (err) {
                setError(toMessage(err));
            }
        },
        [refresh],
    );

    return { categories, loading, error, note, create, rename, remove, refresh };
}
