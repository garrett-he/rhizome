import { invoke } from "@tauri-apps/api/core";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { MemoryRouter } from "react-router-dom";
import type { Mock } from "vitest";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { confirmAction } from "@/lib/dialog";
import type { ConversationSummary } from "@/types/qa";
import { QaPage } from "./QaPage";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@/lib/dialog", () => ({ confirmAction: vi.fn() }));

// 受控 listen：同一事件可注册多个监听（用于验证重复绑定），测试通过 emit 触发回调
type Handler = (event: { payload: Record<string, unknown> }) => void;
const eventHandlers = new Map<string, Handler[]>();
const listenMock = vi.fn(async (event: string, handler: Handler) => {
    const handlers = eventHandlers.get(event) ?? [];
    handlers.push(handler);
    eventHandlers.set(event, handlers);
    return () => {
        const current = eventHandlers.get(event) ?? [];
        const index = current.indexOf(handler);
        if (index >= 0) {
            current.splice(index, 1);
        }
    };
});

vi.mock("@tauri-apps/api/event", () => ({
    listen: (...args: unknown[]) => listenMock(...(args as [string, Handler])),
}));

const invokeMock = vi.mocked(invoke) as unknown as Mock;
const confirmMock = vi.mocked(confirmAction);

function emit(event: string, payload: Record<string, unknown>) {
    for (const handler of eventHandlers.get(event) ?? []) {
        handler({ payload });
    }
}

const CONVERSATION: ConversationSummary = {
    id: "conv1",
    title: "服务拆分实践",
    updated_at: new Date().toISOString(),
};

const OTHER_CONVERSATION: ConversationSummary = {
    id: "conv2",
    title: "会议纪要",
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
    confirmMock.mockReset();
    confirmMock.mockResolvedValue(true);
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

    it("renders the assistant reply as markdown", async () => {
        const user = userEvent.setup();
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "qa_ask") {
                emit("qa-start", { conversation_id: "conv1" });
                emit("qa-delta", { conversation_id: "conv1", text: "**要点**：\n\n- 第一项\n- 第二项\n\n`code`" });
                emit("qa-done", { conversation_id: "conv1", message_id: 7, sources: [] });
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

        await user.type(screen.getByLabelText("问题"), "问题");
        await user.click(screen.getByRole("button", { name: "发送" }));

        expect((await screen.findByText("要点")).tagName).toBe("STRONG");
        expect(screen.getAllByRole("listitem")).toHaveLength(2);
        expect(screen.getByText("code").tagName).toBe("CODE");
    });

    it("keeps the streamed reply when effects run twice under StrictMode", async () => {
        const user = userEvent.setup();
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "qa_ask") {
                emit("qa-start", { conversation_id: "conv1" });
                emit("qa-delta", { conversation_id: "conv1", text: "服务拆分要" });
                emit("qa-delta", { conversation_id: "conv1", text: "注意业务边界。" });
                emit("qa-done", { conversation_id: "conv1", message_id: 7, sources: [] });
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
        render(
            <StrictMode>
                <MemoryRouter>
                    <QaPage />
                </MemoryRouter>
            </StrictMode>,
        );
        await screen.findByText("输入问题，从知识库中寻找答案");

        await user.type(screen.getByLabelText("问题"), "问题");
        await user.click(screen.getByRole("button", { name: "发送" }));

        // 监听若重复绑定，第二个 qa-done 会用空内容覆盖刚固化的回答
        expect(await screen.findByText(/服务拆分要注意业务边界。/)).toBeTruthy();
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

    it("renames a conversation from the item menu", async () => {
        const user = userEvent.setup();
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "list_conversations") {
                return [CONVERSATION];
            }
            if (cmd === "rename_conversation") {
                return null;
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        renderPage();
        await screen.findByText("服务拆分实践");

        await user.click(screen.getByRole("button", { name: "会话操作" }));
        await user.click(screen.getByRole("menuitem", { name: /重命名/ }));

        const input = screen.getByLabelText("会话标题");
        await user.clear(input);
        await user.type(input, "新标题");
        await user.click(screen.getByRole("button", { name: "确认重命名" }));

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("rename_conversation", { id: "conv1", title: "新标题" });
        });
        expect(screen.queryByLabelText("会话标题")).toBeNull();
    });

    it("deletes the current conversation from the item menu after confirmation", async () => {
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
            if (cmd === "delete_conversation") {
                return null;
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        renderPage();
        await screen.findByText("服务拆分实践");

        await user.click(screen.getByText("服务拆分实践"));
        await screen.findByText("之前的问题");

        await user.click(screen.getByRole("button", { name: "会话操作" }));
        await user.click(screen.getByRole("menuitem", { name: /删除/ }));

        await waitFor(() => {
            expect(confirmMock).toHaveBeenCalled();
            expect(invokeMock).toHaveBeenCalledWith("delete_conversation", { id: "conv1" });
        });
        // 删除的是当前会话，对话区清空
        expect(await screen.findByText("输入问题，从知识库中寻找答案")).toBeTruthy();
        expect(screen.queryByText("之前的问题")).toBeNull();
    });

    it("keeps the conversation when deletion is cancelled", async () => {
        const user = userEvent.setup();
        confirmMock.mockResolvedValue(false);
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "list_conversations") {
                return [CONVERSATION];
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        renderPage();
        await screen.findByText("服务拆分实践");

        await user.click(screen.getByRole("button", { name: "会话操作" }));
        await user.click(screen.getByRole("menuitem", { name: /删除/ }));

        await waitFor(() => {
            expect(confirmMock).toHaveBeenCalled();
        });
        expect(invokeMock).not.toHaveBeenCalledWith("delete_conversation", expect.anything());
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

    it("re-enables input after switching conversation during streaming, without polluting entries", async () => {
        const user = userEvent.setup();
        // qa_ask 挂起不 resolve，模拟 LLM 长时间流式
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "qa_ask") {
                emit("qa-start", { conversation_id: "conv1" });
                emit("qa-delta", { conversation_id: "conv1", text: "流式中的回答" });
                await new Promise(() => {});
            }
            if (cmd === "list_conversations") {
                return [CONVERSATION, OTHER_CONVERSATION];
            }
            if (cmd === "list_messages") {
                return [
                    { id: 1, role: "user", content: "之前的问题", sources: [], created_at: "2026-10-22T01:00:00Z" },
                ];
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        renderPage();
        await screen.findByText("输入问题，从知识库中寻找答案");

        await user.type(screen.getByLabelText("问题"), "问题");
        await user.click(screen.getByRole("button", { name: "发送" }));
        expect(screen.getByLabelText("问题")).toBeDisabled();

        // 流式期间切换到另一个历史会话
        await user.click(screen.getByText("会议纪要"));
        await screen.findByText("之前的问题");
        expect(screen.getByLabelText("问题")).toBeEnabled();

        // 旧会话的 qa-done 到达：因会话 id 不匹配被过滤，不污染当前会话的 entries
        act(() => {
            emit("qa-done", { conversation_id: "conv1", message_id: 7, sources: [] });
        });
        expect(screen.queryByText(/流式中的回答/)).toBeNull();
        expect(screen.getByText("之前的问题")).toBeTruthy();
        // 输入保持可用
        expect(screen.getByLabelText("问题")).toBeEnabled();
    });

    it("syncs conversation id when qa_ask resolves before events in a new conversation", async () => {
        const user = userEvent.setup();
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "qa_ask") {
                // 不推送任何事件直接 resolve：命令 resolve 时仍在流式等待、会话 id 仍为 null
                return { conversation_id: "conv-new", sources: [] };
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
        // flush 微任务：让 qa_ask 的 resolve 先完成会话 id 同步
        await act(async () => {});

        // 会话 id 已同步为 conv-new：随后到达的 qa-done 能匹配并复位输入
        act(() => {
            emit("qa-delta", { conversation_id: "conv-new", text: "回答内容" });
            emit("qa-done", { conversation_id: "conv-new", message_id: 9, sources: [] });
        });
        expect(await screen.findByText("回答内容")).toBeTruthy();
        expect(screen.getByLabelText("问题")).toBeEnabled();
    });
});
