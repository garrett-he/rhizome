import type { ReactNode } from "react";

interface SettingsCardProps {
    title: string;
    desc?: string;
    children: ReactNode;
}

export function SettingsCard({ title, desc, children }: SettingsCardProps) {
    return (
        <section className="overflow-hidden rounded-lg border border-neutral-200 bg-neutral-50 shadow-sm">
            <div className="flex flex-col gap-1 px-6 py-4">
                <h2 className="text-base font-medium text-neutral-950">{title}</h2>
                {desc && <p className="text-[13px] text-neutral-500">{desc}</p>}
            </div>
            <div className="p-6">{children}</div>
        </section>
    );
}
