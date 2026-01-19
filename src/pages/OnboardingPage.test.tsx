import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import type { Mock } from "vitest";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Profiles } from "@/types/profile";
import { OnboardingPage } from "./OnboardingPage";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

const invokeMock = vi.mocked(invoke) as unknown as Mock;
const openMock = vi.mocked(open) as unknown as Mock;

const homeDir = "C:\\Users\\Test";

function emptyProfiles(): Profiles {
    return { current: null, vaults: [] };
}

function profilesWithVaults(vaultPaths: string[]): Profiles {
    return {
        current: vaultPaths[0] ?? null,
        vaults: vaultPaths.map((path) => ({
            path,
            added_at: "2026-10-22T00:00:00Z",
            last_opened_at: "2026-10-22T00:00:00Z",
        })),
    };
}

beforeEach(() => {
    invokeMock.mockReset();
    openMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string, args?: { path?: string }) => {
        if (cmd === "get_profiles") {
            return { home_dir: homeDir, profiles: emptyProfiles() };
        }
        if (cmd === "create_vault" || cmd === "open_vault") {
            const path = args?.path ?? "";
            return { current: path, vaults: profilesWithVaults([path]).vaults };
        }
        throw new Error(`unexpected command: ${cmd}`);
    });
});

function renderPage() {
    const user = userEvent.setup();
    render(
        <MemoryRouter initialEntries={["/onboarding"]}>
            <Routes>
                <Route path="/onboarding" element={<OnboardingPage />} />
                <Route path="/" element={<div>app-shell</div>} />
            </Routes>
        </MemoryRouter>,
    );
    return user;
}

async function renderAndFillDefaultPath() {
    const user = renderPage();
    const input = await screen.findByLabelText("知识库路径");
    await waitFor(() => {
        expect(input).toHaveValue(`${homeDir}\\.rhizome`);
    });
    return { user, input };
}

describe("OnboardingPage", () => {
    it("renders brand, labels and default vault path", async () => {
        await renderAndFillDefaultPath();

        expect(screen.getByText("rhizome")).toBeTruthy();
        expect(screen.getByText("创建新知识库")).toBeTruthy();
        expect(screen.getByText("初始化知识库")).toBeTruthy();
        expect(screen.getByText("打开已有的知识库")).toBeTruthy();
        expect(screen.getByText("选择其他目录…")).toBeTruthy();
        expect(screen.queryByText("C:\\vaults\\a")).toBeNull();
    });

    it("browse fills the path input from the directory dialog", async () => {
        const { user, input } = await renderAndFillDefaultPath();
        openMock.mockResolvedValue("D:\\picked");

        await user.click(screen.getByRole("button", { name: "浏览" }));

        await waitFor(() => {
            expect(input).toHaveValue("D:\\picked");
        });
        expect(openMock).toHaveBeenCalledWith({ directory: true, multiple: false });
    });

    it("init invokes create_vault and navigates to the app shell", async () => {
        const { user } = await renderAndFillDefaultPath();

        await user.click(screen.getByRole("button", { name: "初始化知识库" }));

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("create_vault", {
                path: `${homeDir}\\.rhizome`,
            });
        });
        expect(await screen.findByText("app-shell")).toBeTruthy();
    });

    it("clicking a recent vault invokes open_vault and navigates", async () => {
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "get_profiles") {
                return { home_dir: homeDir, profiles: profilesWithVaults(["C:\\vaults\\a"]) };
            }
            if (cmd === "open_vault") {
                return profilesWithVaults(["C:\\vaults\\a"]);
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        const user = renderPage();

        await user.click(await screen.findByText("C:\\vaults\\a"));

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("open_vault", { path: "C:\\vaults\\a" });
        });
        expect(await screen.findByText("app-shell")).toBeTruthy();
    });

    it("picking another directory opens it as vault and navigates", async () => {
        const { user } = await renderAndFillDefaultPath();
        openMock.mockResolvedValue("E:\\other");

        await user.click(screen.getByRole("button", { name: "选择其他目录…" }));

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("open_vault", { path: "E:\\other" });
        });
        expect(await screen.findByText("app-shell")).toBeTruthy();
    });

    it("cancelling the dialog does not open a vault", async () => {
        const { user } = await renderAndFillDefaultPath();
        openMock.mockResolvedValue(null);

        await user.click(screen.getByRole("button", { name: "选择其他目录…" }));

        await waitFor(() => {
            expect(openMock).toHaveBeenCalled();
        });
        expect(invokeMock).not.toHaveBeenCalledWith("open_vault", expect.anything());
        expect(screen.getByText("选择其他目录…")).toBeTruthy();
    });

    it("shows inline error when creating fails", async () => {
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "get_profiles") {
                return { home_dir: homeDir, profiles: emptyProfiles() };
            }
            return Promise.reject("路径已存在且不是目录");
        });
        const { user } = await renderAndFillDefaultPath();

        await user.click(screen.getByRole("button", { name: "初始化知识库" }));

        expect(await screen.findByRole("alert")).toHaveTextContent("路径已存在且不是目录");
        expect(screen.getByText("选择其他目录…")).toBeTruthy();
    });

    it("shows inline error when opening a missing directory", async () => {
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "get_profiles") {
                return { home_dir: homeDir, profiles: profilesWithVaults(["C:\\gone"]) };
            }
            return Promise.reject("目录不存在: C:\\gone");
        });
        const user = renderPage();

        await user.click(await screen.findByText("C:\\gone"));

        expect(await screen.findByRole("alert")).toHaveTextContent("目录不存在: C:\\gone");
        expect(screen.getByText("选择其他目录…")).toBeTruthy();
    });

    it("shows inline error when the directory dialog fails", async () => {
        const { user } = await renderAndFillDefaultPath();
        openMock.mockRejectedValue("对话框不可用");

        await user.click(screen.getByRole("button", { name: "选择其他目录…" }));

        expect(await screen.findByRole("alert")).toHaveTextContent("对话框不可用");
    });
});
