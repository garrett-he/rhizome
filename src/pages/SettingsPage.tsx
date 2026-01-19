import { EmbeddingCard } from "@/components/settings/EmbeddingCard";
import { LlmCard } from "@/components/settings/LlmCard";
import { McpCard } from "@/components/settings/McpCard";
import { PROMPT_NAME, PromptCard } from "@/components/settings/PromptCard";
import { useSettings } from "@/hooks/useSettings";

const PROMPT_KEY = `prompt:${PROMPT_NAME}`;

export function SettingsPage() {
    const { settings, loading, error, save } = useSettings();

    return (
        <div className="flex flex-col gap-5 p-8">
            <h1 className="text-2xl font-semibold text-neutral-950">设置</h1>

            {error && (
                <p role="alert" className="text-[13px] text-red-600">
                    {error}
                </p>
            )}

            <EmbeddingCard
                value={settings.embedding_model ?? null}
                onSave={(value) => void save("embedding_model", value)}
            />
            <LlmCard settings={settings} loading={loading} onSave={save} />
            <PromptCard
                value={settings[PROMPT_KEY] ?? null}
                onSave={(content) => save(PROMPT_KEY, content)}
                onReset={() => save(PROMPT_KEY, null)}
            />
            <McpCard />
        </div>
    );
}
