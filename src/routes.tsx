import type { RouteObject } from "react-router-dom";
import { DocumentLibraryPage } from "./pages/DocumentLibraryPage";
import { OnboardingPage } from "./pages/OnboardingPage";
import { OverviewPage } from "./pages/OverviewPage";
import { RootPage } from "./pages/RootPage";

export const routes: RouteObject[] = [
    {
        path: "/",
        element: <RootPage />,
        children: [
            { index: true, element: <OverviewPage /> },
            { path: "documents", element: <DocumentLibraryPage /> },
        ],
    },
    {
        path: "/onboarding",
        element: <OnboardingPage />,
    },
];
