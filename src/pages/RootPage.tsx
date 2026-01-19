import { Navigate } from "react-router-dom";
import { AppShell } from "@/components/layout/AppShell";
import { useProfiles } from "@/hooks/useProfiles";
import { OverviewPage } from "@/pages/OverviewPage";

export function RootPage() {
    const { profiles, loading } = useProfiles();

    if (loading) {
        return null;
    }
    if (!profiles?.current) {
        return <Navigate to="/onboarding" replace />;
    }
    return (
        <AppShell vaultPath={profiles.current}>
            <OverviewPage />
        </AppShell>
    );
}
