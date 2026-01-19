import {
    ChevronsUpDown,
    Hexagon,
    LayoutDashboard,
    Library,
    ListChecks,
    type LucideIcon,
    MessageCircle,
    Search,
    Settings,
    Tags,
} from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

interface NavItemProps {
    icon: LucideIcon;
    label: string;
    active?: boolean;
}

function NavItem({ icon: Icon, label, active = false }: NavItemProps) {
    return (
        <button
            type="button"
            disabled={!active}
            aria-current={active ? "page" : undefined}
            className={cn(
                "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-zinc-950 transition-colors",
                active && "cursor-default bg-zinc-100",
                !active && "cursor-not-allowed opacity-50",
            )}
        >
            <Icon aria-hidden className="h-4 w-4" />
            {label}
        </button>
    );
}

function SectionTitle({ label }: { label: string }) {
    return <p className="px-2 py-1 text-xs font-medium text-neutral-500">{label}</p>;
}

interface AppShellProps {
    vaultPath: string;
    children: ReactNode;
}

export function AppShell({ vaultPath, children }: AppShellProps) {
    return (
        <div className="flex h-screen overflow-hidden bg-neutral-100">
            <aside className="flex w-64 shrink-0 flex-col gap-4 border-r border-zinc-200 bg-neutral-50 p-2">
                <div className="flex items-center gap-2 rounded-md p-2">
                    <div className="flex h-8 w-8 items-center justify-center rounded-[10px] bg-zinc-100">
                        <Hexagon aria-hidden className="h-4 w-4 text-zinc-800" />
                    </div>
                    <span className="text-sm font-medium text-zinc-950">rhizome</span>
                </div>

                <nav className="flex flex-1 flex-col gap-0.5 overflow-y-auto">
                    <SectionTitle label="工作区" />
                    <NavItem icon={LayoutDashboard} label="总览" active />
                    <NavItem icon={Library} label="文档库" />
                    <NavItem icon={Search} label="搜索" />
                    <NavItem icon={MessageCircle} label="问答" />
                    <SectionTitle label="管理" />
                    <NavItem icon={Tags} label="分类管理" />
                    <NavItem icon={ListChecks} label="任务中心" />
                    <NavItem icon={Settings} label="设置" />
                </nav>

                <div className="flex items-center justify-between gap-2 rounded-md bg-neutral-50 p-2" title={vaultPath}>
                    <span className="truncate text-xs text-zinc-950">{vaultPath}</span>
                    <ChevronsUpDown aria-hidden className="h-4 w-4 shrink-0 text-neutral-500" />
                </div>
            </aside>

            <main className="flex-1 overflow-y-auto">{children}</main>
        </div>
    );
}
