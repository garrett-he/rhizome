import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import { AppShell } from "./AppShell";

function renderShell(initialEntries = ["/"]) {
    return render(
        <MemoryRouter initialEntries={initialEntries}>
            <AppShell vaultPath={"C:\\vaults\\main"}>
                <div>overview-content</div>
            </AppShell>
        </MemoryRouter>,
    );
}

describe("AppShell", () => {
    it("renders brand, nav sections, vault path and content", () => {
        renderShell();

        expect(screen.getByText("rhizome")).toBeTruthy();
        expect(screen.getByText("工作区")).toBeTruthy();
        expect(screen.getByText("管理")).toBeTruthy();
        expect(screen.getByText("C:\\vaults\\main")).toBeTruthy();
        expect(screen.getByText("overview-content")).toBeTruthy();
    });

    it("enables navigation for overview, documents, search, qa and settings only", () => {
        renderShell();

        expect(screen.getByRole("button", { name: "总览" })).not.toBeDisabled();
        expect(screen.getByRole("button", { name: "文档库" })).not.toBeDisabled();
        expect(screen.getByRole("button", { name: "搜索" })).not.toBeDisabled();
        expect(screen.getByRole("button", { name: "问答" })).not.toBeDisabled();
        expect(screen.getByRole("button", { name: "设置" })).not.toBeDisabled();
        expect(screen.getByRole("button", { name: "分类管理" })).toBeDisabled();
        expect(screen.getByRole("button", { name: "任务中心" })).toBeDisabled();
    });

    it("marks the current route as active", () => {
        renderShell(["/documents"]);

        expect(screen.getByRole("button", { name: "文档库" }).getAttribute("aria-current")).toBe("page");
        expect(screen.getByRole("button", { name: "总览" }).getAttribute("aria-current")).toBeNull();
    });

    it("navigates between enabled nav items", async () => {
        const user = userEvent.setup();
        render(
            <MemoryRouter initialEntries={["/documents"]}>
                <Routes>
                    <Route
                        path="*"
                        element={
                            <AppShell vaultPath={"C:\\vaults\\main"}>
                                <div>shell-content</div>
                            </AppShell>
                        }
                    />
                </Routes>
            </MemoryRouter>,
        );

        await user.click(screen.getByRole("button", { name: "总览" }));

        expect(screen.getByRole("button", { name: "总览" }).getAttribute("aria-current")).toBe("page");
        expect(screen.getByRole("button", { name: "文档库" }).getAttribute("aria-current")).toBeNull();
    });

    it("shows vault menu with check on current vault and manage entry", async () => {
        const user = userEvent.setup();
        render(
            <MemoryRouter initialEntries={["/"]}>
                <Routes>
                    <Route
                        path="*"
                        element={
                            <AppShell
                                vaultPath={"C:\\vaults\\main"}
                                vaults={[
                                    { path: "C:\\vaults\\main", added_at: "", last_opened_at: "" },
                                    { path: "C:\\vaults\\other", added_at: "", last_opened_at: "" },
                                ]}
                            >
                                <div>shell-content</div>
                            </AppShell>
                        }
                    />
                    <Route path="/onboarding" element={<div>onboarding-page</div>} />
                </Routes>
            </MemoryRouter>,
        );

        await user.click(screen.getByRole("button", { name: "C:\\vaults\\main" }));

        const menu = screen.getByRole("menu");
        expect(menu).toBeTruthy();
        expect(screen.getByText("C:\\vaults\\other")).toBeTruthy();
        expect(screen.getByRole("menuitem", { name: "管理知识库" })).toBeTruthy();

        await user.click(screen.getByRole("menuitem", { name: "管理知识库" }));

        expect(screen.getByText("onboarding-page")).toBeTruthy();
        expect(screen.queryByRole("menu")).toBeNull();
    });

    it("switches vault via menu and closes it", async () => {
        const onSwitchVault = vi.fn();
        const user = userEvent.setup();
        render(
            <MemoryRouter initialEntries={["/"]}>
                <AppShell
                    vaultPath={"C:\\vaults\\main"}
                    vaults={[
                        { path: "C:\\vaults\\main", added_at: "", last_opened_at: "" },
                        { path: "C:\\vaults\\other", added_at: "", last_opened_at: "" },
                    ]}
                    onSwitchVault={onSwitchVault}
                >
                    <div>shell-content</div>
                </AppShell>
            </MemoryRouter>,
        );

        await user.click(screen.getByRole("button", { name: "C:\\vaults\\main" }));
        await user.click(screen.getByRole("menuitem", { name: "C:\\vaults\\other" }));

        expect(onSwitchVault).toHaveBeenCalledWith("C:\\vaults\\other");
        expect(screen.queryByRole("menu")).toBeNull();
    });
});
