import { SettingsCard } from "./SettingsCard";

const MCP_CONFIG_JSON = `{
  "mcpServers": {
    "rhizome": {
      "command": "rhizome",
      "args": ["mcp", "--store-path", "~/.rhizome"]
    }
  }
}`;

export function McpCard() {
    return (
        <SettingsCard title="MCP 服务">
            <div className="flex flex-col gap-4">
                <div className="flex items-center gap-3">
                    <button
                        type="button"
                        role="switch"
                        aria-checked={false}
                        aria-label="MCP 服务开关"
                        disabled
                        className="inline-flex h-5 w-9 items-center rounded-full bg-neutral-200 opacity-50"
                    >
                        <span className="ml-0.5 h-4 w-4 rounded-full bg-white shadow" />
                    </button>
                    <span className="flex-1 text-sm text-neutral-950">未实现</span>
                </div>
                <pre className="overflow-x-auto rounded-lg bg-neutral-100 p-3.5 font-mono text-xs leading-6 text-neutral-950">
                    {MCP_CONFIG_JSON}
                </pre>
            </div>
        </SettingsCard>
    );
}
