import { render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { routes } from "./routes";

function renderWithRouter(initialEntries = ["/"]) {
    const router = createMemoryRouter(routes, { initialEntries });
    return render(<RouterProvider router={router} />);
}

describe("App", () => {
    it("renders without errors", () => {
        const { container } = renderWithRouter();
        expect(container).toBeTruthy();
    });

    it("renders the home page heading", () => {
        renderWithRouter();
        expect(screen.getByText("Welcome to Tauri + React")).toBeTruthy();
    });
});
