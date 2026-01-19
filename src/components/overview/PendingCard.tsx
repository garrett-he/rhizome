import { FileText } from "lucide-react";
import type { Overview } from "@/types/overview";

function EmptyHint({ text }: { text: string }) {
    return <div className="flex items-center justify-center rounded-md p-6 text-[13px] text-neutral-500">{text}</div>;
}

export function PendingCard({ overview }: { overview: Overview }) {
    return (
        <div className="flex flex-col rounded-lg border border-neutral-200 bg-neutral-50 shadow-sm">
            <div className="px-5 pt-4 pb-2">
                <h3 className="text-base font-medium text-neutral-950">待处理</h3>
            </div>
            <div className="flex flex-col gap-1 p-2.5">
                <div className="flex items-center gap-2 rounded-md px-3 py-2">
                    <FileText aria-hidden className="h-4 w-4 shrink-0 text-neutral-500" />
                    <span className="flex-1 text-sm text-neutral-950">{overview.pending_documents} 篇未编译</span>
                </div>
                {overview.pending_documents === 0 && <EmptyHint text="暂无待处理文档" />}
            </div>
        </div>
    );
}
