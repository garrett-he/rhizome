import {
    Check,
    ChevronsUpDown,
    Hexagon,
    LayoutDashboard,
    Library,
    ListChecks,
    type LucideIcon,
    MessageCircle,
    Search,
    Settings,
    Settings2,
    Tags,
} from "lucide-react";
import { type ReactNode, useState } from "react";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import { cn } from "@/lib/utils";
import type { Vault } from "@/types/profile";

interface NavItemProps {
    icon: LucideIcon;
    label: string;
    to?: string;
}

function NavItem({ icon: Icon, label, to }: NavItemProps) {
    const navigate = useNavigate();
    const { pathname } = useLocation();
    const active = to === undefined ? false : pathname === to;
    const enabled = to !== undefined;

    return (
        <button
            type="button"
            disabled={!enabled}
            aria-current={active ? "page" : undefined}
            onClick={enabled ? () => navigate(to) : undefined}
            className={cn(
                "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-zinc-950 transition-colors",
                active && "cursor-default bg-zinc-100",
                !active && enabled && "hover:bg-zinc-100",
                !enabled && "cursor-not-allowed opacity-50",
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
    vaults?: Vault[];
    onSwitchVault?: (path: string) => void;
    children?: ReactNode;
}

function VaultMenu({
    vaultPath,
    vaults,
    onSwitchVault,
}: {
    vaultPath: string;
    vaults: Vault[];
    onSwitchVault?: (path: string) => void;
}) {
    const navigate = useNavigate();
    const [open, setOpen] = useState(false);
    const close = () => setOpen(false);

    return (
        <div className="relative">
            {open && (
                <button
                    type="button"
                    aria-label="关闭菜单"
                    aria-hidden
                    tabIndex={-1}
                    className="fixed inset-0 z-40 cursor-default"
                    onClick={close}
                />
            )}
            <button
                type="button"
                aria-haspopup="menu"
                aria-expanded={open}
                onClick={() => setOpen((value) => !value)}
                className="flex w-full items-center justify-between gap-2 rounded-md bg-neutral-50 p-2 transition-colors hover:bg-zinc-100"
            >
                <span className="truncate text-xs text-zinc-950">{vaultPath}</span>
                <ChevronsUpDown
                    aria-hidden
                    className={cn("h-4 w-4 shrink-0 text-neutral-500 transition-transform", open && "rotate-180")}
                />
            </button>
            {open && (
                <div
                    role="menu"
                    className="absolute bottom-full left-0 z-50 mb-1 w-full min-w-56 rounded-lg border border-neutral-200 bg-neutral-50 p-1 shadow-md"
                >
                    {vaults.map((vault) => (
                        <button
                            key={vault.path}
                            role="menuitem"
                            type="button"
                            onClick={() => {
                                close();
                                if (vault.path !== vaultPath) {
                                    onSwitchVault?.(vault.path);
                                }
                            }}
                            className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs text-zinc-950 transition-colors hover:bg-zinc-100"
                        >
                            <Check
                                aria-hidden
                                className={cn(
                                    "h-3.5 w-3.5 shrink-0 text-zinc-950",
                                    vault.path === vaultPath ? "opacity-100" : "opacity-0",
                                )}
                            />
                            <span className="truncate">{vault.path}</span>
                        </button>
                    ))}
                    <div aria-hidden className="my-1 h-px bg-neutral-200" />
                    <button
                        role="menuitem"
                        type="button"
                        onClick={() => {
                            close();
                            navigate("/onboarding");
                        }}
                        className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs text-zinc-950 transition-colors hover:bg-zinc-100"
                    >
                        <Settings2 aria-hidden className="h-3.5 w-3.5 shrink-0" />
                        管理知识库
                    </button>
                </div>
            )}
        </div>
    );
}

export function AppShell({ vaultPath, vaults = [], onSwitchVault, children }: AppShellProps) {
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
                    <NavItem icon={LayoutDashboard} label="总览" to="/" />
                    <NavItem icon={Library} label="文档库" to="/documents" />
                    <NavItem icon={Search} label="搜索" />
                    <NavItem icon={MessageCircle} label="问答" />
                    <SectionTitle label="管理" />
                    <NavItem icon={Tags} label="分类管理" />
                    <NavItem icon={ListChecks} label="任务中心" />
                    <NavItem icon={Settings} label="设置" to="/settings" />
                </nav>

                <VaultMenu vaultPath={vaultPath} vaults={vaults} onSwitchVault={onSwitchVault} />
            </aside>

            <main className="flex-1 overflow-y-auto">{children ?? <Outlet />}</main>
        </div>
    );
}
