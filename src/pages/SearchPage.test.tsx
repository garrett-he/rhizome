import { invoke } from "@tauri-apps/api/core";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useSearchParams } from "react-router-dom";
import type { Mock } from "vitest";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SearchHit } from "@/types/document";
import { SearchPage } from "./SearchPage";

function DetailStub() {
    const [params] = useSearchParams();
    return <div>detail-chunk:{params.get("chunk")}</div>;
}

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke) as unknown as Mock;

function hit(overrides: Partial<SearchHit> = {}): SearchHit {
    return {
        chunk_id: 1,
        document_id: "doc1",
        document_title: "微服务架构实践",
        chunk_label: "服务拆分",
        chunk_index: 2,
        snippet: "在拆分服务时，首先要识别业务边界……",
        tags: ["技术"],
        created_at: "2026-10-20T01:00:00Z",
        relevance: 0.92,
        ...overrides,
    };
}

function renderPage() {
    return render(
        <MemoryRouter initialEntries={["/search"]}>
            <Routes>
                <Route path="/search" element={<SearchPage />} />
                <Route path="/documents/:id" element={<DetailStub />} />
            </Routes>
        </MemoryRouter>,
    );
}

beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue([]);
});

describe("SearchPage", () => {
    it("shows initial empty hint", () => {
        renderPage();
        expect(screen.getByText("输入关键词搜索知识库")).toBeTruthy();
    });

    it("rejects empty query without invoking", async () => {
        const user = userEvent.setup();
        renderPage();

        await user.click(screen.getByRole("button", { name: "搜索" }));

        expect(await screen.findByText("请输入搜索关键词")).toBeTruthy();
        expect(invokeMock).not.toHaveBeenCalled();
    });

    it("renders results after searching", async () => {
        const user = userEvent.setup();
        invokeMock.mockResolvedValue([
            hit(),
            hit({
                chunk_id: 2,
                document_id: "doc2",
                document_title: "架构演进之路",
                chunk_index: 3,
                chunk_label: "单体到微服务",
                relevance: 0.85,
                tags: ["工作"],
                snippet: "单体应用逐步演进为微服务……",
                created_at: "2026-10-12T01:00:00Z",
            }),
        ]);
        renderPage();

        await user.type(screen.getByRole("searchbox", { name: "搜索关键词" }), "服务拆分 业务边界");
        await user.click(screen.getByRole("button", { name: "搜索" }));

        expect(await screen.findByText("微服务架构实践")).toBeTruthy();
        expect(screen.getByText("架构演进之路")).toBeTruthy();
        expect(screen.getByText("相关度 0.92")).toBeTruthy();
        expect(screen.getByText("相关度 0.85")).toBeTruthy();
        expect(screen.getByText("02 · 服务拆分")).toBeTruthy();
        expect(screen.getByText(/首先要识别业务边界/)).toBeTruthy();
        expect(screen.getByText(/技术 · 10-20 · chunk 02/)).toBeTruthy();
        expect(screen.getByText(/共 2 条结果/)).toBeTruthy();
        expect(invokeMock).toHaveBeenCalledWith("search_documents", { query: "服务拆分 业务边界", limit: undefined });
    });

    it("shows not-found state when no hits", async () => {
        const user = userEvent.setup();
        renderPage();

        await user.type(screen.getByRole("searchbox", { name: "搜索关键词" }), "不存在的词");
        await user.click(screen.getByRole("button", { name: "搜索" }));

        expect(await screen.findByText("未找到相关内容")).toBeTruthy();
    });

    it("navigates to detail page with chunk param on expand", async () => {
        const user = userEvent.setup();
        invokeMock.mockResolvedValue([hit()]);
        renderPage();

        await user.type(screen.getByRole("searchbox", { name: "搜索关键词" }), "服务拆分");
        await user.click(await screen.findByRole("button", { name: "搜索" }));
        await screen.findByText("微服务架构实践");
        await user.click(screen.getByRole("button", { name: "展开全文 →" }));

        await waitFor(() => {
            expect(screen.getByText("detail-chunk:1")).toBeTruthy();
        });
    });

    it("shows inline error when search fails", async () => {
        const user = userEvent.setup();
        invokeMock.mockRejectedValue("打开数据库失败");
        renderPage();

        await user.type(screen.getByRole("searchbox", { name: "搜索关键词" }), "服务拆分");
        await user.click(screen.getByRole("button", { name: "搜索" }));

        expect(await screen.findByRole("alert")).toHaveTextContent("打开数据库失败");
    });
});
