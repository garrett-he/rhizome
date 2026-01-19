import type { RouteObject } from "react-router-dom";
import { OnboardingPage } from "./pages/OnboardingPage";
import { RootPage } from "./pages/RootPage";

export const routes: RouteObject[] = [
    {
        path: "/",
        element: <RootPage />,
    },
    {
        path: "/onboarding",
        element: <OnboardingPage />,
    },
];
