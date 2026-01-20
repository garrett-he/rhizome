import { invoke } from "@tauri-apps/api/core";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import type { Mock } from "vitest";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DocumentSummary } from "@/types/document";
import type { JobRecord } from "@/types/task";
import { TasksPage } from "./TasksPage";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

// 受控 listen：测试通过 handlers 触发事件回调
type Handler = (event: { payload: Record<string, unknown> }) => void;
const eventHandlers = new Map<string, Handler>();
const listenMock = vi.fn(async (event: string, handler: Handler) => {
    eventHandlers.set(event, handler);
    return () => eventHandlers.delete(event);
});

vi.mock("@tauri-apps/api/event", () => ({
    listen: (...args: unknown[]) => listenMock(...(args as [string, Handler])),
}));

const invokeMock = vi.mocked(invoke) as unknown as Mock;

function emit(event: string, payload: Record<string, unknown>) {
    eventHandlers.get(event)?.({ payload });
}

/** 按文件名找到所在行的 checkbox（label 内含链接与错误信息） */
function checkboxFor(fileName: string): HTMLElement {
    const label = screen.getByText(fileName).closest("label");
    if (label === null) {
        throw new Error(`no label row for ${fileName}`);
    }
    return within(label).getByRole("checkbox");
}

function doc(overrides: Partial<DocumentSummary>): DocumentSummary {
    return {
        id: "doc-raw",
        file_name: "raw.md",
        mime_type: "text/markdown",
        type: "plaintext",
        status: "raw",
        size: 1024,
        created_at: "2026-10-22T01:00:00Z",
        last_error: null,
        category_id: null,
        category_name: null,
        ...overrides,
    };
}

const DOCS: DocumentSummary[] = [
    doc({ id: "doc-raw", file_name: "raw.md", status: "raw" }),
    doc({ id: "doc-compiled", file_name: "compiled.md", status: "compiled" }),
    doc({ id: "doc-failed", file_name: "failed.md", status: "raw", last_error: "compile: LLM 连接超时" }),
];

const JOBS: JobRecord[] = [
    {
        id: "job1",
        kind: "all",
        total: 3,
        succeeded: 2,
        failed: 1,
        status: "completed",
        started_at: "2026-10-22T09:00:00Z",
        finished_at: "2026-10-22T09:01:00Z",
    },
];

function renderPage() {
    return render(
        <MemoryRouter>
            <TasksPage />
        </MemoryRouter>,
    );
}

function mockDefault() {
    invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "list_documents") {
            return DOCS;
        }
        if (cmd === "list_jobs") {
            return JOBS;
        }
        throw new Error(`unexpected command: ${cmd}`);
    });
}

beforeEach(() => {
    invokeMock.mockReset();
    eventHandlers.clear();
    mockDefault();
});

describe("TasksPage", () => {
    it("renders three task groups and history", async () => {
        renderPage();

        expect(await screen.findByText("待编译")).toBeTruthy();
        expect(screen.getByText("待索引")).toBeTruthy();
        expect(screen.getByText("失败")).toBeTruthy();
        expect(screen.getByText("raw.md")).toBeTruthy();
        expect(screen.getByText("compile: LLM 连接超时")).toBeTruthy();
        expect(screen.getByText(/共 3 篇 · 成功 2 · 失败 1/)).toBeTruthy();
    });

    it("runs all documents via the header button with progress events", async () => {
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("raw.md");

        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "run_tasks") {
                emit("task-start", { job_id: "job9", kind: "all", total: 3 });
                emit("task-item", { job_id: "job9", document_id: "doc-raw", step: "compile", ok: true });
                emit("task-item", { job_id: "job9", document_id: "doc-raw", step: "index", ok: true });
                emit("task-done", { job_id: "job9", succeeded: 3, failed: 0 });
                return { job_id: "job9", kind: "all", total: 3, succeeded: 3, failed: 0 };
            }
            if (cmd === "list_documents") {
                return DOCS.map((d) => ({ ...d, status: "indexed" }));
            }
            if (cmd === "list_jobs") {
                return [{ ...JOBS[0], id: "job9", succeeded: 3, failed: 0 }];
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        await user.click(screen.getByRole("button", { name: "一键处理全部" }));

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("run_tasks", { documentIds: [], step: "all" });
        });
        // 完成后显示成功汇总（进度卡与历史行都可能出现）
        expect((await screen.findAllByText(/成功 3 · 失败 0/)).length).toBeGreaterThanOrEqual(1);
    });

    it("runs compile for selected pending documents only", async () => {
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("raw.md");

        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "run_tasks") {
                return { job_id: "job2", kind: "compile", total: 1, succeeded: 1, failed: 0 };
            }
            if (cmd === "list_documents") {
                return DOCS;
            }
            if (cmd === "list_jobs") {
                return JOBS;
            }
            throw new Error(`unexpected command: ${cmd}`);
        });

        await user.click(checkboxFor("raw.md"));
        const compileButtons = screen.getAllByRole("button", { name: "编译所选" });
        await user.click(compileButtons[0]);

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("run_tasks", { documentIds: ["doc-raw"], step: "compile" });
        });
    });

    it("retries selected failed documents with all step", async () => {
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("failed.md");

        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "run_tasks") {
                return { job_id: "job3", kind: "all", total: 1, succeeded: 1, failed: 0 };
            }
            if (cmd === "list_documents") {
                return DOCS;
            }
            if (cmd === "list_jobs") {
                return JOBS;
            }
            throw new Error(`unexpected command: ${cmd}`);
        });

        await user.click(checkboxFor("failed.md"));
        await user.click(screen.getByRole("button", { name: "重试所选" }));

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("run_tasks", { documentIds: ["doc-failed"], step: "all" });
        });
    });

    it("disables run buttons while a batch is running", async () => {
        const user = userEvent.setup();
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "run_tasks") {
                // 挂起直到测试结束：模拟长时间运行
                await new Promise(() => {});
                return { job_id: "job4", kind: "all", total: 0, succeeded: 0, failed: 0 };
            }
            if (cmd === "list_documents") {
                return DOCS;
            }
            if (cmd === "list_jobs") {
                return JOBS;
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        renderPage();
        await screen.findByText("raw.md");

        const runAll = screen.getByRole("button", { name: "一键处理全部" });
        await user.click(runAll);

        await waitFor(() => {
            expect(runAll).toBeDisabled();
        });
        expect(screen.getByRole("button", { name: "刷新" })).toBeDisabled();
    });

    it("shows command error when LLM is not configured", async () => {
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("raw.md");

        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "run_tasks") {
                throw "LLM 未配置";
            }
            if (cmd === "list_documents") {
                return DOCS;
            }
            if (cmd === "list_jobs") {
                return JOBS;
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        await user.click(screen.getByRole("button", { name: "一键处理全部" }));

        expect(await screen.findByText("LLM 未配置")).toBeTruthy();
    });

    it("shows failed items in the progress card and ignores foreign job events", async () => {
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("raw.md");

        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "run_tasks") {
                // 异 job_id 的事件应被忽略
                emit("task-item", {
                    job_id: "other",
                    document_id: "doc-x",
                    step: "compile",
                    ok: false,
                    error: "无关错误",
                });
                emit("task-start", { job_id: "job5", kind: "all", total: 1 });
                emit("task-item", { job_id: "job5", document_id: "doc-raw", step: "compile", ok: true });
                emit("task-item", {
                    job_id: "job5",
                    document_id: "unknown-id",
                    step: "index",
                    ok: false,
                    error: "embedding 模型不可用",
                });
                emit("task-done", { job_id: "job5", succeeded: 0, failed: 1 });
                return { job_id: "job5", kind: "all", total: 1, succeeded: 0, failed: 1 };
            }
            if (cmd === "list_documents") {
                return DOCS;
            }
            if (cmd === "list_jobs") {
                return JOBS;
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        await user.click(screen.getByRole("button", { name: "一键处理全部" }));

        // 成功项展示文件名（清单与进度卡各一处），失败项展示未知 id 回退与错误信息
        expect((await screen.findAllByText("raw.md")).length).toBeGreaterThanOrEqual(2);
        expect(screen.getByText("unknown-")).toBeTruthy();
        expect(screen.getByText("embedding 模型不可用")).toBeTruthy();
        expect(screen.queryByText("无关错误")).toBeNull();
    });

    it("shows running badge for unfinished history jobs", async () => {
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "list_documents") {
                return DOCS;
            }
            if (cmd === "list_jobs") {
                return [{ ...JOBS[0], status: "running", finished_at: null }];
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        renderPage();

        expect(await screen.findByText("进行中")).toBeTruthy();
    });

    it("shows error when loading documents fails", async () => {
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "list_documents") {
                throw "未选择知识库";
            }
            if (cmd === "list_jobs") {
                return [];
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        renderPage();

        expect(await screen.findByRole("alert")).toHaveTextContent("未选择知识库");
    });
});
