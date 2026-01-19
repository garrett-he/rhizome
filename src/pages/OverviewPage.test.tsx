import { invoke } from "@tauri-apps/api/core";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Mock } from "vitest";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Overview } from "@/types/overview";
import { OverviewPage } from "./OverviewPage";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke) as unknown as Mock;

const sampleOverview: Overview = {
    total_documents: 0,
    indexed_documents: 0,
    pending_documents: 0,
    failed_documents: 0,
    storage_bytes: 2576980378,
    categories: [],
};

beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(sampleOverview);
});

describe("OverviewPage", () => {
    it("renders title, actions and zero-value metrics", async () => {
        render(<OverviewPage />);

        expect(await screen.findByText("知识库总览")).toBeTruthy();
        expect(screen.getByText("文档总数")).toBeTruthy();
        expect(screen.getAllByText("已索引").length).toBeGreaterThan(0);
        expect(screen.getAllByText("待处理").length).toBeGreaterThan(0);
        expect(await screen.findByText("2.4 GB")).toBeTruthy();
        expect(screen.getByText("处理流水线")).toBeTruthy();
        expect(screen.getByRole("button", { name: "导入文档" })).toBeDisabled();
    });

    it("renders empty states for pending and categories", async () => {
        render(<OverviewPage />);

        expect(await screen.findByText("暂无待处理文档")).toBeTruthy();
        expect(screen.getByText("暂无分类数据")).toBeTruthy();
    });

    it("refresh re-queries the overview", async () => {
        const user = userEvent.setup();
        render(<OverviewPage />);
        await screen.findByText("知识库总览");

        await user.click(screen.getByRole("button", { name: "刷新" }));

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledTimes(2);
        });
    });

    it("shows inline error when the overview query fails", async () => {
        invokeMock.mockRejectedValue("未选择知识库");
        render(<OverviewPage />);

        expect(await screen.findByRole("alert")).toHaveTextContent("未选择知识库");
    });
});
