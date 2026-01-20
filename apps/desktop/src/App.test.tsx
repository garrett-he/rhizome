import { invoke } from "@tauri-apps/api/core";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke) as unknown as import("vitest").Mock;

beforeEach(() => {
    invokeMock.mockReset();
    window.history.pushState({}, "", "/");
});

describe("App", () => {
    it("renders without errors", () => {
        invokeMock.mockResolvedValue({
            home_dir: "C:\\Users\\Test",
            profiles: { current: null, vaults: [] },
        });
        const { container } = render(<App />);
        expect(container).toBeTruthy();
    });

    it("renders the onboarding page when no vault is selected", async () => {
        invokeMock.mockResolvedValue({
            home_dir: "C:\\Users\\Test",
            profiles: { current: null, vaults: [] },
        });
        render(<App />);
        expect(await screen.findByText("创建新知识库")).toBeTruthy();
        expect(screen.getByText("rhizome")).toBeTruthy();
    });

    it("renders the app shell with overview when a vault is selected", async () => {
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "get_profiles") {
                return {
                    home_dir: "C:\\Users\\Test",
                    profiles: { current: "C:\\vaults\\main", vaults: [] },
                };
            }
            if (cmd === "get_overview") {
                return {
                    total_documents: 0,
                    indexed_documents: 0,
                    pending_documents: 0,
                    failed_documents: 0,
                    storage_bytes: 0,
                    categories: [],
                };
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        render(<App />);

        expect(await screen.findByText("知识库总览")).toBeTruthy();
        expect(screen.getByText("C:\\vaults\\main")).toBeTruthy();
    });
});
