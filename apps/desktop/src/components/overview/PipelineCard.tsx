import { ArrowRight } from "lucide-react";
import { cn } from "@/lib/utils";
import type { Overview } from "@/types/overview";

function StageCard({ count, label, danger = false }: { count: number; label: string; danger?: boolean }) {
    return (
        <div className="flex flex-1 flex-col gap-1 rounded-lg border border-neutral-200 bg-neutral-50 p-4 shadow-sm">
            <span className={cn("text-2xl font-semibold", danger ? "text-[#e7000b]" : "text-neutral-950")}>
                {count}
            </span>
            <span className="text-[13px] text-neutral-950">{label}</span>
        </div>
    );
}

function PipelineArrow() {
    return <ArrowRight aria-hidden className="h-4 w-4 shrink-0 text-neutral-500" />;
}

export function PipelineCard({ overview }: { overview: Overview }) {
    return (
        <section className="flex flex-col gap-3">
            <h2 className="text-base font-semibold text-neutral-950">处理流水线</h2>
            <div className="flex items-center gap-3">
                <StageCard count={overview.pending_documents} label="未编译" />
                <PipelineArrow />
                <StageCard count={0} label="已编译" />
                <PipelineArrow />
                <StageCard count={overview.indexed_documents} label="已索引" />
                <PipelineArrow />
                <StageCard count={overview.failed_documents} label="失败" danger />
            </div>
        </section>
    );
}
