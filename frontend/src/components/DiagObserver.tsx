// frontend/src/components/DiagObserver.tsx
//
// Passive diagnostic observer. Mounts once inside <App />, observes the
// current pathname via useLocation(), listens to browser navigation
// events (popstate, hashchange, beforeunload, pagehide,
// visibilitychange), and forwards an `error-present` event from
// ErrorBoundary. Does NOT modify any useNavigate callsite.
//
// Active only in the diagnostic build (__OPENJARVIS_DIAG_BUILD__ = true).
// In production builds the component renders nothing and `recordDiag`
// short-circuits internally.

import { useEffect } from 'react';
import { useLocation } from 'react-router';
import {
  bumpMountCounter,
  isDiagBuild,
  noteErrorPresent,
  notePathname,
  recordDiag,
} from '../lib/diag';

export function DiagObserver() {
  const location = useLocation();

  useEffect(() => {
    if (!isDiagBuild()) return;
    const mount = bumpMountCounter();
    recordDiag({ kind: 'mount', mountCount: mount });
    return () => {
      recordDiag({ kind: 'unmount', mountCount: mount });
    };
  }, []);

  useEffect(() => {
    if (!isDiagBuild()) return;
    const result = notePathname(location.pathname);
    if (result.changed && result.value !== undefined) {
      recordDiag({
        kind: 'location-change',
        pathname: result.value,
        mountCount: bumpMountCounter(),
      });
    }
  }, [location.pathname]);

  useEffect(() => {
    if (!isDiagBuild()) return;
    const handler = () => {
      // popstate fires after history.back/forward or after history.pushState
      // / replaceState in a parent component. React Router updates the
      // pathname synchronously, so we read it from the current
      // `window.location` rather than from a stale closure.
      const current = window.location.pathname;
      const result = notePathname(current);
      if (result.changed && result.value !== undefined) {
        recordDiag({
          kind: 'popstate',
          pathname: result.value,
        });
      }
    };
    window.addEventListener('popstate', handler);
    return () => window.removeEventListener('popstate', handler);
  }, []);

  useEffect(() => {
    if (!isDiagBuild()) return;
    const handler = () => {
      const current = window.location.pathname;
      const result = notePathname(current);
      if (result.changed && result.value !== undefined) {
        recordDiag({
          kind: 'hashchange',
          pathname: result.value,
        });
      }
    };
    window.addEventListener('hashchange', handler);
    return () => window.removeEventListener('hashchange', handler);
  }, []);

  useEffect(() => {
    if (!isDiagBuild()) return;
    const handler = () => {
      recordDiag({ kind: 'beforeunload' });
    };
    window.addEventListener('beforeunload', handler);
    return () => window.removeEventListener('beforeunload', handler);
  }, []);

  useEffect(() => {
    if (!isDiagBuild()) return;
    const handler = () => {
      recordDiag({ kind: 'pagehide' });
    };
    window.addEventListener('pagehide', handler);
    return () => window.removeEventListener('pagehide', handler);
  }, []);

  useEffect(() => {
    if (!isDiagBuild()) return;
    const handler = () => {
      recordDiag({ kind: 'visibilitychange' });
    };
    document.addEventListener('visibilitychange', handler);
    return () => document.removeEventListener('visibilitychange', handler);
  }, []);

  return null;
}

/**
 * Called by ErrorBoundary.componentDidCatch. Emits an `error-present`
 * entry — never the error message, name, or stack.
 */
export function notifyDiagErrorPresent(): void {
  if (!isDiagBuild()) return;
  if (noteErrorPresent(true)) {
    recordDiag({ kind: 'error-present' });
  }
}
