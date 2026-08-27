import { describe, expect, it } from 'vitest';
import {
  BUNDLED_DESKTOP_PROFILE,
  detectTauriRuntime,
  resolveDesktopRuntime,
  selectApiBase,
} from './api';

describe('detectTauriRuntime', () => {
  it('recognizes the official Tauri global when an internal bridge is not exposed', () => {
    expect(detectTauriRuntime({ __TAURI__: {} } as Window)).toBe(true);
  });

  it('recognizes the legacy internal bridge and rejects a normal browser runtime', () => {
    expect(detectTauriRuntime({ __TAURI_INTERNALS__: {} } as Window)).toBe(true);
    expect(detectTauriRuntime(undefined)).toBe(false);
    expect(detectTauriRuntime({} as Window)).toBe(false);
  });
});

describe('compiled desktop profile', () => {
  it('matches the expected build profile', () => {
    expect(BUNDLED_DESKTOP_PROFILE).toBe(
      import.meta.env.VITE_EXPECT_DESKTOP_PROFILE === 'true',
    );
  });
});

describe('resolveDesktopRuntime', () => {
  it('uses the desktop profile compiled into the Tauri bundle without runtime globals', () => {
    expect(resolveDesktopRuntime(true, undefined)).toBe(true);
    expect(resolveDesktopRuntime(false, undefined)).toBe(false);
  });
});

describe('selectApiBase', () => {
  it('keeps the packaged desktop on its native local backend despite a legacy remote setting', () => {
    expect(
      selectApiBase(
        true,
        'http://127.0.0.1:48123',
        'https://untrusted.example.invalid',
        'https://build.example.invalid',
      ),
    ).toBe('http://127.0.0.1:48123');
  });

  it('uses the local fallback until the native backend publishes its actual port', () => {
    expect(selectApiBase(true, null, 'https://untrusted.example.invalid')).toBe(
      'http://127.0.0.1:8000',
    );
  });

  it('retains the upstream web behavior outside the packaged desktop', () => {
    expect(
      selectApiBase(
        false,
        null,
        'https://configured.example.invalid',
        'https://build.example.invalid',
      ),
    ).toBe('https://configured.example.invalid');
  });
});
