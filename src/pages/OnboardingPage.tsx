import { ChevronRight, Folder, FolderOpen, FolderSearch, Network, Sparkles } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useProfiles } from "@/hooks/useProfiles";
import type { Vault } from "@/types/profile";

function joinVaultPath(homeDir: string): string {
    if (!homeDir) {
        return "";
    }
    const sep = homeDir.includes("\\") ? "\\" : "/";
    return `${homeDir}${sep}.rhizome`;
}

export function OnboardingPage() {
    const navigate = useNavigate();
    const { homeDir, profiles, loading, error, createVault, openVault, pickDirectory } = useProfiles();
    const [path, setPath] = useState("");
    const [busy, setBusy] = useState(false);

    const defaultPath = useMemo(() => joinVaultPath(homeDir), [homeDir]);

    useEffect(() => {
        if (defaultPath && !path) {
            setPath(defaultPath);
        }
    }, [defaultPath, path]);

    const handleBrowse = useCallback(async () => {
        const selected = await pickDirectory();
        if (selected) {
            setPath(selected);
        }
    }, [pickDirectory]);

    const handleInit = useCallback(async () => {
        setBusy(true);
        try {
            const ok = await createVault(path.trim());
            if (ok) {
                navigate("/", { replace: true });
            }
        } finally {
            setBusy(false);
        }
    }, [createVault, navigate, path]);

    const handleOpenVault = useCallback(
        async (vaultPath: string) => {
            setBusy(true);
            try {
                const ok = await openVault(vaultPath);
                if (ok) {
                    navigate("/", { replace: true });
                }
            } finally {
                setBusy(false);
            }
        },
        [navigate, openVault],
    );

    const handleOpenOther = useCallback(async () => {
        const selected = await pickDirectory();
        if (selected) {
            await handleOpenVault(selected);
        }
    }, [handleOpenVault, pickDirectory]);

    const vaults = profiles?.vaults ?? [];

    return (
        <main className="flex min-h-screen flex-col items-center justify-center gap-6 bg-neutral-100 p-6">
            <div className="flex items-center gap-3">
                <div className="flex h-10 w-10 items-center justify-center rounded-[10px] bg-neutral-900">
                    <Network aria-hidden className="h-[22px] w-[22px] text-neutral-50" />
                </div>
                <h1 className="text-[22px] font-semibold text-neutral-950">rhizome</h1>
            </div>

            <section className="w-[560px] max-w-full rounded-lg border border-neutral-200 bg-neutral-50 shadow-sm">
                <div className="flex flex-col gap-3.5 p-6">
                    <h2 className="text-[13px] font-medium text-neutral-950">创建新知识库</h2>

                    <div className="flex gap-2">
                        <Input
                            aria-label="知识库路径"
                            value={path}
                            disabled={busy}
                            onChange={(event) => setPath(event.target.value)}
                        />
                        <Button
                            type="button"
                            variant="outline"
                            disabled={busy || loading}
                            onClick={() => void handleBrowse()}
                        >
                            <FolderOpen aria-hidden className="h-4 w-4" />
                            浏览
                        </Button>
                    </div>

                    <Button type="button" disabled={busy || loading || !path.trim()} onClick={() => void handleInit()}>
                        <Sparkles aria-hidden className="h-4 w-4" />
                        初始化知识库
                    </Button>

                    <div aria-hidden className="flex items-center gap-3 py-1">
                        <span className="h-px flex-1 bg-neutral-200" />
                        <span className="text-xs text-neutral-500">或</span>
                        <span className="h-px flex-1 bg-neutral-200" />
                    </div>

                    <h2 className="text-[13px] font-medium text-neutral-950">打开已有的知识库</h2>

                    {vaults.length > 0 && (
                        <ul className="flex flex-col gap-2">
                            {vaults.map((vault: Vault) => (
                                <li key={vault.path}>
                                    <button
                                        type="button"
                                        className="flex w-full items-center gap-3 rounded-lg border border-neutral-200 bg-neutral-50 px-3 py-2.5 text-left transition-colors hover:bg-neutral-100 disabled:pointer-events-none disabled:opacity-50"
                                        disabled={busy}
                                        onClick={() => void handleOpenVault(vault.path)}
                                    >
                                        <Folder aria-hidden className="h-[18px] w-[18px] shrink-0 text-neutral-500" />
                                        <span className="flex-1 truncate text-[13px] font-medium text-neutral-950">
                                            {vault.path}
                                        </span>
                                        <ChevronRight aria-hidden className="h-4 w-4 shrink-0 text-neutral-500" />
                                    </button>
                                </li>
                            ))}
                        </ul>
                    )}

                    <Button
                        type="button"
                        variant="outline"
                        disabled={busy || loading}
                        onClick={() => void handleOpenOther()}
                    >
                        <FolderSearch aria-hidden className="h-4 w-4" />
                        选择其他目录…
                    </Button>

                    {error && (
                        <p role="alert" className="text-[13px] text-red-600">
                            {error}
                        </p>
                    )}
                </div>
            </section>
        </main>
    );
}
