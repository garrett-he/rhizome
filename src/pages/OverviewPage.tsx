import { RefreshCw } from "lucide-react";
import { CategoryCard } from "@/components/overview/CategoryCard";
import { PendingCard } from "@/components/overview/PendingCard";
import { PipelineCard } from "@/components/overview/PipelineCard";
import { Button } from "@/components/ui/button";
import { useOverview } from "@/hooks/useOverview";
import { formatBytes } from "@/lib/format";
import type { Overview } from "@/types/overview";

function MetricCard({ label, value }: { label: string; value: string }) {
    return (
        <div className="flex flex-col gap-2 rounded-lg border border-neutral-200 bg-neutral-50 p-5 shadow-sm transition-shadow hover:shadow-md">
            <span className="text-[13px] text-neutral-500">{label}</span>
            <span className="text-[28px] font-semibold leading-8 text-neutral-950">{value}</span>
        </div>
    );
}

function OverviewBody({ overview }: { overview: Overview }) {
    return (
        <>
            <div className="grid grid-cols-4 gap-4">
                <MetricCard label="文档总数" value={String(overview.total_documents)} />
                <MetricCard label="已索引" value={String(overview.indexed_documents)} />
                <MetricCard label="待处理" value={String(overview.pending_documents)} />
                <MetricCard label="存储占用" value={formatBytes(overview.storage_bytes)} />
            </div>
            <PipelineCard overview={overview} />
            <div className="grid grid-cols-2 gap-4">
                <PendingCard overview={overview} />
                <CategoryCard categories={overview.categories} />
            </div>
        </>
    );
}

export function OverviewPage() {
    const { overview, loading, error, refresh } = useOverview();

    return (
        <div className="flex flex-col gap-6 p-8">
            <div className="flex items-center justify-between">
                <h1 className="text-2xl font-semibold text-neutral-950">知识库总览</h1>
                <div className="flex gap-3">
                    <Button type="button" variant="outline" disabled={loading} onClick={() => void refresh()}>
                        <RefreshCw aria-hidden className="h-4 w-4" />
                        刷新
                    </Button>
                </div>
            </div>

            {error && (
                <p role="alert" className="text-[13px] text-red-600">
                    {error}
                </p>
            )}

            {overview && <OverviewBody overview={overview} />}
        </div>
    );
}
