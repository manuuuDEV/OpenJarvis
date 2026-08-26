import { describe, expect, it } from 'vitest';
import { selectApiBase } from './api';

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
