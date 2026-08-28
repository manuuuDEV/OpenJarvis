/// <reference types="vite/client" />

// Diagnostic build flag. Set at build time only via the OPENJARVIS_BUILD
// env var in vite.config.ts. When true, the diagnostic UI in
// `frontend/src/components/DiagObserver.tsx` and the "Diagnostics" section
// in `frontend/src/components/SettingsPage.tsx` are compiled into the
// bundle. A diagnostic build is a separate version (e.g. 1.0.12-diag.1)
// distinct from the shipping version. Production builds leave this false.
declare const __OPENJARVIS_DIAG_BUILD__: boolean;
declare const __OPENJARVIS_DESKTOP_BUILD__: boolean;

interface ImportMetaEnv {
  readonly VITE_API_URL?: string;
  readonly VITE_SUPABASE_URL?: string;
  readonly VITE_SUPABASE_ANON_KEY?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
