import type { RouteObject } from "react-router-dom";
import { CategoriesPage } from "./pages/CategoriesPage";
import { DocumentDetailPage } from "./pages/DocumentDetailPage";
import { DocumentLibraryPage } from "./pages/DocumentLibraryPage";
import { OnboardingPage } from "./pages/OnboardingPage";
import { OverviewPage } from "./pages/OverviewPage";
import { QaPage } from "./pages/QaPage";
import { RootPage } from "./pages/RootPage";
import { SearchPage } from "./pages/SearchPage";
import { SettingsPage } from "./pages/SettingsPage";

export const routes: RouteObject[] = [
    {
        path: "/",
        element: <RootPage />,
        children: [
            { index: true, element: <OverviewPage /> },
            { path: "documents", element: <DocumentLibraryPage /> },
            { path: "documents/:id", element: <DocumentDetailPage /> },
            { path: "search", element: <SearchPage /> },
            { path: "qa", element: <QaPage /> },
            { path: "categories", element: <CategoriesPage /> },
            { path: "settings", element: <SettingsPage /> },
        ],
    },
    {
        path: "/onboarding",
        element: <OnboardingPage />,
    },
];
