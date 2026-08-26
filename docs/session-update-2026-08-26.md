# OpenJarvis Desktop v1.0.2 — aggiornamento locale della working copy

## Cosa è stato completato in questa sessione

La working copy locale in `/home/ubuntu/openjarvis-local` è stata estesa in modo coerente con il profilo **cloud-only** e **privacy-first** già impostato.

| Area | Risultato |
|---|---|
| Provider cloud in Impostazioni | La UI e il runtime sono ora allineati ai soli provider richiesti dagli screenshot: Groq, Google Gemini, OpenRouter, NVIDIA NIM, SambaNova Cloud, Alibaba Cloud Model Studio, OpenAI, Pollinations, Hugging Face e Together AI. |
| Selezione provider attivo | È rimasta la regola di sicurezza per cui **un solo provider attivo** viene autorizzato e inoltrato al backend alla volta; le altre chiavi restano soltanto nel portachiavi del sistema operativo. |
| Fallback impliciti | Rimossi nel profilo desktop sicuro: il router cloud non inferisce più il provider dal nome del modello quando il profilo desktop è attivo. |
| Pollinations | Corretto il base URL canonico `https://gen.pollinations.ai`, evitando la situazione in cui il provider era visibile in UI ma non instradabile. |
| Trascrizione voce | Implementata una configurazione separata per **Groq Whisper**, con consenso distinto per l’audio e chiave isolata dal backend Python. |
| Esperienza voce | La modalità disponibile è ora dichiarata correttamente come **conversazione a turni**: registrazione, stop, trascrizione, revisione nel campo chat, invio manuale. |
| Gemini Live | Implementato il flusso audio-audio: consenso separato, chiave Gemini solo nel keyring Rust, token effimero monouso vincolato al modello e audio, WebSocket WSS diretto e microfono PCM. Il renderer non persiste token o audio e chiude la sessione davanti a tool call. Resta non provato con account, microfono e Windows reali. |
| Verifica Jarvis | Aggiunto un report nativo senza effetti collaterali per configurazione cloud, backend, trascrizione, ADB, browser controllato, broker Windows e Gemini Live. Non avvia browser/broker/device/audio, non crea token e non invia dati cloud. |
| Browser controllato | L’integrazione Playwright esistente è stata rafforzata: nel profilo desktop sicuro richiede HTTPS, blocca URL con credenziali/token/sessione, host punycode e accorciatori; blocca anche login, campi account/identità, OTP, pagamenti, invii, pubblicazioni ed eliminazioni. |
| Broker desktop Windows | Migliorato il completamento dei piani UI Automation: il riepilogo non passa più in query string ma in corpo JSON autenticato; aggiunta una redazione preventiva del testo accessibile prima del ritorno al backend. |
| Android ADB | Aggiunto un modulo opzionale per sola diagnostica software Android: l’utente configura localmente `adb.exe` di Platform Tools, rileva e seleziona un solo device e conferma il perimetro. Il modello può soltanto proporre la scansione, mai una shell ADB o input al telefono. |
| Workspace protetto | Adattato in modo nativo il pattern *protected paths* di Open Interpreter: nelle root autorizzate il modello non può leggere, elencare o scrivere metadati VCS, configurazioni di agenti o IDE. Non è stato incorporato alcun runtime esterno. |
| Retry approvazioni | Solo proposte pendenti strettamente identiche — tipo, descrizione, chiave di permesso, livello di rischio e payload canonico — sono deduplicate; azioni semanticamente diverse restano card separate e il claim del broker resta monouso. |

## Validazioni locali effettivamente rieseguite

| Verifica | Esito |
|---|---|
| `uv run ruff check` sui file modificati | Superato |
| `uv run pytest tests/server/test_cloud_router.py -q` | `8 passed` |
| `uv run pytest tests/speech/test_discovery.py -q` | `8 passed` |
| `uv run pytest tests/speech/test_groq_whisper.py tests/speech/test_discovery.py -q` | `12 passed` |
| `uv run pytest tests/security/test_controlled_local_approval_route.py -q` | `3 passed` |
| `uv run pytest tests/server/test_cloud_router.py tests/security/test_controlled_local_approval_route.py tests/speech/test_discovery.py tests/speech/test_groq_whisper.py -q` | `23 passed` |
| `cargo check` su Linux | Superato |
| `cargo test` su Linux | `34 passed` |
| `npm run build` nel frontend | Superato |
| `git diff --check` | Superato anche nel controllo conclusivo dopo l’estensione ADB |
| `cargo fmt -- --check` | Non eseguibile: il componente `rustfmt` non è installato nel toolchain locale; non è stato installato solo per la sessione |
| `uv run ruff check` su tool ADB, policy, route e test | Superato |
| Test ADB, approvazioni e policy prompt | `11 passed` |
| `cargo test` dopo integrazione ADB | `34 passed` su Linux |
| Build frontend dopo sezione ADB | Superata con `npm run build` |
| Lint policy browser e prompt desktop | Superato |
| Test policy browser, prompt, ADB e approval lifecycle | `17 passed` |
| Test Rust con self-test e Gemini Live | `36 passed` su Linux |
| Build frontend con self-test e Gemini Live | Superata con `npm run build` |
| Lint Ruff su workspace protetto, store approvazioni e test | Superato |
| Workspace protetto e lifecycle/deduplicazione approvazioni | `18 passed` |
| Regressione sicurezza combinata: locale, approvazioni, browser, desktop, ADB e prompt | `38 passed` |
| `git diff --check` dopo le ultime modifiche | Superato |
| Regressione Python finale sulle aree modificate | `375 passed`, `6 skipped`; presenti warning FastAPI di deprecazione |
| Build frontend finale | Superata con `npm run build` |
| Test Rust/Tauri finale su Linux | `36 passed` |

## Limiti residui dichiarati apertamente

| Tema | Stato reale |
|---|---|
| Build Windows reale | Non eseguita in questa sessione. |
| Compilazione effettiva del broker UI Automation su Windows | Non ancora verificata su toolchain Windows reale. |
| Compilazione ed esecuzione del broker ADB su Windows con Android reale | Non eseguita: il target Rust Windows non è installato nell’ambiente locale e qui non è disponibile `adb` né un device Android autorizzato. |
| Installer `.exe` / `.msi` finale | Non prodotto in questa sessione. |
| Firma del binario | Non eseguita. |
| Smoke test installazione su PC Windows | Non eseguito. |
| Gemini Live audio-audio | Codice implementato e build TypeScript/Rust superate, ma non provato con chiave/account Gemini, token provisioning, microfono, audio di risposta, rete o Windows reale. Non va dichiarato operativo finché non sarà eseguito questo smoke test. |
| Suite Python completa del repository | Non verde: `7944 passed`, `121 failed`, `71 skipped`; i fallimenti sono diffusi in aree non limitate a questa tranche locale. |

## File principali aggiornati in questa sessione

| File | Scopo |
|---|---|
| `frontend/src-tauri/src/lib.rs` | Runtime desktop: isolamento chiavi, configurazione provider, configurazione trascrizione Groq, relay nativo audio, nuovi comandi Tauri. |
| `frontend/src/pages/SettingsPage.tsx` | UI provider multipli, consenso cloud, consenso audio Groq, stato voce e chiarimento sulla modalità a turni. |
| `frontend/src/lib/api.ts` | Contratti frontend per provider, stato trascrizione e comandi nativi. |
| `frontend/src/hooks/useSpeech.ts` | Aggiornamento dinamico dello stato disponibilità trascrizione. |
| `src/openjarvis/server/cloud_router.py` | Provider esplicito, nessun fallback desktop, Pollinations corretto. |
| `src/openjarvis/server/approval_routes.py` | Completamento broker via JSON autenticato. |
| `src/openjarvis/tools/controlled_local.py` | Risoluzione delle azioni locali con esclusione preventiva dei metadati VCS, agenti e IDE nelle root autorizzate. |
| `src/openjarvis/tools/approval_store.py` | Duplicazione delle proposte pendenti evitata per retry identici, senza indebolire approvazione, TTL o claim monouso. |
| `frontend/src-tauri/src/desktop_broker.rs` | Pre-redazione testo accessibile e POST JSON del completion payload. |
| `frontend/src-tauri/src/android_adb_broker.rs` | Broker Windows separato, con token per avvio, claim monouso, allowlist diagnostica, timeout, parsing locale e riepilogo limitato. |
| `src/openjarvis/tools/controlled_android_adb.py` | Unici tool ADB visibili al modello: proposta di diagnostica e lettura dell’esito redatto. |
| `docs/controlled-adb-android-design.md` | Design di sicurezza, comandi consentiti ed esclusioni tecniche. |
| `src/openjarvis/speech/groq_whisper.py` | Backend Groq Whisper server-side compatibile OpenAI. |
| `src/openjarvis/speech/_discovery.py` | Blocco discovery locale nel profilo desktop e selezione Groq Whisper esplicita. |
| `frontend/src/hooks/useGeminiLive.ts` | Client Live PCM/WSS con stop manuale, senza persistenza di audio/token e senza tool/camera/schermo. |
| `src/openjarvis/security/browser_policy.py` | Preflight locale URL e blocchi interazione sensibile del browser sicuro. |
| `frontend/src-tauri/src/lib.rs` | Aggiunti self-test nativo e provisioning token Gemini Live vincolato. |
| `docs/candidate-integration-roadmap.md` | Valutazione dei repository/pattern della mappa e decisioni di integrazione selettiva. |
| `docs/windows-smoke-test-v1.0.2.md` | Istruzioni concrete per installare l’artefatto privato e provare ciascun perimetro su Windows senza testare flussi sensibili. |

## Prossimo passo logico

Il prossimo passo concreto, quando lo vorrai, sarà fare ciò che qui non è stato ancora dichiarato come completato: **commit/push della working copy soltanto con tua autorizzazione**, poi **build Windows privata**, quindi **smoke test reale su Windows** per verificare broker UI Automation, broker ADB con Android USB autorizzato, portachiavi, Gemini Live con audio reale, browser controllato, installer e avvio finale.
