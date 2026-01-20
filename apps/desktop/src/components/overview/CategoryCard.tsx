import type { CategoryStat } from "@/types/overview";

function EmptyHint({ text }: { text: string }) {
    return <div className="flex items-center justify-center rounded-md p-6 text-[13px] text-neutral-500">{text}</div>;
}

function CategoryRow({ stat, max }: { stat: CategoryStat; max: number }) {
    return (
        <div className="flex items-center gap-3">
            <span className="w-16 shrink-0 truncate text-[13px] text-neutral-950">{stat.name}</span>
            <div
                className="h-3.5 rounded bg-neutral-900"
                style={{ width: `${Math.max((stat.count / max) * 100, 4)}%` }}
            />
            <span className="text-[13px] text-neutral-500">{stat.count}</span>
        </div>
    );
}

export function CategoryCard({ categories }: { categories: CategoryStat[] }) {
    const max = Math.max(...categories.map((c) => c.count), 1);

    return (
        <div className="flex flex-col rounded-lg border border-neutral-200 bg-neutral-50 shadow-sm">
            <div className="px-5 pt-4 pb-2">
                <h3 className="text-base font-medium text-neutral-950">分类分布</h3>
            </div>
            <div className="flex flex-col gap-3 p-5">
                {categories.length === 0 ? (
                    <EmptyHint text="暂无分类数据" />
                ) : (
                    categories.map((stat) => <CategoryRow key={stat.name} stat={stat} max={max} />)
                )}
            </div>
        </div>
    );
}
