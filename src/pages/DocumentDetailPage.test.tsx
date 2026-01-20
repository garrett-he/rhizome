import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import type { Mock } from "vitest";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ChunkInfo, DocumentDetail } from "@/types/document";
import { DocumentDetailPage } from "./DocumentDetailPage";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn() }));

const invokeMock = vi.mocked(invoke) as unknown as Mock;
const revealMock = vi.mocked(revealItemInDir) as unknown as Mock;

const COMPILED =
    "# 微服务架构实践\n\n<!-- chunk: 服务拆分 -->\n第一段内容。\n\n<!-- chunk: 数据一致性 -->\n第二段内容。";

const METADATA = JSON.stringify({
    title: "微服务架构实践",
    tags: ["架构", "微服务"],
    language: "zh",
    summary: "本文探讨微服务拆分的原则与权衡。",
});

function detail(overrides: Partial<DocumentDetail> = {}): DocumentDetail {
    return {
        id: "doc1",
        file_name: "notes.md",
        mime_type: "text/markdown",
        type: "plaintext",
        sha1: "a3f9e2b81c77d4e0f1a2b3c4d5e6f7a8b9c0c41d",
        size: 2202009,
        status: "indexed",
        content: "# 原始全文",
        content_compiled: COMPILED,
        metadata_compiled: METADATA,
        created_at: "2026-10-20T01:00:00Z",
        updated_at: "2026-10-22T01:00:00Z",
        compiled_at: "2026-10-21T01:00:00Z",
        llm_model: "gpt-5",
        indexed_at: "2026-10-22T01:00:00Z",
        chunk_count: 2,
        embedding_model: "BAAI/bge-small-zh-v1.5",
        last_error: null,
        category_id: null,
        category_name: null,
        blob_path: "C:\\vault\\blobs\\a3\\f9\\a3f9e2",
        ...overrides,
    };
}

function chunk(overrides: Partial<ChunkInfo> = {}): ChunkInfo {
    return { id: 1, label: "服务拆分", text: "第一段内容。", ...overrides };
}

function renderPage(initialEntry = "/documents/doc1") {
    return render(
        <MemoryRouter initialEntries={[initialEntry]}>
            <Routes>
                <Route path="/documents/:id" element={<DocumentDetailPage />} />
            </Routes>
        </MemoryRouter>,
    );
}

function mockHappyPath(
    overrides: Partial<DocumentDetail> = {},
    chunks: ChunkInfo[] = [chunk(), chunk({ id: 2, label: "数据一致性", text: "第二段内容。" })],
) {
    invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "get_document") {
            return detail(overrides);
        }
        if (cmd === "list_chunks") {
            return chunks;
        }
        if (cmd === "list_categories") {
            return [
                { id: "cat-a", name: "技术笔记", document_count: 1 },
                { id: "cat-b", name: "会议记录", document_count: 0 },
            ];
        }
        if (cmd === "compile_documents") {
            return [{ document_id: "doc1", title: "t", tags: null, ok: true, error: null }];
        }
        if (cmd === "index_documents") {
            return [{ document_id: "doc1", title: "t", chunks: 2, ok: true, error: null }];
        }
        throw new Error(`unexpected command: ${cmd}`);
    });
}

beforeEach(() => {
    invokeMock.mockReset();
    revealMock.mockReset();
    revealMock.mockResolvedValue(undefined);
    // jsdom 未实现平滑滚动
    Element.prototype.scrollIntoView = vi.fn();
    vi.spyOn(window, "confirm").mockReturnValue(true);
});

describe("DocumentDetailPage", () => {
    it("renders header, meta row, metadata card and chunk list", async () => {
        mockHappyPath();
        renderPage();

        // 页面标题 + 渲染视图 H1 同名，共 2 处
        const titles = await screen.findAllByText("微服务架构实践");
        expect(titles.length).toBeGreaterThanOrEqual(2);
        expect(
            screen.getByText(/2.1 MB · sha1 a3f9e2…c41d · 导入于 10-20 · 2 chunks · BAAI\/bge-small-zh-v1\.5/),
        ).toBeTruthy();
        expect(screen.getByText("架构")).toBeTruthy();
        expect(screen.getByText("微服务")).toBeTruthy();
        expect(screen.getByText(/本文探讨微服务拆分的原则与权衡/)).toBeTruthy();
        expect(screen.getByText("切片（2）")).toBeTruthy();
        expect(screen.getByText("01 · 服务拆分")).toBeTruthy();
        expect(screen.getByText("02 · 数据一致性")).toBeTruthy();
        // 渲染视图含编译正文（导语标题保留）
        expect(screen.getByText("第一段内容。")).toBeTruthy();
    });

    it("switches to raw view showing original content", async () => {
        mockHappyPath();
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("01 · 服务拆分");

        await user.click(screen.getByRole("tab", { name: "原始文本" }));

        expect(screen.getByText("# 原始全文")).toBeTruthy();
        expect(screen.queryByText("第一段内容。")).toBeNull();
    });

    it("highlights the matching section when a chunk is clicked", async () => {
        mockHappyPath();
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("01 · 服务拆分");

        await user.click(screen.getByRole("button", { name: /02 · 数据一致性/ }));

        expect(Element.prototype.scrollIntoView).toHaveBeenCalled();
        const target = screen.getByText("第二段内容。").closest("section");
        expect(target?.className).toContain("bg-accent");
    });

    it("shows alert and retry button when last_error is present", async () => {
        mockHappyPath(
            {
                status: "raw",
                content_compiled: null,
                chunk_count: null,
                embedding_model: null,
                last_error: "compile: LLM 连接超时",
            },
            [],
        );
        const user = userEvent.setup();
        renderPage();

        expect(await screen.findByText("处理失败")).toBeTruthy();
        expect(screen.getByText(/compile: LLM 连接超时/)).toBeTruthy();

        await user.click(screen.getByRole("button", { name: "重试" }));
        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("compile_documents", { documentIds: ["doc1"], force: false });
        });
        expect(invokeMock).toHaveBeenCalledWith("index_documents", { documentIds: ["doc1"] });
    });

    it("recompiles with force after confirmation", async () => {
        mockHappyPath();
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("01 · 服务拆分");

        await user.click(screen.getByRole("button", { name: "重新编译" }));

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("compile_documents", { documentIds: ["doc1"], force: true });
        });
        expect(window.confirm).toHaveBeenCalled();
    });

    it("does not recompile when confirmation is dismissed", async () => {
        mockHappyPath();
        vi.spyOn(window, "confirm").mockReturnValue(false);
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("01 · 服务拆分");

        await user.click(screen.getByRole("button", { name: "重新编译" }));

        expect(invokeMock).not.toHaveBeenCalledWith("compile_documents", expect.anything());
    });

    it("reindexes the document", async () => {
        mockHappyPath();
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("01 · 服务拆分");

        await user.click(screen.getByRole("button", { name: "重新索引" }));

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("index_documents", { documentIds: ["doc1"] });
        });
    });

    it("reveals the blob file in file manager", async () => {
        mockHappyPath();
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("01 · 服务拆分");

        await user.click(screen.getByRole("button", { name: "打开原始文件" }));

        await waitFor(() => {
            expect(revealMock).toHaveBeenCalledWith("C:\\vault\\blobs\\a3\\f9\\a3f9e2");
        });
    });

    it("shows error state with back link when document is missing", async () => {
        invokeMock.mockRejectedValue("文档不存在: doc1");
        renderPage();

        expect(await screen.findByRole("alert")).toHaveTextContent("文档不存在: doc1");
        expect(screen.getByText("返回文档库")).toBeTruthy();
    });

    it("shows reindex hint for compiled-but-unindexed document", async () => {
        mockHappyPath({ status: "compiled", content_compiled: COMPILED, chunk_count: null, embedding_model: null }, []);
        renderPage();

        expect(await screen.findByText(/尚未索引，点击「重新索引」生成切片/)).toBeTruthy();
    });

    it("selects and scrolls to chunk from ?chunk= param", async () => {
        mockHappyPath();
        const scrollSpy = vi.spyOn(Element.prototype, "scrollIntoView");
        renderPage("/documents/doc1?chunk=2");

        // 对应切片被初始选中并滚动定位
        await waitFor(() => {
            expect(scrollSpy).toHaveBeenCalled();
        });
        const target = screen.getByText("第二段内容。").closest("section");
        expect(target?.className).toContain("bg-accent");
    });

    it("ignores unknown ?chunk= param", async () => {
        mockHappyPath();
        renderPage("/documents/doc1?chunk=999");

        await screen.findByText("01 · 服务拆分");
        const target = screen.getByText("第一段内容。").closest("section");
        expect(target?.className).not.toContain("bg-accent");
    });

    it("changes document category via the select", async () => {
        mockHappyPath({ category_id: null, category_name: null });
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("01 · 服务拆分");

        const select = screen.getByLabelText("所属分类") as HTMLSelectElement;
        expect(select.value).toBe("");

        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "set_document_category") {
                return null;
            }
            if (cmd === "get_document") {
                return detail({ category_id: "cat-a", category_name: "技术笔记" });
            }
            if (cmd === "list_chunks") {
                return [chunk(), chunk({ id: 2, label: "数据一致性", text: "第二段内容。" })];
            }
            if (cmd === "list_categories") {
                return [
                    { id: "cat-a", name: "技术笔记", document_count: 1 },
                    { id: "cat-b", name: "会议记录", document_count: 0 },
                ];
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        await user.selectOptions(select, "cat-a");

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("set_document_category", {
                documentId: "doc1",
                categoryId: "cat-a",
            });
        });
        // 刷新后选中值同步为新分类
        await waitFor(() => {
            expect((screen.getByLabelText("所属分类") as HTMLSelectElement).value).toBe("cat-a");
        });
    });

    it("clears document category by selecting the placeholder option", async () => {
        mockHappyPath({ category_id: "cat-a", category_name: "技术笔记" });
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("01 · 服务拆分");

        const select = screen.getByLabelText("所属分类") as HTMLSelectElement;
        expect(select.value).toBe("cat-a");

        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "set_document_category") {
                return null;
            }
            if (cmd === "get_document") {
                return detail({ category_id: null, category_name: null });
            }
            if (cmd === "list_chunks") {
                return [chunk(), chunk({ id: 2, label: "数据一致性", text: "第二段内容。" })];
            }
            if (cmd === "list_categories") {
                return [{ id: "cat-a", name: "技术笔记", document_count: 1 }];
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        await user.selectOptions(select, "");

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("set_document_category", { documentId: "doc1", categoryId: null });
        });
    });
});
