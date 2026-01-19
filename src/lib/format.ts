export function formatBytes(bytes: number): string {
    if (bytes <= 0) {
        return "0 B";
    }
    const units = ["B", "KB", "MB", "GB", "TB"];
    const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
    const value = bytes / 1024 ** index;
    return `${value >= 100 ? value.toFixed(0) : value.toFixed(1)} ${units[index]}`;
}

/** RFC3339 时间戳 → "MM-DD" 短格式（解析失败时返回 "—"） */
export function formatShortDate(timestamp: string): string {
    const date = new Date(timestamp);
    if (Number.isNaN(date.getTime())) {
        return "—";
    }
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    return `${month}-${day}`;
}
