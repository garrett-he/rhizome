import { RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useOverview } from "@/hooks/useOverview";
import { SettingsCard } from "./SettingsCard";

const EMBEDDING_MODELS = [
    { id: "BAAI/bge-small-zh-v1.5", label: "BAAI/bge-small-zh-v1.5（384 维）" },
    { id: "BAAI/bge-small-en-v1.5", label: "BAAI/bge-small-en-v1.5（384 维）" },
    { id: "BAAI/bge-m3", label: "BAAI/bge-m3（1024 维）" },
];

interface EmbeddingCardProps {
    value: string | null;
    onSave: (value: string) => void;
}

export function EmbeddingCard({ value, onSave }: EmbeddingCardProps) {
    const { overview } = useOverview();
    const current = value ?? EMBEDDING_MODELS[0].id;
    const known = EMBEDDING_MODELS.some((model) => model.id === current);
    const options = known ? EMBEDDING_MODELS : [{ id: current, label: current }, ...EMBEDDING_MODELS];

    return (
        <SettingsCard title="嵌入模型">
            <div className="flex flex-col gap-3">
                <select
                    aria-label="嵌入模型"
                    className="h-9 w-full rounded-md border border-neutral-200 bg-neutral-50 px-3 text-sm text-neutral-950"
                    value={current}
                    onChange={(event) => onSave(event.target.value)}
                >
                    {options.map((model) => (
                        <option key={model.id} value={model.id}>
                            {model.label}
                        </option>
                    ))}
                </select>
                <div className="flex items-center gap-3">
                    <Button type="button" variant="outline" disabled title="索引功能将在后续版本提供">
                        <RefreshCw aria-hidden className="h-4 w-4" />
                        重新索引全部文档
                    </Button>
                    <span className="inline-flex items-center rounded-2xl bg-neutral-100 px-2 py-0.5 text-xs font-medium text-neutral-900">
                        当前 {overview?.indexed_documents ?? 0} 篇已索引
                    </span>
                </div>
            </div>
        </SettingsCard>
    );
}
