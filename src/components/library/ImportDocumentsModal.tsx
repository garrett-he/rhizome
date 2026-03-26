import { Loader2, Upload } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { pickFiles } from "@/lib/documents";
import type { CategorySummary } from "@/types/category";

interface ImportDocumentsModalProps {
    categories: CategorySummary[];
    disabled: boolean;
    onClose: () => void;
    onImport: (categoryName: string | undefined, paths: string[]) => Promise<void>;
}

function fileName(path: string): string {
    return path.split(/[\\/]/).pop() || path;
}

export function ImportDocumentsModal({ categories, disabled, onClose, onImport }: ImportDocumentsModalProps) {
    const [categoryName, setCategoryName] = useState("");
    const [paths, setPaths] = useState<string[]>([]);
    const [selecting, setSelecting] = useState(false);
    const [importing, setImporting] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const selectFiles = async () => {
        setSelecting(true);
        setError(null);
        try {
            setPaths(await pickFiles());
        } catch (err) {
            setError(err instanceof Error ? err.message : String(err));
        } finally {
            setSelecting(false);
        }
    };

    const submit = async () => {
        if (paths.length === 0) {
            return;
        }
        setImporting(true);
        try {
            await onImport(categoryName === "" ? undefined : categoryName, paths);
        } finally {
            setImporting(false);
            onClose();
        }
    };

    const busy = disabled || importing;

    return (
        <>
            <button
                type="button"
                aria-label="关闭弹窗"
                className="fixed inset-0 z-50 cursor-default bg-black/40"
                onClick={onClose}
            />
            <div
                role="dialog"
                aria-label="导入文档"
                className="fixed left-1/2 top-1/2 z-50 flex w-[520px] max-w-full -translate-x-1/2 -translate-y-1/2 flex-col rounded-lg border border-neutral-200 bg-neutral-50 shadow-lg"
            >
                <div className="px-6 py-4">
                    <h3 className="text-base font-medium text-neutral-950">导入文档</h3>
                </div>
                <div className="flex flex-col gap-4 px-6 pb-4">
                    <label htmlFor="import-category" className="flex flex-col gap-1.5 text-[13px] text-neutral-500">
                        导入分类
                        <select
                            id="import-category"
                            value={categoryName}
                            disabled={busy || selecting}
                            onChange={(event) => setCategoryName(event.target.value)}
                            className="h-9 rounded-md border border-neutral-200 bg-white px-2 text-sm text-neutral-950 outline-none transition-colors focus:border-neutral-400"
                        >
                            <option value="">无</option>
                            {categories.map((category) => (
                                <option key={category.id} value={category.name}>
                                    {category.name}
                                </option>
                            ))}
                        </select>
                    </label>
                    <div className="flex flex-col gap-2">
                        <span className="text-[13px] text-neutral-500">文件</span>
                        <button
                            type="button"
                            disabled={busy || selecting}
                            onClick={() => void selectFiles()}
                            className="flex flex-col items-center justify-center gap-1.5 rounded-md border border-dashed border-neutral-300 bg-white px-4 py-6 text-[13px] text-neutral-500 transition-colors hover:border-neutral-400 hover:text-neutral-950 disabled:cursor-not-allowed disabled:opacity-50"
                        >
                            {selecting ? (
                                <Loader2 aria-hidden className="h-5 w-5 animate-spin" />
                            ) : (
                                <Upload aria-hidden className="h-5 w-5" />
                            )}
                            {selecting ? "选择中…" : "选择文件"}
                        </button>
                        {paths.length > 0 && (
                            <ul aria-label="已选文件" className="flex flex-col gap-1 text-[13px] text-neutral-500">
                                {paths.map((path) => (
                                    <li key={path} title={path} className="truncate">
                                        {fileName(path)}
                                    </li>
                                ))}
                            </ul>
                        )}
                        {error && (
                            <p role="alert" className="text-[13px] text-red-600">
                                {error}
                            </p>
                        )}
                    </div>
                </div>
                <div className="flex justify-end gap-3 px-6 pb-4">
                    <Button type="button" variant="outline" disabled={importing} onClick={onClose}>
                        取消
                    </Button>
                    <Button
                        type="button"
                        disabled={busy || selecting || paths.length === 0}
                        onClick={() => void submit()}
                    >
                        {importing ? "导入中…" : "导入"}
                    </Button>
                </div>
            </div>
        </>
    );
}
