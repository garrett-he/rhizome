import { invoke } from "@tauri-apps/api/core";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Mock } from "vitest";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SettingsPage } from "./SettingsPage";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke) as unknown as Mock;

const DEFAULT_PROMPT = "# docs compile default prompt with {content}";

beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string, _args?: Record<string, unknown>) => {
        if (cmd === "get_settings") {
            return [{ key: "embedding_model", value: "BAAI/bge-m3" }];
        }
        if (cmd === "get_overview") {
            return {
                total_documents: 0,
                indexed_documents: 5,
                pending_documents: 0,
                failed_documents: 0,
                storage_bytes: 0,
                categories: [],
            };
        }
        if (cmd === "get_default_prompt") {
            return DEFAULT_PROMPT;
        }
        if (cmd === "set_setting") {
            return null;
        }
        throw new Error(`unexpected command: ${cmd}`);
    });
});

describe("SettingsPage", () => {
    it("renders cards with values from settings", async () => {
        render(<SettingsPage />);

        expect(await screen.findByText("设置")).toBeTruthy();
        expect(screen.getByText("嵌入模型")).toBeTruthy();
        expect(screen.getByText("LLM")).toBeTruthy();
        expect(screen.getByText("编译提示词")).toBeTruthy();
        expect(screen.getByText("问答提示词")).toBeTruthy();
        expect(screen.getByText("docs_compile")).toBeTruthy();
        expect(screen.getByText("docs_answer")).toBeTruthy();
        expect(screen.getByText("MCP 服务")).toBeTruthy();
        expect(screen.getByLabelText("嵌入模型")).toHaveValue("BAAI/bge-m3");
        expect(screen.getByText("当前 5 篇已索引")).toBeTruthy();
        expect(screen.getByText("未实现")).toBeTruthy();
        expect(screen.getByText(/mcpServers/)).toBeTruthy();
    });

    it("saves embedding model on change", async () => {
        const user = userEvent.setup();
        render(<SettingsPage />);
        await screen.findByText("设置");

        await user.selectOptions(screen.getByLabelText("嵌入模型"), "BAAI/bge-small-zh-v1.5");

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("set_setting", {
                key: "embedding_model",
                value: "BAAI/bge-small-zh-v1.5",
            });
        });
    });

    it("saves LLM fields together, empty field deletes key", async () => {
        const user = userEvent.setup();
        render(<SettingsPage />);
        await screen.findByText("设置");

        await user.type(screen.getByLabelText("Base URL"), "https://api.example.com/v1");
        await user.click(screen.getByRole("button", { name: "保存" }));

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("set_setting", {
                key: "llm_base_url",
                value: "https://api.example.com/v1",
            });
            expect(invokeMock).toHaveBeenCalledWith("set_setting", { key: "llm_api_key", value: null });
            expect(invokeMock).toHaveBeenCalledWith("set_setting", { key: "llm_model", value: null });
        });
    });

    it("edits compile prompt in modal and saves to settings", async () => {
        const user = userEvent.setup();
        render(<SettingsPage />);
        await screen.findByText("设置");

        // 两张提示词卡片，第一张为 docs_compile
        await user.click(screen.getAllByRole("button", { name: "编辑" })[0]);
        const textarea = await screen.findByLabelText("提示词内容");
        expect(textarea).toHaveValue(DEFAULT_PROMPT);

        await user.clear(textarea);
        await user.type(textarea, "custom prompt");
        await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "保存" }));

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("set_setting", {
                key: "prompt:docs_compile",
                value: "custom prompt",
            });
        });
        expect(screen.queryByRole("dialog")).toBeNull();
    });

    it("edits answer prompt in modal and saves to settings", async () => {
        const user = userEvent.setup();
        render(<SettingsPage />);
        await screen.findByText("设置");

        await user.click(screen.getAllByRole("button", { name: "编辑" })[1]);
        const textarea = await screen.findByLabelText("提示词内容");

        await user.clear(textarea);
        await user.type(textarea, "answer prompt");
        await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "保存" }));

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("set_setting", {
                key: "prompt:docs_answer",
                value: "answer prompt",
            });
        });
    });

    it("resets prompt to default by deleting the key", async () => {
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "get_settings") {
                return [{ key: "prompt:docs_compile", value: "custom" }];
            }
            if (cmd === "set_setting") {
                return null;
            }
            if (cmd === "get_overview") {
                return {
                    total_documents: 0,
                    indexed_documents: 0,
                    pending_documents: 0,
                    failed_documents: 0,
                    storage_bytes: 0,
                    categories: [],
                };
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        const user = userEvent.setup();
        render(<SettingsPage />);

        expect(await screen.findByText("已定制")).toBeTruthy();
        // 第一张卡（docs_compile）已定制；第二张未定制故禁用
        await user.click(screen.getAllByRole("button", { name: "恢复默认" })[0]);

        await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith("set_setting", {
                key: "prompt:docs_compile",
                value: null,
            });
        });
    });
});
