import { Plug } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { testLlmConnection } from "@/lib/settings";
import { SettingsCard } from "./SettingsCard";

interface LlmCardProps {
    settings: Record<string, string | null>;
    loading: boolean;
    onSave: (key: string, value: string | null) => Promise<boolean>;
}

const FIELDS = [
    { key: "llm_base_url", label: "Base URL", type: "text", stateKey: "baseUrl" },
    { key: "llm_api_key", label: "API Key", type: "password", stateKey: "apiKey" },
    { key: "llm_model", label: "模型", type: "text", stateKey: "model" },
] as const;

export function LlmCard({ settings, loading, onSave }: LlmCardProps) {
    const [values, setValues] = useState({ baseUrl: "", apiKey: "", model: "" });
    const [saving, setSaving] = useState(false);
    const [saved, setSaved] = useState(false);
    const [testing, setTesting] = useState(false);
    const [testResult, setTestResult] = useState<{ ok: boolean; message: string } | null>(null);
    const initialized = useRef(false);

    useEffect(() => {
        if (!initialized.current && !loading) {
            setValues({
                baseUrl: settings.llm_base_url ?? "",
                apiKey: settings.llm_api_key ?? "",
                model: settings.llm_model ?? "",
            });
            initialized.current = true;
        }
    }, [loading, settings]);

    const handleSave = async () => {
        setSaving(true);
        setSaved(false);
        const ok =
            (await onSave("llm_base_url", values.baseUrl || null)) &&
            (await onSave("llm_api_key", values.apiKey || null)) &&
            (await onSave("llm_model", values.model || null));
        setSaving(false);
        setSaved(ok);
    };

    const handleTest = async () => {
        setTesting(true);
        setTestResult(null);
        try {
            await testLlmConnection(values.baseUrl, values.apiKey, values.model);
            setTestResult({ ok: true, message: "连接成功" });
        } catch (err) {
            setTestResult({ ok: false, message: err instanceof Error ? err.message : String(err) });
        } finally {
            setTesting(false);
        }
    };

    return (
        <SettingsCard title="大语言模型">
            <div className="flex flex-col gap-4">
                <div className="grid grid-cols-3 gap-4">
                    {FIELDS.map((field) => (
                        <label
                            key={field.key}
                            htmlFor={field.key}
                            className="flex flex-col gap-1.5 text-[13px] text-neutral-500"
                        >
                            {field.label}
                            <Input
                                id={field.key}
                                type={field.type}
                                value={values[field.stateKey]}
                                disabled={loading}
                                onChange={(event) => {
                                    setValues((prev) => ({ ...prev, [field.stateKey]: event.target.value }));
                                    setTestResult(null);
                                }}
                            />
                        </label>
                    ))}
                </div>
                <div className="flex items-center gap-3">
                    <Button type="button" disabled={loading || saving} onClick={() => void handleSave()}>
                        保存
                    </Button>
                    <Button
                        type="button"
                        variant="outline"
                        disabled={loading || testing}
                        onClick={() => void handleTest()}
                    >
                        <Plug aria-hidden className="h-4 w-4" />
                        {testing ? "测试中…" : "测试连接"}
                    </Button>
                    {testResult !== null && (
                        <span
                            role={testResult.ok ? "status" : "alert"}
                            className={testResult.ok ? "text-[13px] text-emerald-600" : "text-[13px] text-red-600"}
                        >
                            {testResult.message}
                        </span>
                    )}
                    {saved && <span className="text-[13px] text-neutral-500">已保存</span>}
                </div>
            </div>
        </SettingsCard>
    );
}
