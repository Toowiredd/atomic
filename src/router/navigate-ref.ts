import type { NavigateFunction } from 'react-router-dom';

/// Store actions need to change the URL, but they run outside React and can't
/// call `useNavigate()` themselves. `RouterBridge` writes the live navigate
/// function here on mount so store actions can call `navigateTo(...)` without
/// a React context.
///
/// This is deliberately a module-scoped mutable — a single `RouterBridge`
/// lives for the app's lifetime, so the churn is nil and the indirection is
/// what lets Zustand actions drive routing without restructuring callers.
let navigateFn: NavigateFunction | null = null;

export function setNavigateFn(nav: NavigateFunction): void {
  navigateFn = nav;
}

/// Push a new URL. No-op if the router isn't mounted yet (e.g. during
/// onboarding the layout renders before RouterBridge).
export function navigateTo(to: string, options?: { replace?: boolean }): void {
  navigateFn?.(to, options);
}

/// Step back in browser history. If no history exists, fall back to the
/// provided URL — typically the base view so closing an overlay from a cold
/// deep-link lands the user somewhere sensible rather than exiting the app.
export function navigateBack(fallback: string = '/'): void {
  if (typeof window === 'undefined') return;
  // history.length > 1 isn't a perfect signal (it includes entries before the
  // app loaded), but for SPAs it's a decent proxy for "we have somewhere to
  // go back to". The fallback handles the edge case where we don't.
  if (window.history.length > 1) {
    window.history.back();
  } else {
    navigateFn?.(fallback, { replace: true });
  }
}

export function navigateForward(): void {
  if (typeof window === 'undefined') return;
  window.history.forward();
}
