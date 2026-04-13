import { useState, useEffect } from 'react';
import { Button } from '../../ui/Button';
import { getMcpStdioConfig, getMcpHttpConfig, createApiToken, type McpConfig } from '../../../lib/api';
import { isDesktopApp, isLocalServer, getMcpBridgePath, getTransport } from '../../../lib/transport';
import type { HttpTransport } from '../../../lib/transport/http';

function copyToClipboard(text: string) {
  if (navigator.clipboard && window.isSecureContext) {
    return navigator.clipboard.writeText(text);
  }
  const textarea = document.createElement('textarea');
  textarea.value = text;
  textarea.style.position = 'fixed';
  textarea.style.opacity = '0';
  document.body.appendChild(textarea);
  textarea.select();
  document.execCommand('copy');
  document.body.removeChild(textarea);
  return Promise.resolve();
}

export function McpSetupStep() {
  const [mcpConfig, setMcpConfig] = useState<McpConfig | null>(null);
  const [copied, setCopied] = useState(false);
  const [isCreating, setIsCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const isLocal = isDesktopApp() && isLocalServer();

  useEffect(() => {
    if (isLocal) {
      getMcpBridgePath().then((path) => {
        if (path) setMcpConfig(getMcpStdioConfig(path));
        else setError('Could not locate atomic-mcp-bridge. Ensure the app bundle is complete.');
      });
    }
  }, [isLocal]);

  const handleCreateToken = async () => {
    setIsCreating(true);
    setError(null);
    try {
      const result = await createApiToken('mcp-integration');
      const transport = getTransport() as HttpTransport;
      setMcpConfig(getMcpHttpConfig(transport.getConfig().baseUrl, result.token));
    } catch (e) {
      setError(String(e));
    } finally {
      setIsCreating(false);
    }
  };

  const handleCopy = async () => {
    if (!mcpConfig) return;
    await copyToClipboard(JSON.stringify(mcpConfig, null, 2));
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const configJson = mcpConfig ? JSON.stringify(mcpConfig, null, 2) : '';

  return (
    <div className="space-y-5 px-2">
      <div className="text-center mb-4">
        <h2 className="text-xl font-bold text-[var(--color-text-primary)] mb-1">MCP Integration</h2>
        <p className="text-sm text-[var(--color-text-secondary)]">
          Connect AI assistants to your knowledge base via MCP
        </p>
      </div>

      <div className="space-y-4">
        {isLocal ? (
          <>
            <p className="text-sm text-[var(--color-text-secondary)]">
              The Atomic MCP bridge is bundled with the desktop app and connects to the local server automatically.
            </p>
            <div className="p-4 bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-lg space-y-3">
              <h3 className="text-sm font-medium text-[var(--color-text-primary)]">Setup Instructions</h3>
              <ol className="space-y-2 text-sm text-[var(--color-text-secondary)] list-decimal list-inside">
                <li>Open your MCP client settings (e.g. Claude Desktop &gt; <span className="text-[var(--color-text-primary)]">Developer &gt; Edit Config</span>)</li>
                <li>Add the following to your configuration file:</li>
              </ol>
            </div>
            <div className="relative">
              <pre className="p-4 bg-[var(--color-bg-main)] border border-[var(--color-border)] rounded-lg text-sm text-[var(--color-text-primary)] overflow-x-auto font-mono">
                {configJson || 'Loading...'}
              </pre>
              <Button variant="secondary" size="sm" onClick={handleCopy} className="absolute top-2 right-2" disabled={!mcpConfig}>
                {copied ? 'Copied!' : 'Copy'}
              </Button>
            </div>
          </>
        ) : !mcpConfig ? (
          <>
            <p className="text-sm text-[var(--color-text-secondary)]">
              Connect your MCP client to this Atomic server's HTTP endpoint. A dedicated API token is required.
            </p>
            <Button variant="secondary" onClick={handleCreateToken} disabled={isCreating}>
              {isCreating ? 'Creating...' : 'Create MCP Token'}
            </Button>
            {error && <p className="text-sm text-red-500">{error}</p>}
          </>
        ) : (
          <>
            <div className="p-3 bg-amber-500/10 border border-amber-500/30 rounded-md text-xs text-amber-400">
              Save this config now — the token won't be shown again.
            </div>
            <div className="p-4 bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-lg space-y-3">
              <h3 className="text-sm font-medium text-[var(--color-text-primary)]">Setup Instructions</h3>
              <ol className="space-y-2 text-sm text-[var(--color-text-secondary)] list-decimal list-inside">
                <li>Open your MCP client settings (e.g. Claude Desktop &gt; <span className="text-[var(--color-text-primary)]">Developer &gt; Edit Config</span>)</li>
                <li>Add the following to your configuration file:</li>
              </ol>
            </div>
            <div className="relative">
              <pre className="p-4 bg-[var(--color-bg-main)] border border-[var(--color-border)] rounded-lg text-sm text-[var(--color-text-primary)] overflow-x-auto font-mono">
                {configJson}
              </pre>
              <Button variant="secondary" size="sm" onClick={handleCopy} className="absolute top-2 right-2">
                {copied ? 'Copied!' : 'Copy'}
              </Button>
            </div>
          </>
        )}

        <div className="p-3 bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-md text-xs text-[var(--color-text-secondary)]">
          <p>After saving, restart your MCP client. Atomic will appear as an available MCP tool.</p>
        </div>
      </div>
    </div>
  );
}
