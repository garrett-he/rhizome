import { Button } from "@/components/ui/button";

interface PromptEditModalProps {
    /** settings KV 名（不含 prompt: 前缀），用于标题与 aria-label */
    name: string;
    draft: string;
    saving: boolean;
    onChange: (value: string) => void;
    onClose: () => void;
    onSave: () => void;
}

export function PromptEditModal({ name, draft, saving, onChange, onClose, onSave }: PromptEditModalProps) {
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
                aria-label={`编辑提示词 ${name}`}
                className="fixed left-1/2 top-1/2 z-50 flex max-h-[80vh] w-[720px] max-w-full -translate-x-1/2 -translate-y-1/2 flex-col rounded-lg border border-neutral-200 bg-neutral-50 shadow-lg"
            >
                <div className="px-6 py-4">
                    <h3 className="text-base font-medium text-neutral-950">编辑提示词 {name}</h3>
                </div>
                <textarea
                    aria-label="提示词内容"
                    className="mx-6 mb-4 min-h-[320px] flex-1 resize-none rounded-md border border-neutral-200 bg-white p-3 font-mono text-xs leading-5 text-neutral-950"
                    value={draft}
                    onChange={(event) => onChange(event.target.value)}
                />
                <div className="flex justify-end gap-3 px-6 pb-4">
                    <Button type="button" variant="outline" onClick={onClose}>
                        取消
                    </Button>
                    <Button type="button" disabled={saving} onClick={onSave}>
                        保存
                    </Button>
                </div>
            </div>
        </>
    );
}
