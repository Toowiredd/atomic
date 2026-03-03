import { useState } from 'react';
import { Button } from '../../ui/Button';
import { QRCode } from '../QRCode';
import { createApiToken } from '../../../lib/api';
import { isDesktopApp, getLocalServerConfig } from '../../../lib/transport';
import type { HttpTransport } from '../../../lib/transport/http';
import { getTransport } from '../../../lib/transport';
import type { OnboardingState, OnboardingAction } from '../useOnboardingState';

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

interface MobileSetupStepProps {
  state: OnboardingState;
  dispatch: React.Dispatch<OnboardingAction>;
}

export function MobileSetupStep({ state, dispatch }: MobileSetupStepProps) {
  const [isGenerating, setIsGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const getServerBaseUrl = () => {
    if (isDesktopApp()) {
      const localConfig = getLocalServerConfig();
      return localConfig?.baseUrl || 'http://127.0.0.1:44380';
    }
    const transport = getTransport() as HttpTransport;
    return transport.getConfig().baseUrl;
  };

  const handleGenerateQR = async () => {
    setIsGenerating(true);
    setError(null);
    try {
      const result = await createApiToken('mobile-setup');
      dispatch({ type: 'SET_MOBILE_TOKEN', token: result.token });
    } catch (e) {
      setError(String(e));
    } finally {
      setIsGenerating(false);
    }
  };

  const qrPayload = state.mobileToken
    ? JSON.stringify({ url: getServerBaseUrl(), token: state.mobileToken })
    : null;

  const handleCopy = async () => {
    if (!qrPayload) return;
    await copyToClipboard(qrPayload);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="space-y-5 px-2">
      <div className="text-center mb-4">
        <h2 className="text-xl font-bold text-[var(--color-text-primary)] mb-1">Mobile App</h2>
        <p className="text-sm text-[var(--color-text-secondary)]">
          Connect the Atomic iOS app by scanning a QR code
        </p>
      </div>

      {!state.mobileToken ? (
        <div className="flex flex-col items-center space-y-4">
          <div className="p-4 bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-lg text-center space-y-3 w-full">
            <p className="text-sm text-[var(--color-text-secondary)]">
              Generate a QR code containing your server URL and a new API token. Scan it with the Atomic iOS app to connect instantly.
            </p>
            <Button onClick={handleGenerateQR} disabled={isGenerating}>
              {isGenerating ? 'Generating...' : 'Generate QR Code'}
            </Button>
          </div>
          {error && (
            <p className="text-sm text-red-500">{error}</p>
          )}
        </div>
      ) : (
        <div className="flex flex-col items-center space-y-4">
          <div className="p-6 bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-lg">
            <QRCode value={qrPayload!} size={220} />
          </div>

          <p className="text-sm text-[var(--color-text-secondary)] text-center">
            Open the Atomic iOS app and tap <strong className="text-[var(--color-text-primary)]">Scan QR Code</strong> on the setup screen
          </p>

          <div className="w-full space-y-2">
            <div className="flex items-center gap-2 text-xs text-[var(--color-text-secondary)]">
              <div className="flex-1 h-px bg-[var(--color-border)]" />
              <span>or copy manually</span>
              <div className="flex-1 h-px bg-[var(--color-border)]" />
            </div>

            <div className="flex gap-2">
              <code className="flex-1 px-3 py-2 bg-[var(--color-bg-main)] border border-[var(--color-border)] rounded-md text-xs text-[var(--color-text-primary)] truncate">
                {getServerBaseUrl()}
              </code>
              <Button variant="secondary" size="sm" onClick={handleCopy}>
                {copied ? 'Copied!' : 'Copy'}
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
