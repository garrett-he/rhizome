import { AlertTriangle } from "lucide-react";
import type { ComponentProps } from "react";
import { cn } from "@/lib/utils";

/// 警告样式的 Alert（标题 + 描述）
export function Alert({ className, ...props }: ComponentProps<"div">) {
    return (
        <div
            role="alert"
            className={cn(
                "flex w-full items-start gap-3 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-red-800",
                className,
            )}
            {...props}
        />
    );
}

export function AlertIcon({ className, ...props }: ComponentProps<"span">) {
    return (
        <span className={cn("mt-0.5 shrink-0 text-red-500", className)} {...props}>
            <AlertTriangle aria-hidden className="h-4 w-4" />
        </span>
    );
}

export function AlertTitle({ className, ...props }: ComponentProps<"div">) {
    return <div className={cn("text-[13px] font-medium leading-none", className)} {...props} />;
}

export function AlertDescription({ className, ...props }: ComponentProps<"div">) {
    return <div className={cn("mt-1 text-[13px] leading-5 text-red-700", className)} {...props} />;
}
