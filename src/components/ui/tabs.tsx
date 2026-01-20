import type { ComponentProps } from "react";
import { cn } from "@/lib/utils";

export interface TabsProps {
    value: string;
    onValueChange: (value: string) => void;
    className?: string;
    items: Array<{ value: string; label: string }>;
}

/// 轻量受控 Tabs（button 实现，不依赖 radix）
export function Tabs({ value, onValueChange, className, items }: TabsProps) {
    return (
        <div role="tablist" className={cn("inline-flex items-center gap-1 rounded-md bg-muted p-1", className)}>
            {items.map((item) => (
                <button
                    key={item.value}
                    type="button"
                    role="tab"
                    aria-selected={value === item.value}
                    onClick={() => onValueChange(item.value)}
                    className={cn(
                        "rounded-sm px-3 py-1 text-[13px] font-medium transition-colors",
                        value === item.value
                            ? "bg-background text-foreground shadow-sm"
                            : "text-muted-foreground hover:text-foreground",
                    )}
                >
                    {item.label}
                </button>
            ))}
        </div>
    );
}

export function TabsContent({ className, ...props }: ComponentProps<"div">) {
    return <div role="tabpanel" className={cn("outline-none", className)} {...props} />;
}
