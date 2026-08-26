import { Cloud, KeyRound, Settings, ShieldCheck, X } from 'lucide-react';
import { useNavigate } from 'react-router';
import { useAppStore } from '../lib/store';

/**
 * Secure desktop entry point for inference configuration.
 *
 * The upstream command palette mixed Ollama downloads, a small provider subset,
 * and direct API-key fields. The cloud-only desktop profile deliberately has one
 * configuration surface: Settings. This prevents duplicate key storage paths,
 * prevents accidental local-model downloads, and ensures provider consent is
 * recorded before a backend can start.
 */
export function CommandPalette() {
  const navigate = useNavigate();
  const setCommandPaletteOpen = useAppStore((state) => state.setCommandPaletteOpen);

  const openSettings = () => {
    setCommandPaletteOpen(false);
    navigate('/settings');
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center pt-[15vh]"
      onClick={() => setCommandPaletteOpen(false)}
      role="presentation"
    >
      <div className="fixed inset-0" style={{ background: 'rgba(0,0,0,0.5)' }} />
      <section
        aria-label="Cloud configuration"
        className="relative w-full max-w-lg overflow-hidden rounded-xl"
        style={{
          background: 'var(--color-surface)',
          border: '1px solid var(--color-border)',
          boxShadow: 'var(--shadow-lg)',
        }}
        onClick={(event) => event.stopPropagation()}
      >
        <header
          className="flex items-center justify-between px-5 py-4"
          style={{ borderBottom: '1px solid var(--color-border)' }}
        >
          <div className="flex items-center gap-2.5">
            <Cloud size={18} style={{ color: 'var(--color-accent)' }} />
            <div>
              <h2 className="text-sm font-semibold" style={{ color: 'var(--color-text)' }}>
                Configurazione cloud
              </h2>
              <p className="text-[11px]" style={{ color: 'var(--color-text-tertiary)' }}>
                Un provider autorizzato e un modello attivo alla volta
              </p>
            </div>
          </div>
          <button
            aria-label="Chiudi"
            className="rounded p-1.5 cursor-pointer"
            onClick={() => setCommandPaletteOpen(false)}
            style={{ color: 'var(--color-text-tertiary)' }}
          >
            <X size={16} />
          </button>
        </header>

        <div className="space-y-3 px-5 py-5">
          <div className="flex gap-3 rounded-lg p-3" style={{ background: 'var(--color-bg-secondary)' }}>
            <KeyRound size={16} className="mt-0.5 shrink-0" style={{ color: 'var(--color-accent)' }} />
            <p className="text-xs leading-5" style={{ color: 'var(--color-text-secondary)' }}>
              Le chiavi dei provider sono gestite esclusivamente dal portachiavi del sistema operativo.
              Questa finestra non salva né mostra chiavi e non abilita download di modelli locali.
            </p>
          </div>
          <div className="flex gap-3 rounded-lg p-3" style={{ background: 'var(--color-bg-secondary)' }}>
            <ShieldCheck size={16} className="mt-0.5 shrink-0" style={{ color: 'var(--color-success)' }} />
            <p className="text-xs leading-5" style={{ color: 'var(--color-text-secondary)' }}>
              In Impostazioni puoi selezionare i provider autorizzati, leggere il consenso al trattamento
              cloud e avviare Verifica Jarvis prima di usare funzioni locali controllate.
            </p>
          </div>
          <button
            className="flex w-full items-center justify-center gap-2 rounded-lg px-4 py-2.5 text-sm font-medium cursor-pointer"
            onClick={openSettings}
            style={{ background: 'var(--color-accent)', color: 'var(--color-on-accent)' }}
          >
            <Settings size={16} />
            Apri Impostazioni sicure
          </button>
        </div>
      </section>
    </div>
  );
}
