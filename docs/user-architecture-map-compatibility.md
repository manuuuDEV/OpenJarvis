# Confronto con la mappa architetturale fornita dall’utente

## Esito della lettura integrale

La mappa ricevuta è stata letta fino alla sezione conclusiva **«Cosa non fare mai»**. È ora trattata come un insieme di vincoli di prodotto e di sicurezza da preservare prima di qualunque futura modifica.

> La mappa descrive un’applicazione **Node.js/TypeScript + Electron**, con un singolo `server.ts` e moduli quali `hardware_automation.ts`, `system_automation.ts`, `computer_use.ts` e `adb_bridge.*`. La working copy corrente è invece **OpenJarvis**: React/Vite, Tauri/Rust e backend Python/FastAPI. Non contiene Electron né i file citati nella mappa.

Questa non è una ragione per ignorare la mappa: significa che non è possibile dichiarare che le relative implementazioni siano già presenti in OpenJarvis soltanto perché sono descritte nel documento. Le loro regole utili devono essere confrontate e adattate, non copiate o duplicate.

## Confronto puntuale

| Area della mappa utente | Stato nella working copy OpenJarvis | Regola operativa risultante |
|---|---|---|
| Automazione browser con Playwright/browser-use | Non è presente come layer Electron/Node nella working copy verificata. | Non importare né duplicare quella architettura senza un confronto dedicato con il modello di sicurezza desktop esistente. |
| Mouse/tastiera OS con nut.js | Non presente. OpenJarvis adotta invece un broker Windows UI Automation privo di mouse/tastiera globali. | Non reintrodurre mouse, coordinate, `SendInput`, clipboard o shell nel broker controllato. |
| Computer use visivo con screenshot e click | Non presente nella forma descritta; il broker corrente agisce solo su elementi UI Automation verificati. | Conservare la scelta più restrittiva: niente screenshot/click a coordinate senza una nuova decisione di sicurezza. |
| Voce, Edge-TTS e Gemini Live della mappa | OpenJarvis ha Groq Whisper isolato e conversazione a turni; Gemini Live è intenzionalmente non attivo. | Non dichiarare Live disponibile finché non esista un bridge sicuro e verificato che non esponga chiavi cloud. |
| ADB con pure-python-adb e endpoint generici | Non presente. È stato aggiunto un broker ADB Rust separato, limitato a diagnostica software in sola lettura. | Non introdurre gli endpoint generici `/command`, `/tap`, `/text`, `/open_app`, `/connect` o `/screenshot`. |
| Sicurezza anti-malware, kill switch e sub-agenti | Non sono stati verificati come componenti equivalenti della working copy durante questo confronto. | Non presumere equivalenza; valutare separatamente ogni eventuale richiesta futura. |
| Provider universali e provider locali | La working copy segue il requisito cloud-only e l’allowlist UI concordata. | Non reintrodurre Ollama o modelli locali, né fallback cloud impliciti. |

## Vincoli che diventano permanenti per le prossime modifiche

| Vincolo | Applicazione in OpenJarvis |
|---|---|
| Nessun framework o server esterno monolitico | Integrare solo pattern strettamente necessari e nativi dello stack Tauri/Rust + Python esistente. |
| Nessuna dichiarazione assoluta senza evidenza | Distinguere sempre codice implementato, test locale, build Windows e prova reale sul dispositivo. |
| Non automatizzare account reali di terzi senza analisi dedicata | Restano bloccati login, credenziali, OTP, pagamenti, invii e flussi account sia su Windows sia su Android. |
| Non implementare chiamate autonome | Qualunque futuro sviluppo di telefonia o chiamate richiede una discussione di sicurezza indipendente. |
| Non duplicare lavoro già svolto | Prima di una nuova feature, confrontare questa mappa con i file e i test effettivamente presenti nella working copy. |

## Evidenza del confronto

Il controllo della struttura del repository corrente ha trovato `frontend/package.json`, `pyproject.toml` e `rust/Cargo.toml`; non ha trovato `electron-builder`, `server.ts`, `hardware_automation.ts`, `system_automation.ts`, `computer_use.ts` o `adb_bridge.*` come sorgenti del progetto. Ha invece trovato i broker correnti `frontend/src-tauri/src/desktop_broker.rs` e `frontend/src-tauri/src/android_adb_broker.rs`, oltre ai tool Python controllati corrispondenti.

Questo documento non modifica il codice né sostituisce la mappa originale dell’utente; ne preserva le decisioni utili e le differenze architetturali verificabili per evitare duplicazioni o regressioni.
