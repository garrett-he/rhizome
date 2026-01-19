import { invoke } from "@tauri-apps/api/core";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import type { Mock } from "vitest";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Overview } from "@/types/overview";
import { OverviewPage } from "./OverviewPage";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke) as unknown as Mock;

const sampleOverview: Overview = {
    total_documents: 12,
    indexed_documents: 3,
    pending_documents: 9,
    failed_documents: 0,
    storage_bytes: 2576980378,
    categories: [],
};

const emptyOverview: Overview = {
    total_documents: 0,
    indexed_documents: 0,
    pending_documents: 0,
    failed_documents: 0,
    storage_bytes: 0,
    categories: [],
};

beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(sampleOverview);
});

function renderPage() {
    return render(
        <MemoryRouter initialEntries={["/"]}>
            <Routes>
                <Route path="/" element={<OverviewPage />} />
                <Route path="/documents" element={<div>document-library</div>} />
            </Routes>
        </MemoryRouter>,
    );
}

describe("OverviewPage", () => {
    it("renders title, actions and metrics", async () => {
        renderPage();

        expect(await screen.findByText("知识库总览")).toBeTruthy();
        expect(screen.getByText("文档总数")).toBeTruthy();
        expect(screen.getByText("12")).toBeTruthy();
        expect(screen.getAllByText("已索引").length).toBeGreaterThan(0);
        expect(screen.getAllByText("待处理").length).toBeGreaterThan(0);
        expect(await screen.findByText("2.4 GB")).toBeTruthy();
        expect(screen.getByText("处理流水线")).toBeTruthy();
        expect(screen.queryByRole("button", { name: "导入文档" })).toBeNull();
    });

    it("renders empty states for pending and categories", async () => {
        invokeMock.mockResolvedValue(emptyOverview);
        renderPage();

        expect(await screen.findByText("暂无待处理文档")).toBeTruthy();
        expect(screen.getByText("暂无分类数据")).toBeTruthy();
    });

    it("refresh re-queries the overview", async () => {
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("知识库总览");

        await user.click(screen.getByRole("button", { name: "刷新" }));

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledTimes(2);
        });
    });

    it("shows inline error when the overview query fails", async () => {
        invokeMock.mockRejectedValue("未选择知识库");
        renderPage();

        expect(await screen.findByRole("alert")).toHaveTextContent("未选择知识库");
    });
});
