import { EmbeddingCard } from "@/components/settings/EmbeddingCard";
import { LlmCard } from "@/components/settings/LlmCard";
import { McpCard } from "@/components/settings/McpCard";
import { PromptCard } from "@/components/settings/PromptCard";
import { useSettings } from "@/hooks/useSettings";

const COMPILE_PROMPT_KEY = "prompt:docs_compile";
const ANSWER_PROMPT_KEY = "prompt:docs_answer";

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
                name="docs_compile"
                title="编译提示词"
                description="文档编译：生成 YAML 元数据（title / summary / language / tags）与清洗后的 Markdown 正文"
                value={settings[COMPILE_PROMPT_KEY] ?? null}
                onSave={(content) => save(COMPILE_PROMPT_KEY, content)}
                onReset={() => save(COMPILE_PROMPT_KEY, null)}
            />
            <PromptCard
                name="docs_answer"
                title="问答提示词"
                description="知识库问答：把检索到的上下文与用户问题组装为带引用标注的回答"
                value={settings[ANSWER_PROMPT_KEY] ?? null}
                onSave={(content) => save(ANSWER_PROMPT_KEY, content)}
                onReset={() => save(ANSWER_PROMPT_KEY, null)}
            />
            <McpCard />
        </div>
    );
}
