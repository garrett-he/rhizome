import { describe, expect, it } from "vitest";
import { charCount, labeledSectionCount, shortSha1, splitByChunkMarkers } from "./markdown";

describe("splitByChunkMarkers", () => {
    it("splits sections by markers and keeps lead-in", () => {
        const markdown = "# 标题\n\n<!-- chunk: 正文 -->\n第一段。\n\n<!-- chunk: 结尾 -->\n最后一段。";
        const sections = splitByChunkMarkers(markdown);

        expect(sections).toHaveLength(3);
        expect(sections[0]).toEqual({ label: "", text: "# 标题" });
        expect(sections[1]).toEqual({ label: "正文", text: "第一段。" });
        expect(sections[2]).toEqual({ label: "结尾", text: "最后一段。" });
        expect(labeledSectionCount(sections)).toBe(2);
    });

    it("tolerates marker spacing variants and CRLF", () => {
        const markdown = "前导\r\n<!--chunk:  空格标签-->\r\n内容";
        const sections = splitByChunkMarkers(markdown);

        expect(sections).toEqual([
            { label: "", text: "前导" },
            { label: "空格标签", text: "内容" },
        ]);
    });

    it("returns single section without markers", () => {
        expect(splitByChunkMarkers("只有一段")).toEqual([{ label: "", text: "只有一段" }]);
        expect(splitByChunkMarkers("")).toEqual([]);
    });

    it("drops empty sections between markers", () => {
        const markdown = "<!-- chunk: a -->\n内容甲\n<!-- chunk: b -->\n<!-- chunk: c -->\n内容丙";
        const sections = splitByChunkMarkers(markdown);

        expect(sections).toEqual([
            { label: "a", text: "内容甲" },
            { label: "c", text: "内容丙" },
        ]);
    });
});

describe("shortSha1", () => {
    it("truncates long sha1", () => {
        expect(shortSha1("a3f9e2b81c77d4e0f1a2b3c4d5e6f7a8b9c0c41d")).toBe("a3f9e2…c41d");
    });

    it("returns short input as-is", () => {
        expect(shortSha1("abc")).toBe("abc");
        expect(shortSha1("abcdefghij")).toBe("abcdefghij");
    });
});

describe("charCount", () => {
    it("formats with thousand separators", () => {
        expect(charCount("x".repeat(1100))).toBe("1,100 字");
        expect(charCount("你好")).toBe("2 字");
    });
});
