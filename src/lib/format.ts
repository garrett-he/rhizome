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

/**
 * RFC3339 时间戳 → 相对时间：今天显示 "HH:mm"、昨天显示 "昨天"、
 * 更早显示 "MM-DD"（解析失败返回 "—"）。
 */
export function formatRelativeTime(timestamp: string, now: Date = new Date()): string {
    const date = new Date(timestamp);
    if (Number.isNaN(date.getTime())) {
        return "—";
    }
    const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
    const dayOffset = Math.round((startOfDay(now) - startOfDay(date)) / 86_400_000);
    if (dayOffset === 0) {
        const hours = String(date.getHours()).padStart(2, "0");
        const minutes = String(date.getMinutes()).padStart(2, "0");
        return `${hours}:${minutes}`;
    }
    if (dayOffset === 1) {
        return "昨天";
    }
    return formatShortDate(timestamp);
}
