// frontend/src/lib/diag.ts
//
// Renderer-side diagnostic instrumentation for the 1.0.12-diag.1 build.
//
// Hard allowlist. The exported timeline contains ONLY these fields:
//   - t:        relative timestamp in ms since renderer boot
//   - kind:     enumerated ("mount" | "unmount" | "location-change" |
//                          "popstate" | "hashchange" | "beforeunload" |
//                          "pagehide" | "visibilitychange" |
//                          "setup-status-present" | "error-present")
//   - pathname: location.pathname, max 64 chars, NO query / hash
//   - mountCount: integer
//   - setupPhase, setupSource: enumerated strings, max 32 chars each
//
// Everything else is excluded by construction: this module never reads
// `error.message`, `error.name`, stack frames, `setupStatus.error`,
// `localStorage`, `sessionStorage`, the clipboard, the `__TAURI__` internals,
// the `window` global, full URLs, query strings, or file paths. Callers
// MUST NOT pass those values into `recordDiag`. The companion test
// `frontend/src/lib/diag.test.ts` asserts the absence of those fields in
// any emitted entry.
//
// The buffer is module-level, capped at 256 entries (~24 KB at 96 B per
// entry). `dumpDiag()` returns a JSON string ready for the
// `URL.createObjectURL(new Blob(...))` download pattern used by
// `SettingsPage.tsx` for the "Export conversations" button. No Tauri
// dialog plugin is involved; the dialog plugin is not wired on the Rust
// side (Cargo.toml / capabilities/default.json), so the existing web
// `Blob` + `a.click()` pattern is the minimal compatible mechanism.

declare const __OPENJARVIS_DIAG_BUILD__: boolean;

export type DiagKind =
  | 'mount'
  | 'unmount'
  | 'location-change'
  | 'popstate'
  | 'hashchange'
  | 'beforeunload'
  | 'pagehide'
  | 'visibilitychange'
  | 'setup-status-present'
  | 'error-present';

export type DiagEntry = {
  t: number;
  kind: DiagKind;
  pathname?: string;
  mountCount?: number;
  setupPhase?: string;
  setupSource?: string;
};

const RING_CAP = 256;
const PATHNAME_CAP = 64;
const PHASE_CAP = 32;
const SOURCE_CAP = 32;

const ring: DiagEntry[] = [];
let recording = false;
let bootOriginMs: number | null = null;
let mountCounter = 0;
let lastPathname: string | null = null;
let lastSetupPhase: string | null = null;
let lastSetupSource: string | null = null;
let lastErrorPresent = false;

// Pure allowlist enums. Anything not on these lists is dropped.
const ALLOWED_KINDS: ReadonlySet<DiagKind> = new Set<DiagKind>([
  'mount',
  'unmount',
  'location-change',
  'popstate',
  'hashchange',
  'beforeunload',
  'pagehide',
  'visibilitychange',
  'setup-status-present',
  'error-present',
]);

// Allowed phase values from the Rust `SetupStatus` (lib.rs) and frontend
// `SetupStatus` (lib/api.ts). Anything outside this set is replaced with
// the literal `'unknown'` to keep the timeline a small enum.
const ALLOWED_PHASES: ReadonlySet<string> = new Set<string>([
  'starting',
  'ready',
  'configuration_required',
  'unknown',
]);

// Allowed source values from the Rust `SetupStatus.source` and the
// frontend `SetupStatus.source` (lib/api.ts).
const ALLOWED_SOURCES: ReadonlySet<string> = new Set<string>([
  'ollama',
  'custom',
  'cloud',
  'unknown',
]);

function cap(value: string, max: number): string {
  if (value.length <= max) return value;
  return value.slice(0, max);
}

function sanitizePathname(raw: unknown): string | undefined {
  if (typeof raw !== 'string' || raw.length === 0) return undefined;
  // Strip query and hash explicitly. The user requirement is "only
  // pathname without query".
  const qIdx = raw.indexOf('?');
  const hIdx = raw.indexOf('#');
  const cut = [qIdx, hIdx].filter((i) => i >= 0).sort((a, b) => a - b)[0];
  const pathOnly = cut === undefined ? raw : raw.slice(0, cut);
  // Restrict to safe characters: leading '/', alphanumerics, '-', '_',
  // '.', '/'. This is a defence-in-depth check; React Router pathnames
  // already match this shape.
  const safe = /^\/[A-Za-z0-9._\-/]*$/.test(pathOnly);
  if (!safe) return undefined;
  return cap(pathOnly, PATHNAME_CAP);
}

function sanitizeEnum(
  value: unknown,
  allowed: ReadonlySet<string>,
  capSize: number,
): string | undefined {
  if (typeof value !== 'string' || value.length === 0) return undefined;
  const v = allowed.has(value) ? value : 'unknown';
  return cap(v, capSize);
}

/**
 * Record a single entry into the ring buffer. The argument is the ONLY
 * surface that can populate the buffer; callers MUST construct the object
 * from already-typed sources, and MUST NOT include any field outside the
 * allowlist. Any field not on the allowlist is silently dropped.
 *
 * This function is NOT itself gated on the build flag: it writes only to
 * an in-memory ring (max 256 entries) and is never persisted unless the
 * gated `Settings -> Diagnostics -> Download JSON` button is clicked. The
 * build flag gating happens at the call sites — `DiagObserver` effects,
 * `ErrorBoundary.componentDidCatch` (via `notifyDiagErrorPresent`), and
 * `SetupScreen` all early-return when `isDiagBuild()` is false. Keeping
 * `recordDiag` always-functional makes the allowlist unit-testable: the
 * test `diag.test.ts` exercises the real recording path and asserts no
 * forbidden value ever reaches the buffer.
 */
export function recordDiag(input: {
  kind: DiagKind;
  pathname?: unknown;
  mountCount?: number;
  setupPhase?: unknown;
  setupSource?: unknown;
}): void {
  if (!ALLOWED_KINDS.has(input.kind)) {
    return;
  }
  if (bootOriginMs === null) {
    bootOriginMs = performance.now();
  }
  const entry: DiagEntry = { t: Math.round(performance.now() - bootOriginMs), kind: input.kind };
  const pathname = sanitizePathname(input.pathname);
  if (pathname !== undefined) entry.pathname = pathname;
  if (typeof input.mountCount === 'number' && Number.isFinite(input.mountCount) && input.mountCount >= 0) {
    entry.mountCount = Math.floor(input.mountCount);
  }
  const phase = sanitizeEnum(input.setupPhase, ALLOWED_PHASES, PHASE_CAP);
  if (phase !== undefined) entry.setupPhase = phase;
  const source = sanitizeEnum(input.setupSource, ALLOWED_SOURCES, SOURCE_CAP);
  if (source !== undefined) entry.setupSource = source;
  ring.push(entry);
  if (ring.length > RING_CAP) ring.shift();
}

/**
 * Bump the mount counter and return the new value. Called by the
 * `DiagObserver` component on mount and again on unmount so a re-mount
 * (e.g. WebView2 reload) shows as a non-decreasing sequence with a
 * discontinuity at the remount.
 */
export function bumpMountCounter(): number {
  mountCounter += 1;
  return mountCounter;
}

/**
 * Internal state memoisation. The observer calls `notePathname`,
 * `noteSetupStatus`, `noteErrorPresent` with the latest values from the
 * renderer. `recordDiag` then deduplicates: a value equal to the previous
 * one is dropped, so a re-render that re-uses the same pathname does not
 * flood the ring. The memoised values are reset only on `clearDiag()`.
 */
export function notePathname(value: unknown): { changed: boolean; value?: string } {
  const v = sanitizePathname(value);
  if (v === undefined) return { changed: false };
  if (v === lastPathname) return { changed: false, value: v };
  lastPathname = v;
  return { changed: true, value: v };
}

export function noteSetupStatus(phase: unknown, source: unknown): {
  changed: boolean;
  phase?: string;
  source?: string;
} {
  const p = sanitizeEnum(phase, ALLOWED_PHASES, PHASE_CAP);
  const s = sanitizeEnum(source, ALLOWED_SOURCES, SOURCE_CAP);
  if (p === undefined || s === undefined) return { changed: false };
  const changed = p !== lastSetupPhase || s !== lastSetupSource;
  if (changed) {
    lastSetupPhase = p;
    lastSetupSource = s;
  }
  return { changed, phase: p, source: s };
}

export function noteErrorPresent(present: boolean): boolean {
  if (present === lastErrorPresent) return false;
  lastErrorPresent = present;
  return true;
}

export function startRecording(): void {
  recording = true;
  // Recording is on; existing entries are kept. A new run simply starts
  // appending. Reset the last-seen memoisation so a path that the previous
  // run already saw is recorded again.
  lastPathname = null;
  lastSetupPhase = null;
  lastSetupSource = null;
  lastErrorPresent = false;
}

export function stopRecording(): void {
  recording = false;
}

export function isRecording(): boolean {
  return recording;
}

export function isDiagBuild(): boolean {
  return typeof __OPENJARVIS_DIAG_BUILD__ !== 'undefined' && !!__OPENJARVIS_DIAG_BUILD__;
}

/**
 * Return the current ring as a JSON string. The string is ready to feed
 * into `URL.createObjectURL(new Blob([json], { type: 'application/json' }))`.
 * The shape is `{ schema: 1, mountCounter, entries: DiagEntry[] }`.
 */
export function dumpDiag(): string {
  const out = {
    schema: 1,
    mountCounter,
    entries: ring.slice(),
  };
  return JSON.stringify(out, null, 2);
}

/**
 * Test-only: clear the ring. Not exported in the production module's
 * surface; used by `diag.test.ts` to assert isolation between tests.
 */
export function clearDiagForTest(): void {
  ring.length = 0;
  recording = false;
  bootOriginMs = null;
  mountCounter = 0;
  lastPathname = null;
  lastSetupPhase = null;
  lastSetupSource = null;
  lastErrorPresent = false;
}
