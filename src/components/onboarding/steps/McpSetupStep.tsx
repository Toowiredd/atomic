import { useState, useEffect } from 'react';
import { Button } from '../../ui/Button';
import { getMcpConfig, type McpConfig } from '../../../lib/api';
import { isDesktopApp, getLocalServerConfig } from '../../../lib/transport';
import type { HttpTransport } from '../../../lib/transport/http';
import { getTransport } from '../../../lib/transport';

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

  useEffect(() => {
    let baseUrl: string;
    if (isDesktopApp()) {
      const localConfig = getLocalServerConfig();
      baseUrl = localConfig?.baseUrl || 'http://127.0.0.1:44380';
    } else {
      const transport = getTransport() as HttpTransport;
      baseUrl = transport.getConfig().baseUrl;
    }
    const config = getMcpConfig(baseUrl);
    setMcpConfig(config);
  }, []);

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
        <h2 className="text-xl font-bold text-[var(--color-text-primary)] mb-1">Claude Desktop Integration</h2>
        <p className="text-sm text-[var(--color-text-secondary)]">
          Connect Atomic as an MCP server so Claude can search your knowledge base
        </p>
      </div>

      <div className="space-y-4">
        <div className="p-4 bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-lg space-y-3">
          <h3 className="text-sm font-medium text-[var(--color-text-primary)]">Setup Instructions</h3>
          <ol className="space-y-2 text-sm text-[var(--color-text-secondary)] list-decimal list-inside">
            <li>Open Claude Desktop settings</li>
            <li>Navigate to <span className="text-[var(--color-text-primary)]">Developer &gt; Edit Config</span></li>
            <li>Add the following to your configuration file:</li>
          </ol>
        </div>

        <div className="relative">
          <pre className="p-4 bg-[var(--color-bg-main)] border border-[var(--color-border)] rounded-lg text-sm text-[var(--color-text-primary)] overflow-x-auto font-mono">
            {configJson}
          </pre>
          <Button
            variant="secondary"
            size="sm"
            onClick={handleCopy}
            className="absolute top-2 right-2"
          >
            {copied ? 'Copied!' : 'Copy'}
          </Button>
        </div>

        <div className="p-3 bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-md text-xs text-[var(--color-text-secondary)]">
          <p>After saving the config, restart Claude Desktop. Atomic will appear as an available MCP tool.</p>
        </div>
      </div>
    </div>
  );
}
