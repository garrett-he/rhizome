import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Mock } from "vitest";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DocumentSummary } from "@/types/document";
import { DocumentLibraryPage } from "./DocumentLibraryPage";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

const invokeMock = vi.mocked(invoke) as unknown as Mock;
const openMock = vi.mocked(open) as unknown as Mock;

function doc(overrides: Partial<DocumentSummary>): DocumentSummary {
    return {
        id: "doc1",
        file_name: "notes.md",
        mime_type: "text/markdown",
        type: "plaintext",
        status: "raw",
        size: 2202009,
        created_at: "2026-10-22T01:00:00Z",
        ...overrides,
    };
}

beforeEach(() => {
    invokeMock.mockReset();
    openMock.mockReset();
    invokeMock.mockResolvedValue([]);
});

describe("DocumentLibraryPage", () => {
    it("renders empty state when no documents", async () => {
        render(<DocumentLibraryPage />);

        expect(await screen.findByText("暂无文档，点击右上角导入文档添加")).toBeTruthy();
    });

    it("renders document rows with formatted columns", async () => {
        invokeMock.mockResolvedValue([
            doc({ file_name: "notes.md" }),
            doc({ id: "doc2", file_name: "sub.srt", mime_type: "application/x-subrip" }),
        ]);
        render(<DocumentLibraryPage />);

        expect(await screen.findByText("notes.md")).toBeTruthy();
        expect(screen.getByText("sub.srt")).toBeTruthy();
        expect(screen.getAllByText("文本").length).toBe(2);
        expect(screen.getAllByText("已导入").length).toBe(2);
        expect(screen.getAllByText("2.1 MB").length).toBe(2);
        expect(screen.getAllByText("10-22").length).toBe(2);
    });

    it("shows summary after importing files", async () => {
        const user = userEvent.setup();
        render(<DocumentLibraryPage />);
        await screen.findByText("暂无文档，点击右上角导入文档添加");

        openMock.mockResolvedValue(["C:\\docs\\a.md", "C:\\docs\\b.png"]);
        invokeMock.mockImplementation(async (cmd: string, args?: { paths?: string[] }) => {
            if (cmd === "add_documents") {
                return {
                    added: 1,
                    skipped: 0,
                    failures: [{ path: args?.paths?.[1] ?? "", error: "不支持的文件类型" }],
                };
            }
            return [doc({ file_name: "a.md" })];
        });
        await user.click(screen.getByRole("button", { name: "导入文档" }));

        expect(await screen.findByText(/新增 1 · 跳过 0 · 失败 1/)).toBeTruthy();
        expect(screen.getByText(/不支持的文件类型/)).toBeTruthy();
        expect(screen.getByText("a.md")).toBeTruthy();
        expect(openMock).toHaveBeenCalledWith({
            multiple: true,
            filters: [{ name: "纯文本", extensions: ["md", "txt", "srt"] }],
        });
    });

    it("cancelling the dialog imports nothing", async () => {
        const user = userEvent.setup();
        render(<DocumentLibraryPage />);
        await screen.findByText("暂无文档，点击右上角导入文档添加");

        openMock.mockResolvedValue(null);
        await user.click(screen.getByRole("button", { name: "导入文档" }));

        await waitFor(() => {
            expect(openMock).toHaveBeenCalled();
        });
        expect(invokeMock).not.toHaveBeenCalledWith("add_documents", expect.anything());
        expect(screen.queryByText(/新增/)).toBeNull();
    });

    it("shows inline error when listing fails", async () => {
        invokeMock.mockRejectedValue("未选择知识库");
        render(<DocumentLibraryPage />);

        expect(await screen.findByRole("alert")).toHaveTextContent("未选择知识库");
    });
});
