import { Check, FolderPlus, Pencil, RefreshCw, Trash2, X } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useCategories } from "@/hooks/useCategories";
import type { CategorySummary } from "@/types/category";

function RenameRow({
    category,
    busy,
    onConfirm,
    onCancel,
}: {
    category: CategorySummary;
    busy: boolean;
    onConfirm: (name: string) => void;
    onCancel: () => void;
}) {
    const [name, setName] = useState(category.name);
    return (
        <form
            className="flex items-center gap-2"
            onSubmit={(event) => {
                event.preventDefault();
                onConfirm(name);
            }}
        >
            <Input
                aria-label="新分类名"
                value={name}
                onChange={(event) => setName(event.target.value)}
                disabled={busy}
                autoFocus
                className="h-8 max-w-[220px]"
            />
            <Button type="submit" variant="outline" size="sm" disabled={busy || name.trim() === ""}>
                <Check aria-hidden className="h-3.5 w-3.5" />
                确认
            </Button>
            <Button type="button" variant="ghost" size="sm" disabled={busy} onClick={onCancel}>
                <X aria-hidden className="h-3.5 w-3.5" />
                取消
            </Button>
        </form>
    );
}

export function CategoriesPage() {
    const { categories, loading, error, note, create, rename, remove, refresh } = useCategories();
    const [newName, setNewName] = useState("");
    const [creating, setCreating] = useState(false);
    const [renamingId, setRenamingId] = useState<string | null>(null);
    const [renaming, setRenaming] = useState(false);
    const [deleting, setDeleting] = useState<string | null>(null);

    const submitCreate = () => {
        const trimmed = newName.trim();
        if (trimmed === "") {
            return;
        }
        setCreating(true);
        void create(trimmed).then((ok) => {
            setCreating(false);
            if (ok) {
                setNewName("");
            }
        });
    };

    return (
        <div className="flex flex-col gap-5 p-8">
            <div className="flex items-center justify-between">
                <h1 className="text-2xl font-semibold text-neutral-950">分类管理</h1>
                <Button type="button" variant="outline" disabled={loading} onClick={() => void refresh()}>
                    <RefreshCw aria-hidden className="h-4 w-4" />
                    刷新
                </Button>
            </div>

            <form className="flex items-center gap-3" onSubmit={(event) => event.preventDefault()}>
                <Input
                    aria-label="分类名"
                    placeholder="输入分类名，如：技术笔记"
                    value={newName}
                    onChange={(event) => setNewName(event.target.value)}
                    disabled={creating}
                    className="h-9 max-w-[280px]"
                />
                <Button type="button" disabled={creating || newName.trim() === ""} onClick={submitCreate}>
                    <FolderPlus aria-hidden className="h-4 w-4" />
                    新建分类
                </Button>
            </form>

            {error && (
                <p role="alert" className="text-[13px] text-red-600">
                    {error}
                </p>
            )}
            {note && !error && <p className="text-[13px] text-neutral-500">{note}</p>}

            {categories.length === 0 ? (
                <div className="flex items-center justify-center rounded-lg border border-neutral-200 bg-neutral-50 py-16 text-[13px] text-neutral-500">
                    {loading ? "加载中…" : "暂无分类，在上方输入名称创建"}
                </div>
            ) : (
                <ul className="flex flex-col overflow-hidden rounded-lg border border-neutral-200 bg-neutral-50 shadow-sm">
                    {categories.map((category) => (
                        <li
                            key={category.id}
                            className="flex items-center justify-between gap-4 border-b border-neutral-200 px-4 py-3 last:border-b-0"
                        >
                            {renamingId === category.id ? (
                                <RenameRow
                                    category={category}
                                    busy={renaming}
                                    onConfirm={(name) => {
                                        setRenaming(true);
                                        void rename(category.id, name).then((ok) => {
                                            setRenaming(false);
                                            if (ok) {
                                                setRenamingId(null);
                                            }
                                        });
                                    }}
                                    onCancel={() => setRenamingId(null)}
                                />
                            ) : (
                                <>
                                    <div className="flex min-w-0 items-center gap-3">
                                        <span className="truncate text-sm font-medium text-neutral-950">
                                            {category.name}
                                        </span>
                                        <span className="shrink-0 rounded-2xl bg-neutral-100 px-2 py-0.5 text-xs font-medium text-neutral-900">
                                            {category.document_count} 个文档
                                        </span>
                                    </div>
                                    <div className="flex shrink-0 gap-1">
                                        <Button
                                            type="button"
                                            variant="ghost"
                                            size="sm"
                                            disabled={deleting !== null}
                                            onClick={() => setRenamingId(category.id)}
                                        >
                                            <Pencil aria-hidden className="h-3.5 w-3.5" />
                                            重命名
                                        </Button>
                                        <Button
                                            type="button"
                                            variant="ghost"
                                            size="sm"
                                            disabled={deleting !== null}
                                            className="text-red-600 hover:text-red-700"
                                            onClick={() => {
                                                setDeleting(category.id);
                                                void remove(category).then(() => setDeleting(null));
                                            }}
                                        >
                                            <Trash2 aria-hidden className="h-3.5 w-3.5" />
                                            删除
                                        </Button>
                                    </div>
                                </>
                            )}
                        </li>
                    ))}
                </ul>
            )}
        </div>
    );
}
