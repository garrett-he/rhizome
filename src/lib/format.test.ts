import { describe, expect, it } from "vitest";
import { formatRelativeTime } from "./format";

describe("formatRelativeTime", () => {
    it("shows HH:mm for today", () => {
        const now = new Date(2026, 9, 22, 21, 40);
        expect(formatRelativeTime("2026-10-22T21:05:00+08:00", now)).toBe("21:05");
    });

    it("shows 昨天 for previous day", () => {
        const now = new Date(2026, 9, 22, 9, 0);
        expect(formatRelativeTime("2026-10-21T20:00:00+08:00", now)).toBe("昨天");
    });

    it("shows MM-DD for older dates", () => {
        const now = new Date(2026, 9, 22);
        expect(formatRelativeTime("2026-10-12T01:00:00+08:00", now)).toBe("10-12");
    });

    it("returns — for invalid timestamp", () => {
        expect(formatRelativeTime("not-a-date")).toBe("—");
    });
});
