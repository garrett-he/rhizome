import type { RouteObject } from "react-router-dom";
import { OnboardingPage } from "./pages/OnboardingPage";

export const routes: RouteObject[] = [
    {
        path: "/",
        element: <OnboardingPage />,
    },
];
