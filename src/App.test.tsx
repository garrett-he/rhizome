import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";

vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn().mockResolvedValue({
        home_dir: "C:\\Users\\Test",
        profiles: { current: null, vaults: [] },
    }),
}));

describe("App", () => {
    it("renders without errors", () => {
        const { container } = render(<App />);
        expect(container).toBeTruthy();
    });

    it("renders the onboarding page", async () => {
        render(<App />);
        expect(await screen.findByText("创建新知识库")).toBeTruthy();
        expect(screen.getByText("rhizome")).toBeTruthy();
    });
});
