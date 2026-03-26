import { ask } from "@tauri-apps/plugin-dialog";
import { describe, expect, it, vi } from "vitest";
import { confirmAction } from "./dialog";

vi.mock("@tauri-apps/plugin-dialog", () => ({ ask: vi.fn() }));

describe("confirmAction", () => {
    it("delegates to plugin-dialog ask with warning kind", async () => {
        vi.mocked(ask).mockResolvedValue(true);

        await expect(confirmAction("确定删除吗？")).resolves.toBe(true);
        expect(ask).toHaveBeenCalledWith("确定删除吗？", { title: "确认操作", kind: "warning" });
    });

    it("returns false when the dialog is dismissed", async () => {
        vi.mocked(ask).mockResolvedValue(false);

        await expect(confirmAction("确定删除吗？")).resolves.toBe(false);
    });
});
