import { describe, expect, it } from 'vitest';
import {
  cloudConfigurationRequired,
  cloudProfileNeedsConfiguration,
  type SetupStatus,
} from './api';

function status(overrides: Partial<SetupStatus> = {}): SetupStatus {
  return {
    phase: 'starting',
    detail: 'Initializing...',
    ollama_ready: false,
    server_ready: false,
    model_ready: false,
    error: null,
    source: 'cloud',
    ...overrides,
  };
}

describe('cloudProfileNeedsConfiguration', () => {
  it('requires provider, model, and explicit processing acknowledgement before cloud boot', () => {
    expect(cloudProfileNeedsConfiguration({ kind: 'cloud' })).toBe(true);
    expect(
      cloudProfileNeedsConfiguration({
        kind: 'cloud',
        provider: 'openai',
        model: 'gpt-5-mini',
        providerProcessingAcknowledged: true,
      }),
    ).toBe(false);
  });
});

describe('cloudConfigurationRequired', () => {
  it('identifies an unconfigured cloud profile so first launch can open Settings', () => {
    expect(
      cloudConfigurationRequired(
        status({
          phase: 'configuration_required',
          error: 'Choose one authorized cloud provider and model in Settings.',
        }),
      ),
    ).toBe(true);
  });

  it('does not treat an active cloud startup or legacy local source as configuration-required', () => {
    expect(cloudConfigurationRequired(status({ phase: 'ready' }))).toBe(false);
    expect(
      cloudConfigurationRequired(
        status({ source: 'ollama', phase: 'configuration_required' }),
      ),
    ).toBe(false);
    expect(cloudConfigurationRequired(null)).toBe(false);
  });
});
