import { useState } from "react";
import { Button } from "@/components/ui/button";
import { getDefaultPrompt } from "@/lib/settings";
import { PromptEditModal } from "./PromptEditModal";
import { SettingsCard } from "./SettingsCard";

export const PROMPT_NAME = "docs_compile";

const PROMPT_DESC = "文档编译：生成 YAML 元数据（title / summary / language / tags）与清洗后的 Markdown 正文";

interface PromptCardProps {
    value: string | null;
    onSave: (content: string) => Promise<boolean>;
    onReset: () => Promise<boolean>;
}

export function PromptCard({ value, onSave, onReset }: PromptCardProps) {
    const [editing, setEditing] = useState(false);
    const [draft, setDraft] = useState("");
    const [saving, setSaving] = useState(false);
    const [defaultContent, setDefaultContent] = useState<string | null>(null);
    const customized = value !== null;

    const openEdit = async () => {
        let effective = value;
        if (effective === null) {
            effective = defaultContent ?? (await getDefaultPrompt(PROMPT_NAME));
            setDefaultContent(effective);
        }
        setDraft(effective);
        setEditing(true);
    };

    const handleSave = async () => {
        setSaving(true);
        const ok = await onSave(draft);
        setSaving(false);
        if (ok) {
            setEditing(false);
        }
    };

    return (
        <SettingsCard title="提示词">
            <div className="flex flex-col gap-2">
                <div className="flex items-center gap-3 text-xs font-normal text-neutral-500">
                    <span className="w-[212px]">文件名</span>
                    <span className="flex-1">说明</span>
                    <span className="w-[162px]">操作</span>
                </div>
                <div className="flex items-center gap-3 border-b border-neutral-200 py-2.5">
                    <div className="flex w-[212px] items-center gap-1.5">
                        <span className="font-mono text-[13px] font-medium text-neutral-950">{PROMPT_NAME}</span>
                        {customized && (
                            <span className="inline-flex items-center rounded-2xl bg-neutral-100 px-2 py-0.5 text-xs font-medium text-neutral-900">
                                已定制
                            </span>
                        )}
                    </div>
                    <span className="flex-1 text-[13px] leading-6 text-neutral-500">{PROMPT_DESC}</span>
                    <div className="flex w-[162px] items-center gap-2">
                        <Button type="button" variant="outline" onClick={() => void openEdit()}>
                            编辑
                        </Button>
                        <Button type="button" variant="outline" disabled={!customized} onClick={() => void onReset()}>
                            恢复默认
                        </Button>
                    </div>
                </div>
            </div>
            {editing && (
                <PromptEditModal
                    draft={draft}
                    saving={saving}
                    onChange={setDraft}
                    onClose={() => setEditing(false)}
                    onSave={() => void handleSave()}
                />
            )}
        </SettingsCard>
    );
}
