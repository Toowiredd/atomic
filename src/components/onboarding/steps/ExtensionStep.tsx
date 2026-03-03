import { useState } from 'react';
import { Button } from '../../ui/Button';
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

export function ExtensionStep() {
  const [copiedUrl, setCopiedUrl] = useState(false);
  const [copiedToken, setCopiedToken] = useState(false);

  const getServerInfo = () => {
    if (isDesktopApp()) {
      const localConfig = getLocalServerConfig();
      return {
        url: localConfig?.baseUrl || 'http://127.0.0.1:44380',
        token: localConfig?.authToken || '',
      };
    }
    const transport = getTransport() as HttpTransport;
    const config = transport.getConfig();
    return { url: config.baseUrl, token: config.authToken };
  };

  const serverInfo = getServerInfo();

  const handleCopyUrl = async () => {
    await copyToClipboard(serverInfo.url);
    setCopiedUrl(true);
    setTimeout(() => setCopiedUrl(false), 2000);
  };

  const handleCopyToken = async () => {
    await copyToClipboard(serverInfo.token);
    setCopiedToken(true);
    setTimeout(() => setCopiedToken(false), 2000);
  };

  return (
    <div className="space-y-5 px-2">
      <div className="text-center mb-4">
        <h2 className="text-xl font-bold text-[var(--color-text-primary)] mb-1">Browser Extension</h2>
        <p className="text-sm text-[var(--color-text-secondary)]">
          Save web pages to your knowledge base with one click
        </p>
      </div>

      <div className="space-y-4">
        <div className="p-4 bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-lg space-y-3">
          <h3 className="text-sm font-medium text-[var(--color-text-primary)]">Setup Instructions</h3>
          <ol className="space-y-2 text-sm text-[var(--color-text-secondary)] list-decimal list-inside">
            <li>Install the Atomic browser extension from your browser's extension store</li>
            <li>Click the extension icon and open settings</li>
            <li>Enter the server URL and auth token below</li>
          </ol>
        </div>

        <div className="space-y-3">
          <div className="space-y-1.5">
            <label className="block text-xs font-medium text-[var(--color-text-secondary)]">Server URL</label>
            <div className="flex gap-2">
              <code className="flex-1 px-3 py-2 bg-[var(--color-bg-main)] border border-[var(--color-border)] rounded-md text-sm text-[var(--color-text-primary)] truncate">
                {serverInfo.url}
              </code>
              <Button variant="secondary" size="sm" onClick={handleCopyUrl}>
                {copiedUrl ? 'Copied!' : 'Copy'}
              </Button>
            </div>
          </div>

          <div className="space-y-1.5">
            <label className="block text-xs font-medium text-[var(--color-text-secondary)]">Auth Token</label>
            <div className="flex gap-2">
              <code className="flex-1 px-3 py-2 bg-[var(--color-bg-main)] border border-[var(--color-border)] rounded-md text-sm text-[var(--color-text-primary)] truncate">
                {serverInfo.token ? `${serverInfo.token.substring(0, 12)}...` : 'N/A'}
              </code>
              <Button variant="secondary" size="sm" onClick={handleCopyToken} disabled={!serverInfo.token}>
                {copiedToken ? 'Copied!' : 'Copy'}
              </Button>
            </div>
          </div>
        </div>

        <div className="p-3 bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-md text-xs text-[var(--color-text-secondary)]">
          The extension uses the same REST API as the web interface. You can create a dedicated API token for the extension in Settings &gt; Connection after setup.
        </div>
      </div>
    </div>
  );
}
