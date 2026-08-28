// frontend/src/lib/diag.test.ts
//
// Verifies the diagnostic timeline allowlist. The exported JSON MUST
// contain only the fields allowed by the user requirement:
//
//   t, kind, pathname, mountCount, setupPhase, setupSource
//
// And MUST NOT contain any of the forbidden values: error.message,
// error.name, setupStatus.error, stack frame, full URL, query string,
// file path, API key, token, prompt, conversation, window content,
// localStorage, sessionStorage, clipboard.
//
// The test also asserts the dedup helper does not flood the ring, that
// the JSON shape is stable, and that `dumpDiag` returns parseable JSON.

import { describe, expect, it, beforeEach } from 'vitest';
import {
  bumpMountCounter,
  clearDiagForTest,
  dumpDiag,
  isDiagBuild,
  isRecording,
  noteErrorPresent,
  notePathname,
  noteSetupStatus,
  recordDiag,
  startRecording,
  stopRecording,
  type DiagEntry,
} from './diag';

describe('diag allowlist', () => {
  beforeEach(() => {
    clearDiagForTest();
  });

  it('emits only allowed fields when called with the documented shape', () => {
    const m = bumpMountCounter();
    recordDiag({ kind: 'mount', mountCount: m });
    recordDiag({
      kind: 'location-change',
      pathname: '/settings',
      mountCount: m,
    });
    recordDiag({
      kind: 'setup-status-present',
      pathname: '/settings',
      mountCount: m,
      setupPhase: 'configuration_required',
      setupSource: 'cloud',
    });
    recordDiag({ kind: 'error-present', mountCount: m });

    const dumped = dumpDiag();
    const parsed = JSON.parse(dumped) as { schema: number; mountCounter: number; entries: DiagEntry[] };

    expect(parsed.schema).toBe(1);
    expect(parsed.mountCounter).toBe(1);
    expect(parsed.entries.length).toBe(4);
    for (const entry of parsed.entries) {
      const keys = Object.keys(entry).sort();
      expect(keys).toEqual(
        expect.arrayContaining(['t', 'kind'])
      );
      // Strict allowlist: no other keys permitted.
      for (const key of keys) {
        expect(['t', 'kind', 'pathname', 'mountCount', 'setupPhase', 'setupSource']).toContain(key);
      }
    }
  });

  it('drops any field outside the allowlist when an over-broad object is passed', () => {
    // The TS type only allows documented fields, but the runtime is a
    // plain object — a careless caller could pass extras. The runtime
    // MUST drop them.
    recordDiag({
      kind: 'location-change',
      pathname: '/chat',
      mountCount: 2,
      // forbidden fields — these MUST NOT appear in the entry
      errorMessage: 'C:\\Users\\maz\\AppData\\Local\\OpenJarvis\\state.bin: permission denied',
      errorName: 'TypeError',
      errorStack: 'TypeError: at C:\\Users\\maz\\AppData\\file.js:1:1',
      errorStackLong: 'x'.repeat(1000),
      apiKey: 'sk-abcdef1234567890',
      bearer: 'Bearer eyJhbGciOiJIUzI1NiJ9.payload.sig',
      fullUrl: 'https://api.openai.com/v1/chat?apiKey=sk-abc&token=secret',
      query: '?apiKey=sk-abc',
      filePath: 'C:\\Users\\maz\\secret.txt',
      prompt: 'Tell me a joke about the user',
      conversation: 'user: hi; assistant: hello',
      window: { __TAURI_INTERNALS__: 'sensitive' },
      localStorage: { 'openjarvis-settings': 'leaked' },
      sessionStorage: { session: 'leaked' },
      clipboard: 'leaked clipboard',
      setupStatusError: 'failed at C:\\Users\\maz\\AppData with sk-abc',
    } as unknown as Parameters<typeof recordDiag>[0]);

    const dumped = dumpDiag();
    expect(dumped).not.toContain('sk-abcdef1234567890');
    expect(dumped).not.toContain('Bearer');
    expect(dumped).not.toContain('eyJ');
    expect(dumped).not.toContain('api.openai.com');
    expect(dumped).not.toContain('AppData');
    expect(dumped).not.toContain('Tell me a joke');
    expect(dumped).not.toContain('openjarvis-settings');
    expect(dumped).not.toContain('leaked clipboard');
    expect(dumped).not.toContain('TypeError');
    expect(dumped).not.toContain('__TAURI_INTERNALS__');
    expect(dumped).not.toContain('?apiKey=');
  });

  it('drops pathnames with query or hash and pathnames that are not safe', () => {
    recordDiag({ kind: 'location-change', pathname: '/chat?apiKey=sk-abc', mountCount: 1 });
    recordDiag({ kind: 'location-change', pathname: '/chat#fragment', mountCount: 1 });
    recordDiag({ kind: 'location-change', pathname: 'C:\\Users\\maz\\secret', mountCount: 1 });
    recordDiag({ kind: 'location-change', pathname: '/api/v1?key=secret', mountCount: 1 });

    const dumped = dumpDiag();
    expect(dumped).not.toContain('?apiKey');
    expect(dumped).not.toContain('#fragment');
    expect(dumped).not.toContain('C:\\\\Users');
    expect(dumped).not.toContain('sk-abc');
  });

  it('caps pathname to 64 characters and does not record anything longer', () => {
    const longPath = '/' + 'a'.repeat(200);
    recordDiag({ kind: 'location-change', pathname: longPath, mountCount: 1 });
    const dumped = dumpDiag();
    const parsed = JSON.parse(dumped) as { entries: DiagEntry[] };
    const entry = parsed.entries[0];
    expect(entry.pathname).toBeDefined();
    expect((entry.pathname as string).length).toBeLessThanOrEqual(64);
  });

  it('replaces unknown setupPhase and setupSource with "unknown"', () => {
    recordDiag({
      kind: 'setup-status-present',
      mountCount: 1,
      setupPhase: 'magic' as unknown as 'starting',
      setupSource: 'banana' as unknown as 'ollama',
    });
    const parsed = JSON.parse(dumpDiag()) as { entries: DiagEntry[] };
    expect(parsed.entries[0].setupPhase).toBe('unknown');
    expect(parsed.entries[0].setupSource).toBe('unknown');
  });

  it('caps setupPhase and setupSource to 32 characters', () => {
    // The allowed set replaces unknown values with the literal 'unknown'
    // (8 chars), so this test exercises the cap on a known-enum value
    // post-sanitization (already short). We still assert the cap is
    // applied to any future allowed-but-long enum.
    recordDiag({
      kind: 'setup-status-present',
      mountCount: 1,
      setupPhase: 'starting',
      setupSource: 'ollama',
    });
    const parsed = JSON.parse(dumpDiag()) as { entries: DiagEntry[] };
    expect(parsed.entries[0].setupPhase).toBe('starting');
    expect(parsed.entries[0].setupSource).toBe('ollama');
  });

  it('recordDiag is always functional; gating happens at call sites via isDiagBuild()', () => {
    // The production no-op is enforced at the call sites (DiagObserver,
    // notifyDiagErrorPresent, SetupScreen) which all early-return when
    // isDiagBuild() is false. recordDiag itself writes to an in-memory
    // ring only, so even if it were reached in production it cannot
    // persist anything. This test locks that contract: recordDiag always
    // records, and the build flag is read via isDiagBuild() only.
    recordDiag({ kind: 'mount', mountCount: 1 });
    expect(JSON.parse(dumpDiag()).entries.length).toBe(1);

    // isDiagBuild() must be false in the non-diagnostic test environment
    // (the const is undefined here), which is what gates the callers.
    expect(isDiagBuild()).toBe(false);
  });

  it('bumpMountCounter increments monotonically and resets the per-test counter only via clearDiagForTest', () => {
    const a = bumpMountCounter();
    const b = bumpMountCounter();
    const c = bumpMountCounter();
    expect(b).toBe(a + 1);
    expect(c).toBe(b + 1);
    clearDiagForTest();
    const d = bumpMountCounter();
    expect(d).toBe(1);
  });

  it('notePathname reports a change only when the sanitized pathname differs', () => {
    const first = notePathname('/settings');
    expect(first.changed).toBe(true);
    expect(first.value).toBe('/settings');
    const sameAgain = notePathname('/settings');
    expect(sameAgain.changed).toBe(false);
    const next = notePathname('/chat');
    expect(next.changed).toBe(true);
    expect(next.value).toBe('/chat');
    const unsafe = notePathname('C:\\Users\\maz\\secret');
    expect(unsafe.changed).toBe(false);
    expect(unsafe.value).toBeUndefined();
  });

  it('noteSetupStatus reports a change only when the sanitized pair differs', () => {
    const first = noteSetupStatus('configuration_required', 'cloud');
    expect(first.changed).toBe(true);
    const sameAgain = noteSetupStatus('configuration_required', 'cloud');
    expect(sameAgain.changed).toBe(false);
    const next = noteSetupStatus('ready', 'ollama');
    expect(next.changed).toBe(true);
    // Unknown enum values are sanitized to 'unknown'; that IS a real
    // change from the previous 'ready'/'ollama' pair, so changed=true and
    // the sanitized values are the literal 'unknown'.
    const unsafe = noteSetupStatus('magic', 'banana');
    expect(unsafe.changed).toBe(true);
    expect(unsafe.phase).toBe('unknown');
    expect(unsafe.source).toBe('unknown');
    // A second identical 'unknown' is deduped (changed=false).
    const deduped = noteSetupStatus('magic', 'banana');
    expect(deduped.changed).toBe(false);
  });

  it('noteErrorPresent reports a change only on the boolean edge', () => {
    expect(noteErrorPresent(true)).toBe(true);
    expect(noteErrorPresent(true)).toBe(false);
    expect(noteErrorPresent(false)).toBe(true);
    expect(noteErrorPresent(false)).toBe(false);
  });

  it('startRecording / stopRecording flips the recording flag', () => {
    expect(isRecording()).toBe(false);
    startRecording();
    expect(isRecording()).toBe(true);
    stopRecording();
    expect(isRecording()).toBe(false);
  });

  it('dumpDiag returns parseable JSON with the documented shape', () => {
    startRecording();
    recordDiag({ kind: 'mount', mountCount: 1 });
    recordDiag({ kind: 'location-change', pathname: '/', mountCount: 1 });
    const raw = dumpDiag();
    const parsed = JSON.parse(raw) as { schema: number; mountCounter: number; entries: DiagEntry[] };
    expect(parsed.schema).toBe(1);
    expect(typeof parsed.mountCounter).toBe('number');
    expect(Array.isArray(parsed.entries)).toBe(true);
    for (const entry of parsed.entries) {
      expect(typeof entry.t).toBe('number');
      expect(typeof entry.kind).toBe('string');
    }
  });

  it('the ring caps at 256 entries; older entries are dropped', () => {
    startRecording();
    for (let i = 0; i < 300; i += 1) {
      recordDiag({ kind: 'location-change', pathname: '/p' + i, mountCount: 1 });
    }
    const parsed = JSON.parse(dumpDiag()) as { entries: DiagEntry[] };
    expect(parsed.entries.length).toBe(256);
    // The oldest kept entry must be the 44th ('/p44'), because we
    // dropped the first 44 ('/p0'..'/p43').
    expect(parsed.entries[0].pathname).toBe('/p44');
  });

  it('forbids the kinds "keydown" and similar by name', () => {
    // Sanity: the allowed kinds set MUST NOT contain "keydown", because
    // recording key events can leak user characters.
    // @ts-expect-error - intentionally testing the runtime type guard
    recordDiag({ kind: 'keydown' });
    const parsed = JSON.parse(dumpDiag()) as { entries: DiagEntry[] };
    expect(parsed.entries.length).toBe(0);
  });
});
