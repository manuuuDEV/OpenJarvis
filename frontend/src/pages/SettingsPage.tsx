import { useState, useEffect, useCallback } from 'react';
import {
  Palette,
  Globe,
  Cpu,
  Database,
  Info,
  Check,
  Sun,
  Moon,
  Monitor,
  Download,
  Upload,
  Trash2,
  Mic,
  Key,
  Search,
  Brain,
} from 'lucide-react';
import { useAppStore, type ThemeMode } from '../lib/store';
import { dumpDiag, isDiagBuild, isRecording, startRecording, stopRecording } from '../lib/diag';
import {
  checkHealth,
  fetchSpeechHealth,
  getAndroidAdbConfig,
  discoverAndroidAdbDevices,
  setAndroidAdbConfig,
  getTranscriptionSource,
  setTranscriptionSource,
  getGeminiLiveConfig,
  setGeminiLiveConfig,
  getMemoryStats,
  getInferenceSource,
  setInferenceSource,
  getControlledFolders,
  setControlledFolders,
  runSecureSelfTest,
  getExecutionGuardStatus,
  getCloudKeyStatus,
  saveCloudKey,
  fetchToolCredentialStatus,
  saveToolCredentials,
  deleteToolCredential,
  isTauri,
  type CloudProvider,
  type AndroidAdbDevice,
  type SecureSelfTestReport,
  type ExecutionGuardStatus,
} from '../lib/api';

const CLOUD_KEY_STATUS_CHANGED = 'openjarvis-cloud-key-status-changed';
const TRANSCRIPTION_STATUS_CHANGED = 'openjarvis-transcription-status-changed';

const CLOUD_PROVIDER_OPTIONS: Array<{
  id: CloudProvider;
  label: string;
  keyName: string;
  keyPlaceholder: string;
  endpointRequired?: boolean;
  endpointHint?: string;
}> = [
  { id: 'groq', label: 'Groq', keyName: 'GROQ_API_KEY', keyPlaceholder: 'gsk_...' },
  { id: 'google', label: 'Google Gemini', keyName: 'GEMINI_API_KEY', keyPlaceholder: 'AI...' },
  { id: 'openrouter', label: 'OpenRouter', keyName: 'OPENROUTER_API_KEY', keyPlaceholder: 'sk-or-...' },
  { id: 'nvidia', label: 'NVIDIA NIM', keyName: 'NVIDIA_API_KEY', keyPlaceholder: 'nvapi-...' },
  { id: 'sambanova', label: 'SambaNova Cloud', keyName: 'SAMBANOVA_API_KEY', keyPlaceholder: 'sn_...', endpointRequired: true, endpointHint: 'HTTPS endpoint shown in your SambaNova console' },
  { id: 'alibaba', label: 'Alibaba Cloud Model Studio', keyName: 'DASHSCOPE_API_KEY', keyPlaceholder: 'sk-...', endpointRequired: true, endpointHint: 'Regional HTTPS endpoint from your Model Studio workspace' },
  { id: 'openai', label: 'OpenAI', keyName: 'OPENAI_API_KEY', keyPlaceholder: 'sk-...' },
  { id: 'pollinations', label: 'Pollinations', keyName: 'POLLINATIONS_API_KEY', keyPlaceholder: 'pk_ or sk_...' },
  { id: 'huggingface', label: 'Hugging Face', keyName: 'HF_TOKEN', keyPlaceholder: 'hf_...' },
  { id: 'together', label: 'Together AI', keyName: 'TOGETHER_API_KEY', keyPlaceholder: '...' },
];

const TRANSCRIPTION_PROVIDER_OPTIONS = [
  { id: 'groq-whisper', label: 'Groq Whisper' },
] as const;

type TranscriptionProvider = '' | (typeof TRANSCRIPTION_PROVIDER_OPTIONS)[number]['id'];
type TranscriptionModel = 'whisper-large-v3-turbo' | 'whisper-large-v3';

function ApiKeyInput({
  keyName,
  placeholder,
  toolName,
}: {
  keyName: string;
  placeholder: string;
  toolName?: string;
}) {
  const [value, setValue] = useState('');
  const [saved, setSaved] = useState(false);
  const [hasKey, setHasKey] = useState(false);
  const [error, setError] = useState('');
  const desktopKeyStorage = isTauri();
  const serverToolStorage = !desktopKeyStorage && !!toolName;
  const canManage = desktopKeyStorage || serverToolStorage;

  const refresh = useCallback(async () => {
    if (!canManage) {
      setHasKey(false);
      return;
    }
    try {
      const status = desktopKeyStorage
        ? await getCloudKeyStatus()
        : await fetchToolCredentialStatus(toolName!);
      setHasKey(!!status[keyName]);
    } catch {
      setHasKey(false);
    }
  }, [canManage, desktopKeyStorage, keyName, toolName]);

  useEffect(() => {
    void refresh();
    window.addEventListener(CLOUD_KEY_STATUS_CHANGED, refresh);
    return () => window.removeEventListener(CLOUD_KEY_STATUS_CHANGED, refresh);
  }, [refresh]);

  const save = async (v: string) => {
    const next = v.trim();
    if (!next) return;
    setError('');
    try {
      if (desktopKeyStorage) {
        await saveCloudKey(keyName, next);
      } else if (toolName) {
        await saveToolCredentials(toolName, { [keyName]: next });
      } else {
        return;
      }
      setValue('');
      setHasKey(true);
      setSaved(true);
      window.dispatchEvent(new Event(CLOUD_KEY_STATUS_CHANGED));
      if (keyName === 'GROQ_API_KEY') window.dispatchEvent(new Event(TRANSCRIPTION_STATUS_CHANGED));
      setTimeout(() => setSaved(false), 2000);
    } catch (e: any) {
      setError(e?.message || 'Failed to save API key');
    }
  };

  const remove = async () => {
    setError('');
    try {
      if (desktopKeyStorage) {
        await saveCloudKey(keyName, '');
      } else if (toolName) {
        await deleteToolCredential(toolName, keyName);
      } else {
        return;
      }
      setValue('');
      setHasKey(false);
      setSaved(true);
      window.dispatchEvent(new Event(CLOUD_KEY_STATUS_CHANGED));
      if (keyName === 'GROQ_API_KEY') window.dispatchEvent(new Event(TRANSCRIPTION_STATUS_CHANGED));
      setTimeout(() => setSaved(false), 2000);
    } catch (e: any) {
      setError(e?.message || 'Failed to remove API key');
    }
  };

  return (
    <div className="flex items-center gap-2">
      <input
        type="password"
        value={value}
        onChange={e => setValue(e.target.value)}
        onBlur={() => { if (value.trim()) void save(value); }}
        placeholder={hasKey ? (desktopKeyStorage ? 'Saved in secure storage' : 'Saved by local server') : placeholder}
        disabled={!canManage}
        className="w-48 px-2 py-1 rounded text-xs"
        style={{ background: 'var(--color-bg)', border: '1px solid var(--color-border)', color: 'var(--color-text)' }} />
      {hasKey && (
        <button
          onClick={() => void remove()}
          className="px-2 py-1 rounded text-[10px] cursor-pointer"
          style={{ color: 'var(--color-error)', border: '1px solid var(--color-error)' }}
        >
          Remove
        </button>
      )}
      {saved && <span className="text-[10px]" style={{ color: 'var(--color-success)' }}>Saved</span>}
      {error && <span className="text-[10px]" style={{ color: 'var(--color-error)' }}>{error}</span>}
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div
      className="rounded-xl p-5"
      style={{ background: 'var(--color-surface)', border: '1px solid var(--color-border)' }}
    >
      <h3 className="text-sm font-semibold mb-4" style={{ color: 'var(--color-text)' }}>
        {title}
      </h3>
      {children}
    </div>
  );
}

function SettingRow({ label, description, children }: { label: string; description?: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between py-3" style={{ borderBottom: '1px solid var(--color-border-subtle)' }}>
      <div>
        <div className="text-sm" style={{ color: 'var(--color-text)' }}>{label}</div>
        {description && (
          <div className="text-xs mt-0.5" style={{ color: 'var(--color-text-tertiary)' }}>{description}</div>
        )}
      </div>
      <div>{children}</div>
    </div>
  );
}

const themeOptions: { value: ThemeMode; label: string; icon: typeof Sun }[] = [
  { value: 'light', label: 'Light', icon: Sun },
  { value: 'dark', label: 'Dark', icon: Moon },
  { value: 'system', label: 'System', icon: Monitor },
];

export function SettingsPage() {
  const desktopBuild = isTauri();
  const settings = useAppStore((s) => s.settings);
  const updateSettings = useAppStore((s) => s.updateSettings);
  const conversations = useAppStore((s) => s.conversations);
  const serverInfo = useAppStore((s) => s.serverInfo);
  const [healthy, setHealthy] = useState<boolean | null>(null);
  const [speechBackendAvailable, setSpeechBackendAvailable] = useState<boolean | null>(null);
  const [saved, setSaved] = useState(false);

  const [memoryStats, setMemoryStats] = useState<{ entries: number; backend: string } | null>(null);
  const [memoryEnabled, setMemoryEnabled] = useState(() => {
    try { return localStorage.getItem('openjarvis-memory-enabled') !== 'false'; } catch { return true; }
  });
  const [memoryBackend, setMemoryBackend] = useState(() => {
    try { return localStorage.getItem('openjarvis-memory-backend') || 'sqlite'; } catch { return 'sqlite'; }
  });
  const [memoryTopK, setMemoryTopK] = useState(() => {
    try { return parseInt(localStorage.getItem('openjarvis-memory-top-k') || '5'); } catch { return 5; }
  });
  const [memoryMinScore, setMemoryMinScore] = useState(() => {
    try { return parseFloat(localStorage.getItem('openjarvis-memory-min-score') || '0.1'); } catch { return 0.1; }
  });
  const [memoryMaxTokens, setMemoryMaxTokens] = useState(() => {
    try { return parseInt(localStorage.getItem('openjarvis-memory-max-tokens') || '2048'); } catch { return 2048; }
  });

  const [cloudProvider, setCloudProvider] = useState<CloudProvider>('openai');
  const [cloudModel, setCloudModel] = useState('');
  const [cloudKey, setCloudKey] = useState('');
  const [cloudEndpoint, setCloudEndpoint] = useState('');
  const [transcriptionProvider, setTranscriptionProvider] = useState<TranscriptionProvider>('');
  const [transcriptionModel, setTranscriptionModel] = useState<TranscriptionModel>('whisper-large-v3-turbo');
  const [transcriptionProcessingAcknowledged, setTranscriptionProcessingAcknowledged] = useState(false);
  const [transcriptionMsg, setTranscriptionMsg] = useState('');
  const [geminiLiveProcessingAcknowledged, setGeminiLiveProcessingAcknowledged] = useState(false);
  const [geminiLiveMsg, setGeminiLiveMsg] = useState('');
  const [providerProcessingAcknowledged, setProviderProcessingAcknowledged] = useState(false);
  const [srcMsg, setSrcMsg] = useState('');
  const [androidAdbPath, setAndroidAdbPath] = useState('');
  const [androidAdbDevices, setAndroidAdbDevices] = useState<AndroidAdbDevice[]>([]);
  const [androidAdbSerial, setAndroidAdbSerial] = useState('');
  const [androidAdbAcknowledged, setAndroidAdbAcknowledged] = useState(false);
  const [androidAdbMsg, setAndroidAdbMsg] = useState('');
  const [controlledFolders, setControlledFoldersState] = useState<string[]>([]);
  const [controlledFolderInput, setControlledFolderInput] = useState('');
  const [controlledFolderMsg, setControlledFolderMsg] = useState('');
  const [secureSelfTestReport, setSecureSelfTestReport] = useState<SecureSelfTestReport | null>(null);
  const [secureSelfTestRunning, setSecureSelfTestRunning] = useState(false);
  const [secureSelfTestError, setSecureSelfTestError] = useState('');
  const [executionGuardStatus, setExecutionGuardStatus] = useState<ExecutionGuardStatus | null>(null);
  const [executionGuardError, setExecutionGuardError] = useState('');

  const refreshExecutionGuardStatus = useCallback(async () => {
    if (!desktopBuild) return;
    try {
      setExecutionGuardError('');
      setExecutionGuardStatus(await getExecutionGuardStatus());
    } catch (error: any) {
      setExecutionGuardStatus(null);
      setExecutionGuardError(error?.message ?? 'Impossibile leggere lo stato di Windows Security.');
    }
  }, [desktopBuild]);

  useEffect(() => {
    getInferenceSource().then((s) => {
      if (s.provider) setCloudProvider(s.provider);
      if (s.model) setCloudModel(s.model);
      if (s.providerEndpoint) setCloudEndpoint(s.providerEndpoint);
      setProviderProcessingAcknowledged(!!s.providerProcessingAcknowledged);
    }).catch(() => {});
    getTranscriptionSource().then((s) => {
      if (!s) return;
      setTranscriptionProvider(s.provider ?? '');
      setTranscriptionModel(s.model);
      setTranscriptionProcessingAcknowledged(s.processing_acknowledged);
    }).catch(() => {});
    getGeminiLiveConfig().then((config) => {
      if (config) setGeminiLiveProcessingAcknowledged(config.processingAcknowledged);
    }).catch(() => {});
    getAndroidAdbConfig().then((config) => {
      if (!config) return;
      setAndroidAdbPath(config.adb_path ?? '');
      setAndroidAdbSerial(config.device_serial ?? '');
      setAndroidAdbAcknowledged(config.diagnostics_acknowledged);
    }).catch(() => {});
    getControlledFolders().then(setControlledFoldersState).catch(() => {});
    void refreshExecutionGuardStatus();
  }, [refreshExecutionGuardStatus]);

  const saveSource = useCallback(async () => {
    try {
      await setInferenceSource({
        kind: 'cloud',
        provider: cloudProvider,
        model: cloudModel,
        apiKey: cloudKey || undefined,
        providerEndpoint: cloudEndpoint || undefined,
        providerProcessingAcknowledged,
      });
      setCloudKey('');
      setSrcMsg('Authorized cloud provider saved. Restart the app to apply.');
    } catch (e: any) {
      setSrcMsg(e?.message ?? 'Failed to save.');
    }
  }, [cloudProvider, cloudModel, cloudKey, cloudEndpoint, providerProcessingAcknowledged]);

  const selectedProvider = CLOUD_PROVIDER_OPTIONS.find((item) => item.id === cloudProvider) ?? CLOUD_PROVIDER_OPTIONS[0];

  const saveTranscriptionSource = useCallback(async () => {
    try {
      await setTranscriptionSource({
        provider: transcriptionProvider || null,
        model: transcriptionModel,
        processing_acknowledged: transcriptionProcessingAcknowledged,
      });
      const health = await fetchSpeechHealth();
      setSpeechBackendAvailable(health.available);
      window.dispatchEvent(new Event(TRANSCRIPTION_STATUS_CHANGED));
      setTranscriptionMsg(
        transcriptionProvider
          ? 'Groq Whisper saved. Recorded audio is sent to Groq only after you stop the microphone.'
          : 'Cloud transcription disabled.',
      );
    } catch (e: any) {
      setTranscriptionMsg(e?.message ?? 'Failed to save transcription settings.');
    }
  }, [transcriptionProvider, transcriptionModel, transcriptionProcessingAcknowledged]);

  const saveGeminiLiveConfig = useCallback(async () => {
    try {
      await setGeminiLiveConfig(geminiLiveProcessingAcknowledged);
      setGeminiLiveMsg(
        geminiLiveProcessingAcknowledged
          ? 'Gemini Live is enabled. Starting a conversation will request a one-use temporary token; microphone audio is sent directly to Google over TLS.'
          : 'Gemini Live disabled.',
      );
    } catch (e: any) {
      setGeminiLiveMsg(e?.message ?? 'Unable to save Gemini Live settings.');
    }
  }, [geminiLiveProcessingAcknowledged]);

  const discoverAndroidDevices = useCallback(async () => {
    setAndroidAdbMsg('');
    try {
      const devices = await discoverAndroidAdbDevices(androidAdbPath);
      setAndroidAdbDevices(devices);
      if (!devices.some((device) => device.serial === androidAdbSerial)) {
        setAndroidAdbSerial('');
      }
      setAndroidAdbMsg(
        devices.length
          ? 'Device rilevati localmente. Seleziona solo il tuo device nello stato “device”.'
          : 'Nessun device ADB rilevato. Collega e sblocca Android, attiva USB debugging e conferma la chiave RSA sul telefono.',
      );
    } catch (e: any) {
      setAndroidAdbDevices([]);
      setAndroidAdbMsg(e?.message ?? 'Rilevamento Android ADB non riuscito.');
    }
  }, [androidAdbPath, androidAdbSerial]);

  const saveAndroidAdb = useCallback(async () => {
    try {
      await setAndroidAdbConfig({
        adb_path: androidAdbPath || null,
        device_serial: androidAdbSerial || null,
        diagnostics_acknowledged: androidAdbAcknowledged,
      });
      setAndroidAdbMsg(
        'Diagnostica Android ADB salvata. L’agente potrà solo proporre una scansione software in sola lettura, con approvazione ogni volta.',
      );
    } catch (e: any) {
      setAndroidAdbMsg(e?.message ?? 'Impossibile salvare la configurazione Android ADB.');
    }
  }, [androidAdbPath, androidAdbSerial, androidAdbAcknowledged]);

  const clearAndroidAdb = useCallback(async () => {
    try {
      await setAndroidAdbConfig({
        adb_path: null,
        device_serial: null,
        diagnostics_acknowledged: false,
      });
      setAndroidAdbPath('');
      setAndroidAdbDevices([]);
      setAndroidAdbSerial('');
      setAndroidAdbAcknowledged(false);
      setAndroidAdbMsg('Diagnostica Android ADB disabilitata.');
    } catch (e: any) {
      setAndroidAdbMsg(e?.message ?? 'Impossibile disabilitare Android ADB.');
    }
  }, []);

  const runDesktopSelfTest = useCallback(async () => {
    setSecureSelfTestRunning(true);
    setSecureSelfTestError('');
    try {
      setSecureSelfTestReport(await runSecureSelfTest());
    } catch (e: any) {
      setSecureSelfTestReport(null);
      setSecureSelfTestError(e?.message ?? 'Impossibile eseguire la verifica sicura.');
    } finally {
      setSecureSelfTestRunning(false);
    }
  }, []);

  const saveControlledFolders = useCallback(async (nextFolders: string[]) => {
    try {
      const savedFolders = await setControlledFolders(nextFolders);
      setControlledFoldersState(savedFolders);
      setControlledFolderInput('');
      setControlledFolderMsg('Approved folders saved. The assistant can only use these folders and the dedicated workspace.');
    } catch (e: any) {
      setControlledFolderMsg(e?.message ?? 'Failed to save approved folders.');
    }
  }, []);

  useEffect(() => {
    checkHealth().then(setHealthy);
    fetchSpeechHealth()
      .then((h) => setSpeechBackendAvailable(h.available))
      .catch(() => setSpeechBackendAvailable(false));
    getMemoryStats()
      .then(setMemoryStats)
      .catch(() => setMemoryStats(null));
  }, []);

  const showSaved = () => {
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  };

  const handleExport = () => {
    const data = localStorage.getItem('openjarvis-conversations') || '{}';
    const blob = new Blob([data], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `openjarvis-export-${new Date().toISOString().slice(0, 10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleImport = () => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json';
    input.onchange = (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;
      const reader = new FileReader();
      reader.onload = (ev) => {
        try {
          const data = JSON.parse(ev.target?.result as string);
          if (data.version === 1) {
            localStorage.setItem('openjarvis-conversations', JSON.stringify(data));
            useAppStore.getState().loadConversations();
            showSaved();
          }
        } catch {}
      };
      reader.readAsText(file);
    };
    input.click();
  };

  const [confirmClear, setConfirmClear] = useState(false);
  const handleClear = () => {
    if (!confirmClear) {
      setConfirmClear(true);
      setTimeout(() => setConfirmClear(false), 3000);
      return;
    }
    localStorage.removeItem('openjarvis-conversations');
    useAppStore.getState().loadConversations();
    setConfirmClear(false);
    showSaved();
  };

  return (
    <div className="flex-1 overflow-y-auto px-6 py-10">
      <div className="max-w-2xl mx-auto">
        <header className="mb-6">
          <div className="flex items-center justify-between gap-3">
            <h1 className="text-lg font-semibold" style={{ color: 'var(--color-text)' }}>
              Settings
            </h1>
            {saved && (
              <span className="flex items-center gap-1 text-xs px-2 py-1 rounded-full" style={{
                background: 'var(--color-accent-subtle)',
                color: 'var(--color-success)',
              }}>
                <Check size={12} /> Saved
              </span>
            )}
          </div>
          <p className="text-sm mt-2 max-w-2xl" style={{ color: 'var(--color-text-secondary)' }}>
            App preferences — appearance, model defaults, keyboard shortcuts, and data management.
          </p>
        </header>

        <div className="flex flex-col gap-4">
          <Section title="Verifica Jarvis">
            <div className="flex flex-col gap-3">
              <p className="text-xs" style={{ color: 'var(--color-text-secondary)' }}>
                Controllo locale e senza effetti collaterali: non avvia broker, browser o ADB, non registra audio e non invia dati al cloud.
              </p>
              <div className="flex items-center gap-3 flex-wrap">
                <button
                  onClick={() => void runDesktopSelfTest()}
                  disabled={!isTauri() || secureSelfTestRunning}
                  className="px-3 py-2 rounded text-xs font-medium cursor-pointer disabled:cursor-not-allowed"
                  style={{
                    background: 'var(--color-accent)',
                    color: 'var(--color-on-accent, white)',
                    opacity: !isTauri() || secureSelfTestRunning ? 0.55 : 1,
                  }}
                >
                  {secureSelfTestRunning ? 'Verifica in corso…' : 'Verifica Jarvis'}
                </button>
                {!isTauri() && (
                  <span className="text-xs" style={{ color: 'var(--color-text-tertiary)' }}>
                    Disponibile nell’app desktop Windows.
                  </span>
                )}
              </div>
              {secureSelfTestError && (
                <p className="text-xs" style={{ color: 'var(--color-error)' }}>{secureSelfTestError}</p>
              )}
              {secureSelfTestReport && (
                <div className="rounded overflow-hidden" style={{ border: '1px solid var(--color-border)' }}>
                  <div className="px-3 py-2 text-xs" style={{ background: 'var(--color-bg-secondary)', color: 'var(--color-text-secondary)' }}>
                    {secureSelfTestReport.passed} verifiche superate · {secureSelfTestReport.warnings} avvisi · {secureSelfTestReport.liveChecksRequired} prove reali richieste
                  </div>
                  <div className="divide-y" style={{ borderColor: 'var(--color-border)' }}>
                    {secureSelfTestReport.checks.map((check) => {
                      const color = check.status === 'pass'
                        ? 'var(--color-success)'
                        : check.status === 'live_check_required'
                          ? 'var(--color-accent)'
                          : 'var(--color-warning)';
                      const label = check.status === 'pass'
                        ? 'Pronto'
                        : check.status === 'live_check_required'
                          ? 'Prova reale richiesta'
                          : check.status === 'not_configured'
                            ? 'Non configurato'
                            : 'Attenzione';
                      return (
                        <div key={check.id} className="px-3 py-2 flex gap-3 items-start">
                          <span className="shrink-0 text-[10px] font-medium mt-0.5" style={{ color }}>{label}</span>
                          <div>
                            <p className="text-xs font-medium" style={{ color: 'var(--color-text)' }}>{check.title}</p>
                            <p className="text-[11px] mt-0.5" style={{ color: 'var(--color-text-secondary)' }}>{check.detail}</p>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </div>
              )}
            </div>
          </Section>

          {desktopBuild && (
            <Section title="Protezione apertura file e app">
              <div className="flex flex-col gap-3">
                <p className="text-xs" style={{ color: 'var(--color-text-secondary)' }}>
                  Monitoraggio locale in sola lettura. Le azioni avviate da Jarvis richiedono un controllo Defender prima dell’apertura; OpenJarvis non modifica Defender, SmartScreen, esclusioni o impostazioni di sicurezza Windows.
                </p>
                <div className="grid grid-cols-1 md:grid-cols-3 gap-2 text-xs">
                  <div className="rounded-lg p-3" style={{ background: 'var(--color-bg-secondary)', border: '1px solid var(--color-border)' }}>
                    <div style={{ color: 'var(--color-text-secondary)' }}>Guardiano Jarvis</div>
                    <div className="font-semibold mt-1" style={{ color: executionGuardStatus?.execution_guard ? 'var(--color-success)' : 'var(--color-error)' }}>
                      {executionGuardStatus ? (executionGuardStatus.execution_guard ? 'Obbligatorio per aperture Jarvis' : 'Non attivo') : 'Stato non letto'}
                    </div>
                  </div>
                  <div className="rounded-lg p-3" style={{ background: 'var(--color-bg-secondary)', border: '1px solid var(--color-border)' }}>
                    <div style={{ color: 'var(--color-text-secondary)' }}>Windows Security / Defender</div>
                    <div className="font-semibold mt-1" style={{ color: executionGuardStatus?.defender_health === 'good' ? 'var(--color-success)' : 'var(--color-warning)' }}>
                      {executionGuardStatus?.defender_health ?? 'Non disponibile'}
                    </div>
                  </div>
                  <div className="rounded-lg p-3" style={{ background: 'var(--color-bg-secondary)', border: '1px solid var(--color-border)' }}>
                    <div style={{ color: 'var(--color-text-secondary)' }}>SmartScreen</div>
                    <div className="font-semibold mt-1" style={{ color: 'var(--color-text)' }}>
                      {executionGuardStatus?.smart_screen ?? 'Stato non disponibile'}
                    </div>
                  </div>
                </div>
                <div className="flex items-center gap-3 flex-wrap">
                  <button
                    onClick={() => void refreshExecutionGuardStatus()}
                    className="px-3 py-2 rounded text-xs font-medium cursor-pointer"
                    style={{ background: 'var(--color-bg-secondary)', border: '1px solid var(--color-border)', color: 'var(--color-text)' }}
                  >
                    Aggiorna stato protezione
                  </button>
                  <span className="text-[11px]" style={{ color: 'var(--color-text-tertiary)' }}>
                    Per una protezione che resta attiva quando Jarvis è chiuso, verifica sempre Windows Security direttamente in Windows.
                  </span>
                </div>
                {executionGuardStatus?.details.map((detail) => (
                  <p key={detail} className="text-[11px]" style={{ color: 'var(--color-text-secondary)' }}>{detail}</p>
                ))}
                {executionGuardError && <p className="text-xs" style={{ color: 'var(--color-error)' }}>{executionGuardError}</p>}
              </div>
            </Section>
          )}

          {/* Appearance */}
          <Section title="Appearance">
            <SettingRow label="Theme" description="Choose how OpenJarvis looks">
              <div className="flex gap-1 p-0.5 rounded-lg" style={{ background: 'var(--color-bg-secondary)' }}>
                {themeOptions.map((opt) => {
                  const isActive = settings.theme === opt.value;
                  return (
                    <button
                      key={opt.value}
                      onClick={() => { updateSettings({ theme: opt.value }); showSaved(); }}
                      className="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-colors cursor-pointer"
                      style={{
                        background: isActive ? 'var(--color-surface)' : 'transparent',
                        color: isActive ? 'var(--color-text)' : 'var(--color-text-tertiary)',
                        boxShadow: isActive ? 'var(--shadow-sm)' : 'none',
                      }}
                    >
                      <opt.icon size={14} />
                      {opt.label}
                    </button>
                  );
                })}
              </div>
            </SettingRow>
            <SettingRow label="Font size">
              <select
                value={settings.fontSize}
                onChange={(e) => { updateSettings({ fontSize: e.target.value as any }); showSaved(); }}
                className="text-sm px-3 py-1.5 rounded-lg outline-none cursor-pointer"
                style={{
                  background: 'var(--color-bg-secondary)',
                  color: 'var(--color-text)',
                  border: '1px solid var(--color-border)',
                }}
              >
                <option value="small">Small</option>
                <option value="default">Default</option>
                <option value="large">Large</option>
              </select>
            </SettingRow>
          </Section>

          {/* Connection */}
          <Section title="Connection">
            <SettingRow label="Server status" description={serverInfo ? `${serverInfo.engine} / ${serverInfo.model}` : 'Not connected'}>
              <div className="flex items-center gap-2">
                <span
                  className="w-2 h-2 rounded-full"
                  style={{ background: healthy === true ? 'var(--color-success)' : healthy === false ? 'var(--color-error)' : 'var(--color-text-tertiary)' }}
                />
                <span className="text-xs" style={{ color: 'var(--color-text-secondary)' }}>
                  {healthy === true ? 'Connected' : healthy === false ? 'Disconnected' : 'Checking...'}
                </span>
              </div>
            </SettingRow>
            {desktopBuild ? (
              <SettingRow
                label="Backend desktop"
                description="Gestito localmente dal processo nativo. Questa build non usa URL o API key del backend salvati nel browser e non può essere reindirizzata a un host remoto."
              >
                <span className="text-xs" style={{ color: 'var(--color-text-secondary)' }}>
                  Endpoint locale gestito dall’app
                </span>
              </SettingRow>
            ) : (
              <>
                <SettingRow label="API URL" description="Set if backend runs on a different port or host">
                  <input
                    type="text"
                    value={settings.apiUrl}
                    onChange={(e) => { updateSettings({ apiUrl: e.target.value }); showSaved(); }}
                    placeholder="http://localhost:8000"
                    className="text-sm px-3 py-1.5 rounded-lg outline-none w-56"
                    style={{
                      background: 'var(--color-bg-secondary)',
                      color: 'var(--color-text)',
                      border: '1px solid var(--color-border)',
                    }}
                  />
                </SettingRow>
                <SettingRow label="API key" description="Required only if the server was started with an API key">
                  <input
                    type="password"
                    value={settings.apiKey}
                    onChange={(e) => { updateSettings({ apiKey: e.target.value }); showSaved(); }}
                    placeholder="OPENJARVIS_API_KEY"
                    autoComplete="off"
                    className="text-sm px-3 py-1.5 rounded-lg outline-none w-56"
                    style={{
                      background: 'var(--color-bg-secondary)',
                      color: 'var(--color-text)',
                      border: '1px solid var(--color-border)',
                    }}
                  />
                </SettingRow>
              </>
            )}
          </Section>

          {/* Cloud-only inference */}
          <Section title="Authorized cloud inference">
            <p className="text-xs mb-3" style={{ color: 'var(--color-text-tertiary)' }}>
              This build does not install, start, or download local models. Requests use TLS in transit; the selected provider processes the prompt and response within its own inference boundary.
            </p>
            <SettingRow label="Provider attivo" description="Un solo provider viene autorizzato e inviato al backend alla volta; le altre chiavi restano isolate nel portachiavi.">
              <select
                value={cloudProvider}
                onChange={(e) => {
                  const next = e.target.value as CloudProvider;
                  setCloudProvider(next);
                  setCloudEndpoint('');
                  setSrcMsg('');
                }}
                className="text-sm px-3 py-1.5 rounded-lg outline-none w-56"
                style={{ background: 'var(--color-bg-secondary)', color: 'var(--color-text)', border: '1px solid var(--color-border)' }}
              >
                {CLOUD_PROVIDER_OPTIONS.map((provider) => (
                  <option key={provider.id} value={provider.id}>{provider.label}</option>
                ))}
              </select>
            </SettingRow>
            <SettingRow label="Modello" description="Inserisci un modello appartenente al provider attivo; non esiste fallback automatico verso altri provider.">
              <input type="text" value={cloudModel} onChange={(e) => { setCloudModel(e.target.value); setSrcMsg(''); }} placeholder="Modello del provider"
                className="text-sm px-3 py-1.5 rounded-lg outline-none w-56"
                style={{ background: 'var(--color-bg-secondary)', color: 'var(--color-text)', border: '1px solid var(--color-border)' }} />
            </SettingRow>
            {selectedProvider.endpointRequired && (
              <SettingRow label="Endpoint del provider" description={selectedProvider.endpointHint}>
                <input type="url" value={cloudEndpoint} onChange={(e) => { setCloudEndpoint(e.target.value); setSrcMsg(''); }} placeholder="https://..."
                  autoComplete="off" className="text-sm px-3 py-1.5 rounded-lg outline-none w-56"
                  style={{ background: 'var(--color-bg-secondary)', color: 'var(--color-text)', border: '1px solid var(--color-border)' }} />
              </SettingRow>
            )}
            <SettingRow label={`${selectedProvider.label} API key`} description="Salvata solo nel portachiavi di Windows; mai nel repository, file di configurazione o log.">
              <input type="password" value={cloudKey} onChange={(e) => { setCloudKey(e.target.value); setSrcMsg(''); }} placeholder={selectedProvider.keyPlaceholder}
                autoComplete="off" className="text-sm px-3 py-1.5 rounded-lg outline-none w-56"
                style={{ background: 'var(--color-bg-secondary)', color: 'var(--color-text)', border: '1px solid var(--color-border)' }} />
            </SettingRow>
            <SettingRow label="Provider processing acknowledgement" description="I understand that TLS protects the connection in transit, but the selected cloud provider processes prompts and responses. Do not include data you are not authorized to share.">
              <label className="flex items-start gap-2 max-w-xs text-xs" style={{ color: 'var(--color-text-secondary)' }}>
                <input
                  type="checkbox"
                  checked={providerProcessingAcknowledged}
                  onChange={(e) => { setProviderProcessingAcknowledged(e.target.checked); setSrcMsg(''); }}
                  className="mt-0.5"
                />
                <span>I explicitly authorize this provider for the selected model.</span>
              </label>
            </SettingRow>
            <SettingRow label="" description={srcMsg}>
              <button onClick={saveSource} disabled={!providerProcessingAcknowledged}
                className="text-sm px-3 py-1.5 rounded-lg outline-none cursor-pointer"
                style={{ background: 'var(--color-accent, var(--color-bg-tertiary))', color: 'var(--color-text)', border: '1px solid var(--color-border)' }}>
                Save authorized cloud profile
              </button>
            </SettingRow>
          </Section>

          <Section title="Diagnostica Android ADB controllata">
            <p className="text-xs mb-3" style={{ color: 'var(--color-text-tertiary)' }}>
              Collega solo il tuo Android, sbloccalo e accetta personalmente la chiave RSA di USB debugging. Il modulo non espone una shell ADB al modello: consente soltanto una scansione software in sola lettura del device scelto qui, dopo approvazione per ogni richiesta.
            </p>
            <SettingRow label="Android Platform Tools" description="Indica il file adb.exe contenuto nella cartella ufficiale platform-tools. Il percorso non viene inviato al backend cloud.">
              <div className="flex gap-2">
                <input
                  type="text"
                  value={androidAdbPath}
                  onChange={(e) => { setAndroidAdbPath(e.target.value); setAndroidAdbDevices([]); setAndroidAdbSerial(''); setAndroidAdbMsg(''); }}
                  placeholder={'C:\\Android\\Sdk\\platform-tools\\adb.exe'}
                  autoComplete="off"
                  className="text-sm px-3 py-1.5 rounded-lg outline-none w-72"
                  style={{ background: 'var(--color-bg-secondary)', color: 'var(--color-text)', border: '1px solid var(--color-border)' }}
                />
                <button
                  onClick={() => void discoverAndroidDevices()}
                  disabled={!androidAdbPath.trim()}
                  className="text-sm px-3 py-1.5 rounded-lg outline-none cursor-pointer"
                  style={{ background: 'var(--color-bg-secondary)', color: 'var(--color-text)', border: '1px solid var(--color-border)' }}
                >
                  Rileva device
                </button>
              </div>
            </SettingRow>
            <SettingRow label="Device Android autorizzato" description="Il seriale rimane nelle impostazioni native locali. Sono selezionabili soltanto device nello stato “device”.">
              <select
                value={androidAdbSerial}
                onChange={(e) => { setAndroidAdbSerial(e.target.value); setAndroidAdbMsg(''); }}
                disabled={!androidAdbDevices.length}
                className="text-sm px-3 py-1.5 rounded-lg outline-none w-72"
                style={{ background: 'var(--color-bg-secondary)', color: 'var(--color-text)', border: '1px solid var(--color-border)' }}
              >
                <option value="">Seleziona un device rilevato</option>
                {androidAdbDevices.map((device) => (
                  <option key={device.serial} value={device.serial} disabled={device.state !== 'device'}>
                    {device.model ? `${device.model} — ` : ''}{device.serial} ({device.state})
                  </option>
                ))}
              </select>
            </SettingRow>
            <SettingRow label="Consenso diagnostica software" description="La scansione legge versione Android, spazio, memoria, batteria e conteggio app. Non apre app, non invia tap/tastiera, non installa o rimuove software, non trasferisce file e non usa root.">
              <label className="flex items-start gap-2 max-w-xs text-xs" style={{ color: 'var(--color-text-secondary)' }}>
                <input
                  type="checkbox"
                  checked={androidAdbAcknowledged}
                  onChange={(e) => { setAndroidAdbAcknowledged(e.target.checked); setAndroidAdbMsg(''); }}
                  className="mt-0.5"
                />
                <span>Autorizzo soltanto la diagnostica Android ADB in sola lettura sul device selezionato.</span>
              </label>
            </SettingRow>
            <SettingRow label="Stato Android ADB" description={androidAdbMsg || 'Configura Platform Tools, rileva il device, selezionalo e conferma il perimetro.'}>
              <div className="flex gap-2">
                <button
                  onClick={() => void saveAndroidAdb()}
                  disabled={!androidAdbPath.trim() || !androidAdbSerial || !androidAdbAcknowledged}
                  className="text-sm px-3 py-1.5 rounded-lg outline-none cursor-pointer"
                  style={{ background: 'var(--color-accent, var(--color-bg-tertiary))', color: 'var(--color-text)', border: '1px solid var(--color-border)' }}
                >
                  Salva autorizzazione
                </button>
                {(androidAdbPath || androidAdbSerial) && (
                  <button
                    onClick={() => void clearAndroidAdb()}
                    className="text-sm px-3 py-1.5 rounded-lg outline-none cursor-pointer"
                    style={{ color: 'var(--color-error)', border: '1px solid var(--color-error)' }}
                  >
                    Disabilita
                  </button>
                )}
              </div>
            </SettingRow>
            <div className="text-xs mt-3 px-1" style={{ color: 'var(--color-text-tertiary)' }}>
              Dopo il salvataggio puoi chiedere in chat una “diagnostica software Android”. L’agente potrà soltanto proporla: la richiesta appare tra le approvazioni e il broker esegue una lista fissa di letture, non comandi ADB arbitrari.
            </div>
          </Section>

          <Section title="Controlled local folders">
            <p className="text-xs mb-3" style={{ color: 'var(--color-text-tertiary)' }}>
              Add up to eight existing folders that you personally authorize. The model cannot add folders itself; file reads are limited and all writes or directory changes still require a separate, one-time approval in this app. System, credential, and broad home folders are rejected.
            </p>
            <div className="flex gap-2 mb-3">
              <input
                type="text"
                value={controlledFolderInput}
                onChange={(e) => { setControlledFolderInput(e.target.value); setControlledFolderMsg(''); }}
                placeholder="C:\\Users\\you\\Documents\\Project"
                className="flex-1 text-sm px-3 py-1.5 rounded-lg outline-none"
                style={{ background: 'var(--color-bg-secondary)', color: 'var(--color-text)', border: '1px solid var(--color-border)' }}
              />
              <button
                onClick={() => void saveControlledFolders([...controlledFolders, controlledFolderInput.trim()])}
                disabled={!controlledFolderInput.trim() || controlledFolders.length >= 8}
                className="text-sm px-3 py-1.5 rounded-lg outline-none cursor-pointer"
                style={{ background: 'var(--color-accent, var(--color-bg-tertiary))', color: 'var(--color-text)', border: '1px solid var(--color-border)' }}
              >
                Add folder
              </button>
            </div>
            {controlledFolders.map((folder) => (
              <div key={folder} className="flex items-center justify-between gap-3 py-2 text-xs" style={{ borderTop: '1px solid var(--color-border-subtle)', color: 'var(--color-text-secondary)' }}>
                <code className="truncate">{folder}</code>
                <button
                  onClick={() => void saveControlledFolders(controlledFolders.filter((item) => item !== folder))}
                  className="px-2 py-1 rounded cursor-pointer"
                  style={{ color: 'var(--color-error)', border: '1px solid var(--color-error)' }}
                >
                  Remove
                </button>
              </div>
            ))}
            {controlledFolderMsg && <p className="text-xs mt-3" style={{ color: 'var(--color-text-tertiary)' }}>{controlledFolderMsg}</p>}
          </Section>

          {/* Provider credentials */}
          <Section title="Credenziali provider salvate">
            <p className="text-xs mb-3" style={{ color: 'var(--color-text-tertiary)' }}>
              Puoi registrare più provider qui. Salvare una chiave non attiva non avvia chiamate: devi selezionare e autorizzare esplicitamente il profilo attivo sopra.
            </p>
            {CLOUD_PROVIDER_OPTIONS.map((provider) => (
              <SettingRow key={provider.id} label={provider.label} description={`Credenziale ${provider.keyName} nel portachiavi di Windows`}>
                <ApiKeyInput keyName={provider.keyName} placeholder={provider.keyPlaceholder} />
              </SettingRow>
            ))}
          </Section>

          {/* Tools */}
          <Section title="Tools">
            <SettingRow label="Web Search" description="Tavily key for web search tool">
              <ApiKeyInput keyName="TAVILY_API_KEY" placeholder="tvly-..." toolName="web_search" />
            </SettingRow>
          </Section>

          {/* Memory */}
          <Section title="Memory">
            <SettingRow label="Memory status" description={memoryStats ? `${memoryStats.backend} backend — ${memoryStats.entries} entries` : 'Unable to reach memory service'}>
              <div className="flex items-center gap-2">
                <Brain size={14} style={{ color: memoryStats ? 'var(--color-accent)' : 'var(--color-text-tertiary)' }} />
                <span className="text-xs" style={{ color: 'var(--color-text-secondary)' }}>
                  {memoryStats ? `${memoryStats.entries} entries` : 'Unavailable'}
                </span>
              </div>
            </SettingRow>
            <SettingRow label="Use memory context" description="Automatically inject relevant memories into conversations">
              <button
                onClick={() => {
                  const next = !memoryEnabled;
                  setMemoryEnabled(next);
                  try { localStorage.setItem('openjarvis-memory-enabled', String(next)); } catch {}
                  showSaved();
                }}
                className="relative w-11 h-6 rounded-full transition-colors cursor-pointer"
                style={{
                  background: memoryEnabled ? 'var(--color-accent)' : 'var(--color-bg-tertiary)',
                }}
              >
                <span
                  className="absolute top-0.5 left-0.5 w-5 h-5 rounded-full transition-transform bg-white"
                  style={{
                    transform: memoryEnabled ? 'translateX(20px)' : 'translateX(0)',
                    boxShadow: '0 1px 3px rgba(0,0,0,0.2)',
                  }}
                />
              </button>
            </SettingRow>
            <SettingRow label="Memory backend" description="Which retrieval engine to use">
              <select
                value={memoryBackend}
                onChange={(e) => {
                  setMemoryBackend(e.target.value);
                  try { localStorage.setItem('openjarvis-memory-backend', e.target.value); } catch {}
                  showSaved();
                }}
                className="text-sm px-3 py-1.5 rounded-lg outline-none cursor-pointer"
                style={{
                  background: 'var(--color-bg-secondary)',
                  color: 'var(--color-text)',
                  border: '1px solid var(--color-border)',
                }}
              >
                <option value="sqlite">sqlite</option>
                <option value="faiss">faiss</option>
                <option value="bm25">bm25</option>
                <option value="colbert">colbert</option>
                <option value="hybrid">hybrid</option>
              </select>
            </SettingRow>
            <SettingRow label="Results to inject" description={`${memoryTopK}`}>
              <input
                type="range"
                min="1"
                max="20"
                step="1"
                value={memoryTopK}
                onChange={(e) => {
                  const v = parseInt(e.target.value);
                  setMemoryTopK(v);
                  try { localStorage.setItem('openjarvis-memory-top-k', String(v)); } catch {}
                  showSaved();
                }}
                className="w-32 cursor-pointer accent-[var(--color-accent)]"
              />
            </SettingRow>
            <SettingRow label="Min relevance score" description={`${memoryMinScore}`}>
              <input
                type="range"
                min="0"
                max="1"
                step="0.05"
                value={memoryMinScore}
                onChange={(e) => {
                  const v = parseFloat(e.target.value);
                  setMemoryMinScore(v);
                  try { localStorage.setItem('openjarvis-memory-min-score', String(v)); } catch {}
                  showSaved();
                }}
                className="w-32 cursor-pointer accent-[var(--color-accent)]"
              />
            </SettingRow>
            <SettingRow label="Max context tokens" description={`${memoryMaxTokens}`}>
              <input
                type="range"
                min="256"
                max="8192"
                step="256"
                value={memoryMaxTokens}
                onChange={(e) => {
                  const v = parseInt(e.target.value);
                  setMemoryMaxTokens(v);
                  try { localStorage.setItem('openjarvis-memory-max-tokens', String(v)); } catch {}
                  showSaved();
                }}
                className="w-32 cursor-pointer accent-[var(--color-accent)]"
              />
            </SettingRow>
          </Section>

          {/* Model defaults */}
          <Section title="Model Defaults">
            <SettingRow label="Temperature" description={`${settings.temperature}`}>
              <input
                type="range"
                min="0"
                max="2"
                step="0.1"
                value={settings.temperature}
                onChange={(e) => { updateSettings({ temperature: parseFloat(e.target.value) }); showSaved(); }}
                className="w-32 cursor-pointer accent-[var(--color-accent)]"
              />
            </SettingRow>
            <SettingRow label="Max tokens" description={`${settings.maxTokens}`}>
              <input
                type="range"
                min="256"
                max="32768"
                step="256"
                value={settings.maxTokens}
                onChange={(e) => { updateSettings({ maxTokens: parseInt(e.target.value) }); showSaved(); }}
                className="w-32 cursor-pointer accent-[var(--color-accent)]"
              />
            </SettingRow>
          </Section>

          {/* Speech */}
          <Section title="Voce e trascrizione">
            <p className="text-xs mb-3" style={{ color: 'var(--color-text-tertiary)' }}>
              La registrazione resta sul dispositivo fino a quando interrompi il microfono. Se abiliti Groq Whisper, quel singolo file audio viene trasmesso a Groq via TLS per la trascrizione; la chiave rimane nel portachiavi del sistema operativo.
            </p>
            <SettingRow label="Provider di trascrizione" description="Nessun modello speech locale viene installato o avviato.">
              <select
                value={transcriptionProvider}
                onChange={(e) => { setTranscriptionProvider(e.target.value as TranscriptionProvider); setTranscriptionMsg(''); }}
                className="text-sm px-3 py-1.5 rounded-lg outline-none w-56"
                style={{ background: 'var(--color-bg-secondary)', color: 'var(--color-text)', border: '1px solid var(--color-border)' }}
              >
                <option value="">Disabilitato</option>
                {TRANSCRIPTION_PROVIDER_OPTIONS.map((provider) => (
                  <option key={provider.id} value={provider.id}>{provider.label}</option>
                ))}
              </select>
            </SettingRow>
            {transcriptionProvider && (
              <>
                <SettingRow label="Modello Groq Whisper" description="Sono ammessi soltanto modelli di trascrizione Groq esplicitamente supportati.">
                  <select
                    value={transcriptionModel}
                    onChange={(e) => { setTranscriptionModel(e.target.value as TranscriptionModel); setTranscriptionMsg(''); }}
                    className="text-sm px-3 py-1.5 rounded-lg outline-none w-56"
                    style={{ background: 'var(--color-bg-secondary)', color: 'var(--color-text)', border: '1px solid var(--color-border)' }}
                  >
                    <option value="whisper-large-v3-turbo">whisper-large-v3-turbo</option>
                    <option value="whisper-large-v3">whisper-large-v3</option>
                  </select>
                </SettingRow>
                <SettingRow label="Consenso elaborazione audio" description="TLS protegge il trasporto; Groq elabora il file audio registrato per produrre la trascrizione.">
                  <label className="flex items-start gap-2 max-w-xs text-xs" style={{ color: 'var(--color-text-secondary)' }}>
                    <input
                      type="checkbox"
                      checked={transcriptionProcessingAcknowledged}
                      onChange={(e) => { setTranscriptionProcessingAcknowledged(e.target.checked); setTranscriptionMsg(''); }}
                      className="mt-0.5"
                    />
                    <span>Autorizzo Groq a elaborare le mie registrazioni vocali quando interrompo il microfono.</span>
                  </label>
                </SettingRow>
              </>
            )}
            <SettingRow label="Salva trascrizione" description={transcriptionMsg}>
              <button
                onClick={() => void saveTranscriptionSource()}
                disabled={!!transcriptionProvider && !transcriptionProcessingAcknowledged}
                className="text-sm px-3 py-1.5 rounded-lg outline-none cursor-pointer"
                style={{ background: 'var(--color-accent, var(--color-bg-tertiary))', color: 'var(--color-text)', border: '1px solid var(--color-border)' }}
              >
                Salva impostazioni voce
              </button>
            </SettingRow>
            <SettingRow label="Dettatura push-to-talk" description="Premi il microfono per registrare e premilo di nuovo per fermare. La trascrizione viene inserita nel campo di testo e resta modificabile prima dell’invio.">
              <button
                onClick={() => { updateSettings({ speechEnabled: !settings.speechEnabled }); showSaved(); }}
                className="relative w-11 h-6 rounded-full transition-colors cursor-pointer"
                style={{
                  background: settings.speechEnabled ? 'var(--color-accent)' : 'var(--color-bg-tertiary)',
                }}
              >
                <span
                  className="absolute top-0.5 left-0.5 w-5 h-5 rounded-full transition-transform bg-white"
                  style={{
                    transform: settings.speechEnabled ? 'translateX(20px)' : 'translateX(0)',
                    boxShadow: '0 1px 3px rgba(0,0,0,0.2)',
                  }}
                />
              </button>
            </SettingRow>
            <SettingRow label="Stato trascrizione" description="Controllato localmente dal runtime desktop; nessuna chiave è restituita all’interfaccia.">
              <div className="flex items-center gap-2">
                <span
                  className="w-2 h-2 rounded-full"
                  style={{ background: speechBackendAvailable ? 'var(--color-success)' : 'var(--color-text-tertiary)' }}
                />
                <span className="text-xs" style={{ color: 'var(--color-text-secondary)' }}>
                  {speechBackendAvailable === null ? 'Verifica in corso…' : speechBackendAvailable ? 'Groq Whisper disponibile' : 'Non configurata'}
                </span>
              </div>
            </SettingRow>
            <SettingRow label="Modalità conversazione" description="Disponibile ora come conversazione a turni: registra, controlla la trascrizione, invia, poi leggi la risposta. Non invia mai l’audio o il testo senza la tua azione.">
              <span className="text-xs" style={{ color: 'var(--color-text-secondary)' }}>A turni, non live</span>
            </SettingRow>
            <SettingRow label="Gemini 3.1 Flash Live" description="Conversazione audio-audio diretta con Google. La chiave Gemini resta nel portachiavi: all’avvio viene creato soltanto un token temporaneo, a uso singolo e vincolato al modello audio.">
              <label className="flex items-start gap-2 max-w-xs text-xs" style={{ color: 'var(--color-text-secondary)' }}>
                <input
                  type="checkbox"
                  checked={geminiLiveProcessingAcknowledged}
                  onChange={(e) => { setGeminiLiveProcessingAcknowledged(e.target.checked); setGeminiLiveMsg(''); }}
                  className="mt-0.5"
                />
                <span>Autorizzo Google a elaborare in tempo reale il microfono durante una sessione Gemini Live. So che TLS protegge il trasporto ma non è cifratura end-to-end verso Google.</span>
              </label>
            </SettingRow>
            <SettingRow label="Salva Gemini Live" description={geminiLiveMsg || 'Richiede una chiave Gemini già salvata nel portachiavi del sistema operativo.'}>
              <button
                onClick={() => void saveGeminiLiveConfig()}
                className="text-sm px-3 py-1.5 rounded-lg outline-none cursor-pointer"
                style={{ background: 'var(--color-accent, var(--color-bg-tertiary))', color: 'var(--color-text)', border: '1px solid var(--color-border)' }}
              >
                Salva Live
              </button>
            </SettingRow>
          </Section>

          {/* Data */}
          <Section title="Data">
            <SettingRow label="Conversations" description={`${conversations.length} stored locally`}>
              <div className="flex gap-2">
                <button
                  onClick={handleExport}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors cursor-pointer"
                  style={{ background: 'var(--color-bg-secondary)', color: 'var(--color-text-secondary)', border: '1px solid var(--color-border)' }}
                  onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--color-bg-tertiary)')}
                  onMouseLeave={(e) => (e.currentTarget.style.background = 'var(--color-bg-secondary)')}
                >
                  <Download size={12} /> Export
                </button>
                <button
                  onClick={handleImport}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors cursor-pointer"
                  style={{ background: 'var(--color-bg-secondary)', color: 'var(--color-text-secondary)', border: '1px solid var(--color-border)' }}
                  onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--color-bg-tertiary)')}
                  onMouseLeave={(e) => (e.currentTarget.style.background = 'var(--color-bg-secondary)')}
                >
                  <Upload size={12} /> Import
                </button>
              </div>
            </SettingRow>
            <SettingRow label="Clear all data" description="Permanently delete all conversations">
              <button
                onClick={handleClear}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors cursor-pointer"
                style={{
                  color: confirmClear ? 'white' : 'var(--color-error)',
                  background: confirmClear ? 'var(--color-error)' : 'transparent',
                  border: '1px solid var(--color-error)',
                }}
                onMouseEnter={(e) => { if (!confirmClear) e.currentTarget.style.background = 'rgba(220,38,38,0.1)'; }}
                onMouseLeave={(e) => { if (!confirmClear) e.currentTarget.style.background = 'transparent'; }}
              >
                <Trash2 size={12} /> {confirmClear ? 'Click again to confirm' : 'Clear'}
              </button>
            </SettingRow>
          </Section>

          {/* Diagnostic build only: capture a redacted navigation timeline.
              Renders nothing in the shipping 1.0.11 build. The captured
              JSON contains ONLY t / kind / pathname / mountCount /
              setupPhase / setupSource — no error text, no stack frames,
              no API keys, no full URLs, no file paths. The "Download
              JSON" button uses the same Blob + a.click() pattern as the
              "Export conversations" button above, so no Tauri dialog
              plugin is required. */}
          {isDiagBuild() && (
            <Section title="Diagnostics (diagnostic build only)">
              <p className="text-xs mb-3" style={{ color: 'var(--color-text-tertiary)' }}>
                Captures mount / unmount, pathname changes, popstate,
                hashchange, beforeunload, pagehide, visibilitychange, the
                boolean fact that an error was caught, and the setup
                phase / source enums. The downloaded file contains only
                those fields.
              </p>
              <div className="flex flex-wrap gap-2 mb-2">
                <button
                  type="button"
                  onClick={() => startRecording()}
                  disabled={isRecording()}
                  className="px-3 py-2 rounded text-xs font-medium transition-colors cursor-pointer disabled:opacity-50"
                  style={{ background: 'var(--color-accent-subtle)', color: 'var(--color-text)' }}
                >
                  Start recording
                </button>
                <button
                  type="button"
                  onClick={() => stopRecording()}
                  disabled={!isRecording()}
                  className="px-3 py-2 rounded text-xs font-medium transition-colors cursor-pointer disabled:opacity-50"
                  style={{ background: 'var(--color-bg-secondary)', color: 'var(--color-text-secondary)', border: '1px solid var(--color-border)' }}
                >
                  Stop recording
                </button>
                <button
                  type="button"
                  onClick={() => {
                    const json = dumpDiag();
                    const blob = new Blob([json], { type: 'application/json' });
                    const url = URL.createObjectURL(blob);
                    const a = document.createElement('a');
                    a.href = url;
                    a.download = `openjarvis-diag-${new Date().toISOString().replace(/[:.]/g, '-')}.json`;
                    document.body.appendChild(a);
                    a.click();
                    document.body.removeChild(a);
                    URL.revokeObjectURL(url);
                  }}
                  className="px-3 py-2 rounded text-xs font-medium transition-colors cursor-pointer"
                  style={{ background: 'var(--color-accent)', color: 'var(--color-on-accent)' }}
                >
                  Download JSON (web download)
                </button>
              </div>
              <p className="text-[11px]" style={{ color: 'var(--color-text-tertiary)' }}>
                Recording is currently {isRecording() ? 'on' : 'off'}. The
                downloaded file is local-only; no telemetry is sent.
              </p>
            </Section>
          )}

          {/* About */}
          <Section title="About">
            <div className="text-sm" style={{ color: 'var(--color-text-secondary)' }}>
              <p className="mb-2">
                <span className="font-semibold" style={{ color: 'var(--color-text)' }}>OpenJarvis</span> — Programming abstractions for on-device AI.
              </p>
              <p className="text-xs" style={{ color: 'var(--color-text-tertiary)' }}>
                Part of Intelligence Per Watt, a research initiative at Stanford SAIL.
              </p>
              <div className="flex gap-3 mt-3 text-xs">
                <a
                  href="https://openjarvis.stanford.edu/"
                  target="_blank"
                  rel="noopener noreferrer"
                  style={{ color: 'var(--color-accent)' }}
                >
                  Project site
                </a>
                <a
                  href="https://open-jarvis.github.io/OpenJarvis/"
                  target="_blank"
                  rel="noopener noreferrer"
                  style={{ color: 'var(--color-accent)' }}
                >
                  Documentation
                </a>
              </div>
            </div>
          </Section>
        </div>
      </div>
    </div>
  );
}
