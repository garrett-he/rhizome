import { invoke } from "@tauri-apps/api/core";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { Mock } from "vitest";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { confirmAction } from "@/lib/dialog";
import { useCategories } from "./useCategories";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@/lib/dialog", () => ({ confirmAction: vi.fn() }));

const invokeMock = vi.mocked(invoke) as unknown as Mock;
const confirmMock = vi.mocked(confirmAction);

beforeEach(() => {
    invokeMock.mockReset();
    confirmMock.mockReset();
    confirmMock.mockResolvedValue(true);
    invokeMock.mockResolvedValue([]);
});

describe("useCategories", () => {
    it("rejects blank name on create without invoking", async () => {
        const { result } = renderHook(() => useCategories());
        await waitFor(() => {
            expect(result.current.loading).toBe(false);
        });

        let ok = true;
        await act(async () => {
            ok = await result.current.create("   ");
        });

        expect(ok).toBe(false);
        expect(result.current.error).toBe("分类名不能为空");
        expect(invokeMock).not.toHaveBeenCalledWith("create_category", expect.anything());
    });

    it("rejects blank name on rename without invoking", async () => {
        const { result } = renderHook(() => useCategories());
        await waitFor(() => {
            expect(result.current.loading).toBe(false);
        });

        let ok = true;
        await act(async () => {
            ok = await result.current.rename("cat-1", "  ");
        });

        expect(ok).toBe(false);
        expect(result.current.error).toBe("分类名不能为空");
        expect(invokeMock).not.toHaveBeenCalledWith("rename_category", expect.anything());
    });

    it("does not delete when confirm is dismissed", async () => {
        confirmMock.mockResolvedValue(false);
        const { result } = renderHook(() => useCategories());
        await waitFor(() => {
            expect(result.current.loading).toBe(false);
        });

        await act(async () => {
            await result.current.remove({ id: "cat-1", name: "A", document_count: 2 });
        });

        expect(invokeMock).not.toHaveBeenCalledWith("delete_category", expect.anything());
    });

    it("surfaces delete errors", async () => {
        confirmMock.mockResolvedValue(true);
        invokeMock.mockImplementation(async (cmd: string) => {
            if (cmd === "delete_category") {
                throw "删除失败";
            }
            return [];
        });
        const { result } = renderHook(() => useCategories());
        await waitFor(() => {
            expect(result.current.loading).toBe(false);
        });

        await act(async () => {
            await result.current.remove({ id: "cat-1", name: "A", document_count: 0 });
        });

        expect(result.current.error).toBe("删除失败");
    });

    it("surfaces load errors from the initial refresh", async () => {
        invokeMock.mockRejectedValue("未选择知识库");
        const { result } = renderHook(() => useCategories());

        await waitFor(() => {
            expect(result.current.error).toBe("未选择知识库");
        });
    });
});
