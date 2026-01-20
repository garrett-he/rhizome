import { invoke } from "@tauri-apps/api/core";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import type { Mock } from "vitest";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ConversationSummary } from "@/types/qa";
import { QaPage } from "./QaPage";

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

const CONVERSATION: ConversationSummary = {
    id: "conv1",
    title: "服务拆分实践",
    updated_at: new Date().toISOString(),
};

function renderPage() {
    return render(
        <MemoryRouter>
            <QaPage />
        </MemoryRouter>,
    );
}

beforeEach(() => {
    invokeMock.mockReset();
    eventHandlers.clear();
    invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "list_conversations") {
            return [CONVERSATION];
        }
        if (cmd === "list_messages") {
            return [];
        }
        if (cmd === "qa_ask") {
            return { conversation_id: "conv1", sources: [] };
        }
        throw new Error(`unexpected command: ${cmd}`);
    });
});

describe("QaPage", () => {
    it("shows initial empty state and history column", async () => {
        renderPage();

        expect(await screen.findByText("输入问题，从知识库中寻找答案")).toBeTruthy();
        expect(screen.getByText("历史对话")).toBeTruthy();
        expect(screen.getByText("服务拆分实践")).toBeTruthy();
        expect(screen.getByPlaceholderText("输入问题…（检索范围：全部知识库）")).toBeTruthy();
    });

    it("streams assistant reply via qa-delta events and finalizes on qa-done", async () => {
        const user = userEvent.setup();
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "qa_ask") {
                // 模拟后端：命令 resolve 前先推送事件
                emit("qa-start", { conversation_id: "conv1" });
                emit("qa-delta", { conversation_id: "conv1", text: "服务拆分要" });
                emit("qa-delta", { conversation_id: "conv1", text: "注意业务边界。" });
                emit("qa-done", {
                    conversation_id: "conv1",
                    message_id: 7,
                    sources: [{ chunk_id: 3, document_id: "doc1", title: "微服务架构实践", chunk_index: 2 }],
                });
                return { conversation_id: "conv1", sources: [] };
            }
            if (cmd === "list_conversations") {
                return [CONVERSATION];
            }
            if (cmd === "list_messages") {
                return [];
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        renderPage();
        await screen.findByText("输入问题，从知识库中寻找答案");

        await user.type(screen.getByLabelText("问题"), "服务拆分要注意什么？");
        await user.click(screen.getByRole("button", { name: "发送" }));

        // 用户气泡
        expect(screen.getByText("服务拆分要注意什么？")).toBeTruthy();
        // 两个 delta 拼接后的完整回答（qa-done 固化）
        expect(await screen.findByText(/服务拆分要注意业务边界。/)).toBeTruthy();
        // 引用来源 chip
        expect(screen.getByText(/微服务架构实践/)).toBeTruthy();
        expect(invokeMock).toHaveBeenCalledWith("qa_ask", {
            question: "服务拆分要注意什么？",
            conversationId: undefined,
        });
    });

    it("does not send empty question", async () => {
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("输入问题，从知识库中寻找答案");

        const send = screen.getByRole("button", { name: "发送" });
        expect(send).toBeDisabled();
        await user.click(send);
        expect(invokeMock).not.toHaveBeenCalledWith("qa_ask", expect.anything());
    });

    it("shows error bubble on qa-error event", async () => {
        const user = userEvent.setup();
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "qa_ask") {
                emit("qa-start", { conversation_id: "conv1" });
                emit("qa-delta", { conversation_id: "conv1", text: "部分回" });
                emit("qa-error", { conversation_id: "conv1", message: "LLM 返回 429" });
                throw new Error("LLM 返回 429");
            }
            if (cmd === "list_conversations") {
                return [CONVERSATION];
            }
            if (cmd === "list_messages") {
                return [];
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        renderPage();
        await screen.findByText("输入问题，从知识库中寻找答案");

        await user.type(screen.getByLabelText("问题"), "问题");
        await user.click(screen.getByRole("button", { name: "发送" }));

        expect(await screen.findByText("LLM 返回 429")).toBeTruthy();
        // 流式占位气泡被移除
        expect(screen.queryByText(/部分回/)).toBeNull();
    });

    it("shows error bubble when qa_ask rejects without qa-error event", async () => {
        const user = userEvent.setup();
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "qa_ask") {
                throw new Error("连接后端失败");
            }
            if (cmd === "list_conversations") {
                return [CONVERSATION];
            }
            if (cmd === "list_messages") {
                return [];
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        renderPage();
        await screen.findByText("输入问题，从知识库中寻找答案");

        await user.type(screen.getByLabelText("问题"), "问题");
        await user.click(screen.getByRole("button", { name: "发送" }));

        expect(await screen.findByText("连接后端失败")).toBeTruthy();
    });

    it("switches conversation from history and loads messages", async () => {
        const user = userEvent.setup();
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "list_conversations") {
                return [CONVERSATION];
            }
            if (cmd === "list_messages") {
                return [
                    { id: 1, role: "user", content: "之前的问题", sources: [], created_at: "2026-10-22T01:00:00Z" },
                    {
                        id: 2,
                        role: "assistant",
                        content: "之前的回答",
                        sources: [],
                        created_at: "2026-10-22T01:00:01Z",
                    },
                ];
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        renderPage();
        await screen.findByText("输入问题，从知识库中寻找答案");

        await user.click(screen.getByText("服务拆分实践"));

        expect(await screen.findByText("之前的问题")).toBeTruthy();
        expect(screen.getByText("之前的回答")).toBeTruthy();
        expect(screen.queryByText("输入问题，从知识库中寻找答案")).toBeNull();
    });

    it("starts a new conversation clearing the view", async () => {
        const user = userEvent.setup();
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "list_conversations") {
                return [CONVERSATION];
            }
            if (cmd === "list_messages") {
                return [
                    { id: 1, role: "user", content: "之前的问题", sources: [], created_at: "2026-10-22T01:00:00Z" },
                ];
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        renderPage();
        await user.click(await screen.findByText("服务拆分实践"));
        await screen.findByText("之前的问题");

        await user.click(screen.getByRole("button", { name: /新会话/ }));

        expect(await screen.findByText("输入问题，从知识库中寻找答案")).toBeTruthy();
        expect(screen.queryByText("之前的问题")).toBeNull();
    });
});
