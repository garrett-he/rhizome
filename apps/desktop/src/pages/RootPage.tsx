import { Navigate, Outlet } from "react-router-dom";
import { AppShell } from "@/components/layout/AppShell";
import { useProfiles } from "@/hooks/useProfiles";

export function RootPage() {
    const { profiles, loading, openVault } = useProfiles();

    if (loading) {
        return null;
    }
    if (!profiles?.current) {
        return <Navigate to="/onboarding" replace />;
    }
    return (
        <AppShell vaultPath={profiles.current} vaults={profiles.vaults} onSwitchVault={(path) => void openVault(path)}>
            {/* key 强制子路由在切换知识库后重挂载，避免各页面沿用旧 vault 的数据 */}
            <Outlet key={profiles.current} />
        </AppShell>
    );
}
