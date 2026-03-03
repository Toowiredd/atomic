export function ConnectionStatus({ status, error }: { status: 'checking' | 'connected' | 'disconnected'; error?: string }) {
  return (
    <div className="flex items-center gap-2 text-sm">
      {status === 'checking' && (
        <>
          <svg className="w-4 h-4 animate-spin text-[var(--color-text-secondary)]" fill="none" viewBox="0 0 24 24">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
          </svg>
          <span className="text-[var(--color-text-secondary)]">Checking connection...</span>
        </>
      )}
      {status === 'connected' && (
        <>
          <div className="w-2 h-2 rounded-full bg-green-500" />
          <span className="text-green-500">Connected</span>
        </>
      )}
      {status === 'disconnected' && (
        <>
          <div className="w-2 h-2 rounded-full bg-red-500" />
          <span className="text-red-500">{error || 'Not connected'}</span>
        </>
      )}
    </div>
  );
}
