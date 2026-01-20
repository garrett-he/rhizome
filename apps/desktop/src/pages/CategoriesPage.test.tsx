import { invoke } from "@tauri-apps/api/core";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import type { Mock } from "vitest";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { CategoriesPage } from "./CategoriesPage";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke) as unknown as Mock;

const CATEGORIES = [
    { id: "cat-a", name: "技术笔记", document_count: 3 },
    { id: "cat-b", name: "会议记录", document_count: 0 },
];

function renderPage() {
    return render(
        <MemoryRouter>
            <CategoriesPage />
        </MemoryRouter>,
    );
}

function mockDefault() {
    invokeMock.mockImplementation(async (cmd: string) => {
        if (cmd === "list_categories") {
            return CATEGORIES;
        }
        throw new Error(`unexpected command: ${cmd}`);
    });
}

beforeEach(() => {
    invokeMock.mockReset();
    mockDefault();
});

describe("CategoriesPage", () => {
    it("renders category list with document counts", async () => {
        renderPage();

        expect(await screen.findByText("技术笔记")).toBeTruthy();
        expect(screen.getByText("会议记录")).toBeTruthy();
        expect(screen.getByText("3 个文档")).toBeTruthy();
        expect(screen.getByText("0 个文档")).toBeTruthy();
        expect(screen.getByLabelText("分类名")).toBeTruthy();
    });

    it("shows empty state when no categories", async () => {
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "list_categories") {
                return [];
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        renderPage();

        expect(await screen.findByText("暂无分类，在上方输入名称创建")).toBeTruthy();
    });

    it("creates a category and refreshes the list", async () => {
        const user = userEvent.setup();
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "list_categories") {
                return CATEGORIES;
            }
            if (cmd === "create_category") {
                return { id: "cat-c", name: "周报", document_count: 0 };
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        renderPage();
        await screen.findByText("技术笔记");

        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "create_category") {
                return { id: "cat-c", name: "周报", document_count: 0 };
            }
            if (cmd === "list_categories") {
                return [...CATEGORIES, { id: "cat-c", name: "周报", document_count: 0 }];
            }
            throw new Error(`unexpected command: ${cmd}`);
        });

        await user.type(screen.getByLabelText("分类名"), "周报");
        await user.click(screen.getByRole("button", { name: "新建分类" }));

        expect(await screen.findByText("周报")).toBeTruthy();
        expect(invokeMock).toHaveBeenCalledWith("create_category", { name: "周报" });
    });

    it("shows backend error on duplicate name", async () => {
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("技术笔记");

        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "create_category") {
                throw "分类 '技术笔记' 已存在";
            }
            throw new Error(`unexpected command: ${cmd}`);
        });

        await user.type(screen.getByLabelText("分类名"), "技术笔记");
        await user.click(screen.getByRole("button", { name: "新建分类" }));

        expect(await screen.findByText("分类 '技术笔记' 已存在")).toBeTruthy();
    });

    it("renames a category inline", async () => {
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("会议记录");

        await user.click(screen.getAllByRole("button", { name: "重命名" })[1]);

        const input = await screen.findByLabelText("新分类名");
        await user.clear(input);
        await user.type(input, "每周例会");
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "rename_category") {
                return { id: "cat-b", name: "每周例会", document_count: 0 };
            }
            if (cmd === "list_categories") {
                return [CATEGORIES[0], { id: "cat-b", name: "每周例会", document_count: 0 }];
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        await user.click(screen.getByRole("button", { name: "确认" }));

        expect(await screen.findByText("每周例会")).toBeTruthy();
        expect(invokeMock).toHaveBeenCalledWith("rename_category", { id: "cat-b", name: "每周例会" });
    });

    it("deletes a category after confirm and shows detach note", async () => {
        const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("技术笔记");

        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "delete_category") {
                return { detached: 3 };
            }
            if (cmd === "list_categories") {
                return [CATEGORIES[1]];
            }
            throw new Error(`unexpected command: ${cmd}`);
        });
        await user.click(screen.getAllByRole("button", { name: "删除" })[0]);

        expect(await screen.findByText("已删除分类「技术笔记」，3 个文档已变为未分类")).toBeTruthy();
        expect(confirmSpy).toHaveBeenCalledWith("删除分类「技术笔记」将把 3 个文档变为未分类，确定删除？");
        expect(invokeMock).toHaveBeenCalledWith("delete_category", { id: "cat-a" });
    });

    it("does not delete when confirm is cancelled", async () => {
        vi.spyOn(window, "confirm").mockReturnValue(false);
        const user = userEvent.setup();
        renderPage();
        await screen.findByText("技术笔记");

        await user.click(screen.getAllByRole("button", { name: "删除" })[0]);

        expect(invokeMock).not.toHaveBeenCalledWith("delete_category", expect.anything());
        expect(screen.getByText("技术笔记")).toBeTruthy();
    });
});
