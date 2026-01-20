import { invoke } from "@tauri-apps/api/core";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { Mock } from "vitest";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useCategories } from "./useCategories";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke) as unknown as Mock;

beforeEach(() => {
    invokeMock.mockReset();
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
        vi.spyOn(window, "confirm").mockReturnValue(false);
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
        vi.spyOn(window, "confirm").mockReturnValue(true);
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
