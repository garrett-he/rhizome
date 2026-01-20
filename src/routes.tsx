import type { RouteObject } from "react-router-dom";
import { DocumentDetailPage } from "./pages/DocumentDetailPage";
import { DocumentLibraryPage } from "./pages/DocumentLibraryPage";
import { OnboardingPage } from "./pages/OnboardingPage";
import { OverviewPage } from "./pages/OverviewPage";
import { RootPage } from "./pages/RootPage";
import { SettingsPage } from "./pages/SettingsPage";

export const routes: RouteObject[] = [
    {
        path: "/",
        element: <RootPage />,
        children: [
            { index: true, element: <OverviewPage /> },
            { path: "documents", element: <DocumentLibraryPage /> },
            { path: "documents/:id", element: <DocumentDetailPage /> },
            { path: "settings", element: <SettingsPage /> },
        ],
    },
    {
        path: "/onboarding",
        element: <OnboardingPage />,
    },
];
