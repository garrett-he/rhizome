import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
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
        last_error: null,
        category_id: null,
        category_name: null,
        ...overrides,
    };
}

const PROCESS_SUMMARY = { compiled: 1, compile_failures: 0, indexed: 1, index_failures: 0 };

beforeEach(() => {
    invokeMock.mockReset();
    openMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "list_categories") {
            return [];
        }
        return [];
    });
});

describe("DocumentLibraryPage", () => {
    it("renders empty state when no documents", async () => {
        render(
            <MemoryRouter>
                <DocumentLibraryPage />
            </MemoryRouter>,
        );

        expect(await screen.findByText("暂无文档，点击右上角导入文档添加")).toBeTruthy();
    });

    it("renders document rows with formatted columns", async () => {
        invokeMock.mockResolvedValue([
            doc({ file_name: "notes.md" }),
            doc({ id: "doc2", file_name: "sub.srt", mime_type: "application/x-subrip" }),
        ]);
        render(
            <MemoryRouter>
                <DocumentLibraryPage />
            </MemoryRouter>,
        );

        expect(await screen.findByText("notes.md")).toBeTruthy();
        expect(screen.getByText("sub.srt")).toBeTruthy();
        expect(screen.getAllByText("文本").length).toBe(2);
        expect(screen.getAllByText("已导入").length).toBe(2);
        expect(screen.getAllByText("2.1 MB").length).toBe(2);
        expect(screen.getAllByText("10-22").length).toBe(2);
    });

    it("shows summary after importing files", async () => {
        const user = userEvent.setup();
        render(
            <MemoryRouter>
                <DocumentLibraryPage />
            </MemoryRouter>,
        );
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
            if (cmd === "process_documents") {
                return PROCESS_SUMMARY;
            }
            return [doc({ file_name: "a.md" })];
        });
        await user.click(screen.getByRole("button", { name: "导入文档" }));

        expect(await screen.findByText(/新增 1 · 跳过 0 · 失败 1/)).toBeTruthy();
        expect(screen.getByText(/自动处理完成：编译 1 · 索引 1/)).toBeTruthy();
        expect(screen.getByText("a.md")).toBeTruthy();
        expect(openMock).toHaveBeenCalledWith({
            multiple: true,
            filters: expect.arrayContaining([{ name: "纯文本", extensions: ["md", "txt", "srt"] }]),
        });
    });

    it("silently skips auto processing when LLM is not configured", async () => {
        const user = userEvent.setup();
        render(
            <MemoryRouter>
                <DocumentLibraryPage />
            </MemoryRouter>,
        );
        await screen.findByText("暂无文档，点击右上角导入文档添加");

        openMock.mockResolvedValue(["C:\\docs\\a.md"]);
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "add_documents") {
                return { added: 1, skipped: 0, failures: [] };
            }
            if (cmd === "process_documents") {
                throw new Error("LLM 未配置");
            }
            return [doc({ file_name: "a.md" })];
        });
        await user.click(screen.getByRole("button", { name: "导入文档" }));

        expect(await screen.findByText(/新增 1 · 跳过 0 · 失败 0/)).toBeTruthy();
        expect(screen.queryByText(/自动处理完成/)).toBeNull();
        expect(screen.queryByRole("alert")).toBeNull();
    });

    it("cancelling the dialog imports nothing", async () => {
        const user = userEvent.setup();
        render(
            <MemoryRouter>
                <DocumentLibraryPage />
            </MemoryRouter>,
        );
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
        render(
            <MemoryRouter>
                <DocumentLibraryPage />
            </MemoryRouter>,
        );

        expect(await screen.findByRole("alert")).toHaveTextContent("未选择知识库");
    });

    it("marks failed documents and shows last_error", async () => {
        invokeMock.mockResolvedValue([doc({ status: "raw", last_error: "compile: 网络错误" })]);
        render(
            <MemoryRouter>
                <DocumentLibraryPage />
            </MemoryRouter>,
        );

        expect(await screen.findByText(/compile: 网络错误/)).toBeTruthy();
        // 失败行提供重试按钮
        expect(screen.getByRole("button", { name: /重试/ })).toBeTruthy();
    });

    it("retries a single document via compile + index", async () => {
        const user = userEvent.setup();
        invokeMock.mockResolvedValue([doc({ status: "raw" })]);
        render(
            <MemoryRouter>
                <DocumentLibraryPage />
            </MemoryRouter>,
        );
        await screen.findByText("notes.md");

        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "compile_documents") {
                return [{ document_id: "doc1", title: "t", tags: null, ok: true, error: null }];
            }
            if (cmd === "index_documents") {
                return [{ document_id: "doc1", title: "t", chunks: 3, ok: true, error: null }];
            }
            return [doc({ status: "indexed" })];
        });
        await user.click(screen.getByRole("button", { name: /重试/ }));

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("compile_documents", { documentIds: ["doc1"], force: false });
        });
        expect(invokeMock).toHaveBeenCalledWith("index_documents", { documentIds: ["doc1"] });
        expect(await screen.findByText("已索引")).toBeTruthy();
    });

    it("runs batch compile+index from header button", async () => {
        const user = userEvent.setup();
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "list_categories") {
                return [];
            }
            return [doc({ status: "raw" })];
        });
        render(
            <MemoryRouter>
                <DocumentLibraryPage />
            </MemoryRouter>,
        );
        await screen.findByText("notes.md");

        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "process_documents") {
                return PROCESS_SUMMARY;
            }
            if (cmd === "list_categories") {
                return [];
            }
            return [doc({ status: "indexed" })];
        });
        await user.click(screen.getByRole("button", { name: "编译并索引" }));

        expect(await screen.findByText(/处理完成：编译 1 · 索引 1/)).toBeTruthy();
        expect(invokeMock).toHaveBeenCalledWith("process_documents");
    });

    it("shows category badges and filters by category", async () => {
        const user = userEvent.setup();
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "list_categories") {
                return [{ id: "cat-a", name: "技术笔记", document_count: 1 }];
            }
            return [
                doc({ file_name: "a.md", category_id: "cat-a", category_name: "技术笔记" }),
                doc({ id: "doc2", file_name: "b.md" }),
            ];
        });
        render(
            <MemoryRouter>
                <DocumentLibraryPage />
            </MemoryRouter>,
        );
        expect(await screen.findByText("a.md")).toBeTruthy();
        // 分类名同时出现在筛选下拉与行徽标中
        expect(screen.getAllByText("技术笔记").length).toBe(2);
        expect(screen.getByText("未分类")).toBeTruthy();

        // 按分类筛选：只剩分类内文档
        await user.selectOptions(screen.getByLabelText("分类筛选"), "cat-a");
        expect(screen.getByText("a.md")).toBeTruthy();
        expect(screen.queryByText("b.md")).toBeNull();

        // 切回全部分类
        await user.selectOptions(screen.getByLabelText("分类筛选"), "");
        expect(screen.getByText("b.md")).toBeTruthy();
    });

    it("shows empty hint when the selected category has no documents", async () => {
        const user = userEvent.setup();
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "list_categories") {
                return [{ id: "cat-x", name: "空分类", document_count: 0 }];
            }
            return [doc({ file_name: "a.md" })];
        });
        render(
            <MemoryRouter>
                <DocumentLibraryPage />
            </MemoryRouter>,
        );
        await screen.findByText("a.md");

        await user.selectOptions(screen.getByLabelText("分类筛选"), "cat-x");

        expect(await screen.findByText("该分类下暂无文档")).toBeTruthy();
    });

    it("imports files into the selected category", async () => {
        const user = userEvent.setup();
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "list_categories") {
                return [{ id: "cat-a", name: "技术笔记", document_count: 0 }];
            }
            return [];
        });
        render(
            <MemoryRouter>
                <DocumentLibraryPage />
            </MemoryRouter>,
        );
        await screen.findByText("暂无文档，点击右上角导入文档添加");

        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "list_categories") {
                return [{ id: "cat-a", name: "技术笔记", document_count: 1 }];
            }
            if (cmd === "add_documents") {
                return { added: 1, skipped: 0, failures: [] };
            }
            if (cmd === "process_documents") {
                return PROCESS_SUMMARY;
            }
            return [doc({ file_name: "a.md", category_id: "cat-a", category_name: "技术笔记" })];
        });
        openMock.mockResolvedValue(["C:\\docs\\a.md"]);
        await user.selectOptions(screen.getByLabelText("导入分类"), "技术笔记");
        await user.click(screen.getByRole("button", { name: "导入文档" }));

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("add_documents", {
                paths: ["C:\\docs\\a.md"],
                categoryName: "技术笔记",
            });
        });
    });
});
