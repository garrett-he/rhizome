import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { AppShell } from "./AppShell";

function renderShell() {
    return render(
        <AppShell vaultPath={"C:\\vaults\\main"}>
            <div>overview-content</div>
        </AppShell>,
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

    it("enables only the active nav item", () => {
        renderShell();

        expect(screen.getByRole("button", { name: "总览" })).not.toBeDisabled();
        expect(screen.getByRole("button", { name: "文档库" })).toBeDisabled();
        expect(screen.getByRole("button", { name: "搜索" })).toBeDisabled();
        expect(screen.getByRole("button", { name: "问答" })).toBeDisabled();
        expect(screen.getByRole("button", { name: "分类管理" })).toBeDisabled();
        expect(screen.getByRole("button", { name: "任务中心" })).toBeDisabled();
        expect(screen.getByRole("button", { name: "设置" })).toBeDisabled();
    });
});
