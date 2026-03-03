import { Button } from '../../ui/Button';
import { isDesktopApp, getTransport, switchTransport } from '../../../lib/transport';
import type { OnboardingState, OnboardingAction } from '../useOnboardingState';

interface WelcomeStepProps {
  state: OnboardingState;
  dispatch: React.Dispatch<OnboardingAction>;
  onNext: () => void;
}

export function WelcomeStep({ state, dispatch, onNext }: WelcomeStepProps) {
  const isDesktop = isDesktopApp();

  const handleTestServer = async () => {
    if (!state.serverUrl.trim() || !state.serverToken.trim()) return;
    dispatch({ type: 'SET_TESTING_SERVER', value: true });
    dispatch({ type: 'SET_SERVER_TEST', result: null });
    try {
      const resp = await fetch(`${state.serverUrl.trim().replace(/\/$/, '')}/health`);
      if (resp.ok) {
        dispatch({ type: 'SET_SERVER_TEST', result: 'success' });
      } else {
        dispatch({ type: 'SET_SERVER_TEST', result: 'error', error: `Server returned ${resp.status}` });
      }
    } catch (e) {
      dispatch({ type: 'SET_SERVER_TEST', result: 'error', error: String(e) });
    } finally {
      dispatch({ type: 'SET_TESTING_SERVER', value: false });
    }
  };

  const handleConnect = async () => {
    try {
      await switchTransport({
        baseUrl: state.serverUrl.trim().replace(/\/$/, ''),
        authToken: state.serverToken.trim(),
      });
      onNext();
    } catch (e) {
      dispatch({ type: 'SET_SERVER_TEST', result: 'error', error: String(e) });
    }
  };

  if (isDesktop) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-center space-y-6 px-8">
        <div className="w-16 h-16 rounded-2xl bg-[var(--color-accent)]/10 flex items-center justify-center">
          <svg className="w-8 h-8 text-[var(--color-accent)]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 6.042A8.967 8.967 0 006 3.75c-1.052 0-2.062.18-3 .512v14.25A8.987 8.987 0 016 18c2.305 0 4.408.867 6 2.292m0-14.25a8.966 8.966 0 016-2.292c1.052 0 2.062.18 3 .512v14.25A8.987 8.987 0 0018 18a8.967 8.967 0 00-6 2.292m0-14.25v14.25" />
          </svg>
        </div>

        <div>
          <h2 className="text-2xl font-bold text-[var(--color-text-primary)] mb-2">
            Welcome to Atomic
          </h2>
          <p className="text-[var(--color-text-secondary)] max-w-md">
            Your personal knowledge base that turns freeform notes into a semantically-connected, AI-augmented knowledge graph.
          </p>
        </div>

        <div className="bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-lg p-4 text-left max-w-md w-full">
          <h3 className="text-sm font-medium text-[var(--color-text-primary)] mb-2">What you'll set up:</h3>
          <ul className="space-y-1.5 text-sm text-[var(--color-text-secondary)]">
            <li className="flex items-center gap-2">
              <span className="text-[var(--color-accent)]">1.</span> AI provider for embeddings, tagging & chat
            </li>
            <li className="flex items-center gap-2">
              <span className="text-[var(--color-accent)]">2.</span> Optional integrations (MCP, mobile, browser extension)
            </li>
            <li className="flex items-center gap-2">
              <span className="text-[var(--color-accent)]">3.</span> Import existing notes or start fresh
            </li>
          </ul>
        </div>

        <p className="text-xs text-[var(--color-text-secondary)]">
          Required steps are marked. You can skip optional steps and configure them later in Settings.
        </p>
      </div>
    );
  }

  // Web mode: needs server connection
  const isConnected = getTransport().isConnected();

  if (isConnected) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-center space-y-6 px-8">
        <div className="w-16 h-16 rounded-2xl bg-green-500/10 flex items-center justify-center">
          <svg className="w-8 h-8 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
          </svg>
        </div>
        <div>
          <h2 className="text-2xl font-bold text-[var(--color-text-primary)] mb-2">Connected</h2>
          <p className="text-[var(--color-text-secondary)]">You're connected to an Atomic server. Let's configure your AI provider.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6 px-2">
      <div className="text-center mb-6">
        <h2 className="text-xl font-bold text-[var(--color-text-primary)] mb-1">Connect to Atomic Server</h2>
        <p className="text-sm text-[var(--color-text-secondary)]">
          Enter the URL and auth token of your running atomic-server
        </p>
      </div>

      <div className="space-y-4">
        <div className="space-y-1.5">
          <label className="block text-sm font-medium text-[var(--color-text-primary)]">Server URL</label>
          <input
            type="text"
            value={state.serverUrl}
            onChange={(e) => dispatch({ type: 'SET_SERVER_URL', value: e.target.value })}
            placeholder="http://localhost:8080"
            className="w-full px-3 py-2 bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-md text-[var(--color-text-primary)] placeholder-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)] focus:border-transparent transition-colors duration-150 text-sm"
          />
        </div>

        <div className="space-y-1.5">
          <label className="block text-sm font-medium text-[var(--color-text-primary)]">Auth Token</label>
          <input
            type="password"
            value={state.serverToken}
            onChange={(e) => dispatch({ type: 'SET_SERVER_TOKEN', value: e.target.value })}
            placeholder="Token (printed by server on startup)"
            className="w-full px-3 py-2 bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-md text-[var(--color-text-primary)] placeholder-[var(--color-text-secondary)] focus:outline-none focus:ring-2 focus:ring-[var(--color-accent)] focus:border-transparent transition-colors duration-150 text-sm"
          />
        </div>

        <div className="flex gap-2">
          <Button variant="secondary" onClick={handleTestServer} disabled={!state.serverUrl.trim() || !state.serverToken.trim() || state.isTestingServer}>
            {state.isTestingServer ? 'Testing...' : 'Test Connection'}
          </Button>
          <Button onClick={handleConnect} disabled={state.serverTestResult !== 'success'}>
            Connect
          </Button>
        </div>

        {state.serverTestResult === 'success' && (
          <div className="text-sm text-green-500">Server reachable</div>
        )}
        {state.serverTestResult === 'error' && (
          <div className="text-sm text-red-500">{state.serverTestError}</div>
        )}

        <div className="p-3 bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-md text-xs text-[var(--color-text-secondary)] space-y-1">
          <p>Start the server with:</p>
          <code className="block text-[var(--color-text-primary)]">cargo run -p atomic-server -- --db-path /path/to/atomic.db serve</code>
          <p>The auth token is printed to stdout on startup.</p>
        </div>
      </div>
    </div>
  );
}
