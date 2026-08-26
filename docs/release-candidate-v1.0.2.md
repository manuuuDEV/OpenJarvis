# OpenJarvis Desktop Secure v1.0.2 — Manifesto release candidate

## Stato

Questa working copy è una **release candidate locale**. Non è stata ancora inviata a GitHub, non è stato creato alcun tag e non esiste ancora un installer Windows pubblicato.

| Elemento | Stato |
|---|---|
| Versione frontend/Tauri | `1.0.2` |
| Profilo inferenza | Solo cloud, provider esplicito, selezionabile nelle Impostazioni e senza fallback implicito |
| Provider esposti in UI | Solo quelli richiesti dagli screenshot: Groq, Google Gemini, OpenRouter, NVIDIA NIM, SambaNova Cloud, Alibaba Cloud Model Studio, OpenAI, Pollinations, Hugging Face, Together AI |
| Modelli locali/Ollama | Non esposti da UI o comandi Tauri; configurazioni legacy rese cloud-only |
| Credenziali provider | Solo archivio credenziali del sistema operativo |
| Trascrizione voce | Groq Whisper configurabile separatamente, con consenso distinto e chiave isolata dal backend Python |
| Conversazione voce | Dettatura a turni disponibile; Gemini 3.1 Flash Live implementato con consenso separato, token effimero a uso singolo e audio WSS diretto, ma non ancora provato con chiave/microfono/rete reali |
| Azioni file/app | Limitate, approvabili, tracciate e monouso; solo retry strettamente identici riusano una sola approvazione pendente |
| Workspace controllati | Metadati VCS, configurazioni agente e IDE protetti: non leggibili, elencabili o scrivibili dagli strumenti cloud-facing |
| Operatore desktop | Broker UI Automation nativo implementato: token per avvio, claim anti-replay, verifica processo/elemento, Invoke/Value e audit redatto; richiede ancora build e test Windows reale |
| Android ADB | Modulo nativo opzionale e approval-gated per diagnostica software in sola lettura su un singolo device scelto localmente; non espone shell ADB, controllo touch, apertura app, file, root o modifiche al telefono; richiede ancora compilazione e test Windows/Android reale |
| Verifica Jarvis | Nuovo report locale senza effetti collaterali per provider, backend, voce, ADB, broker Windows, browser controllato e Gemini Live; non avvia broker, audio, browser, device o token |
| Browser controllato | Playwright già integrato viene ulteriormente vincolato nel profilo desktop: HTTPS pubblico e lettura consentiti; login, credenziali, pagamenti, invii, eliminazioni, URL con segreti/credenziali, IDN e accorciatori sono bloccati localmente |
| Backend runtime | Staged dal commit revisionato durante la build; snapshot generati ignorati da Git |
| Auto-update e analytics | Disabilitati |

## Controlli completati

| Verifica | Esito locale |
|---|---|
| Lint Ruff delle aree modificate | Superato |
| Router cloud desktop + provider espliciti | `8 passed` |
| Speech discovery + Groq Whisper | `12 passed` |
| Route approvazione e lifecycle broker desktop | `3 passed` |
| Regressione combinata provider/broker/speech | `23 passed` |
| Build frontend desktop | Superata con `npm run build` |
| `cargo check` Linux | Superato |
| Test desktop Rust su Linux | `34 passed` |
| `git diff --check` dopo ADB | Superato |
| Tool ADB, lifecycle approvazioni e policy prompt | `11 passed` |
| Build frontend con sezione ADB | Superata con `npm run build` |
| Test Rust Linux dopo integrazione ADB | `34 passed` |
| Self-test, Gemini Live e browser controllato — Rust Linux | `36 passed` |
| Policy browser + policy prompt + ADB/approvazioni | `17 passed` |
| Lint Ruff delle nuove policy browser/prompt | Superato |
| Build frontend con self-test e Gemini Live | Superata con `npm run build` |
| `git diff --check` dopo integrazione selettiva | Superato |
| Workspace protetto + lifecycle approvazioni, inclusa deduplicazione retry | `18 passed` |
| Regressione sicurezza combinata: locale, approvazioni, browser, desktop, ADB e prompt | `38 passed` |
| Lint Ruff su workspace, store approvazioni e relativi test | Superato |
| `git diff --check` dopo workspace protetto e deduplicazione | Superato |
| Regressione Python finale delle aree modificate | `375 passed`, `6 skipped`; warning FastAPI di deprecazione presenti |
| Build frontend finale | Superata con `npm run build` |
| Test Rust/Tauri Linux finale | `36 passed` |
| Suite Python completa upstream | **Non verde**: `7944 passed`, `121 failed`, `71 skipped`; i fallimenti osservati sono diffusi e in larga parte non riconducibili a questa tranche (skills live, browser AX tree/Playwright, git/http tools, MCP, storage stubs, timeout, web search e altri) |

Le verifiche FastAPI producono warning di deprecazione relativi agli handler `on_event`; non sono errori funzionali della release candidate, ma restano debito tecnico dell’upstream. La suite Python totale del repository non può quindi essere presentata come completamente risolta in questa sessione.

## Vincoli applicati

Le cartelle esterne sono scelte manualmente dall’utente nelle Impostazioni e sono limitate a otto directory esistenti, non di sistema e non private. Il modello non può aggiungerle. All’interno di tali root, i metadati di controllo del progetto sono esclusi prima di ogni approvazione: directory `.git`, `.hg`, `.svn`, `.agents`, `.claude`, `.codex`, `.cursor`, `.vscode`, `.idea` e file `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `INSTRUCTIONS.md`, `MCP.json` non possono essere letti, elencati né scritti dagli strumenti cloud-facing. Il confine riprende selettivamente il concetto di *protected paths* di Open Interpreter, senza incorporarne runtime o dipendenze. Le scritture, le directory, l’apertura di app e l’apertura di documenti sono proposte ad alto rischio, con approvazione locale monouso e audit. Solo retry strettamente identici — stesso tipo, descrizione, chiave di permesso, livello di rischio e payload canonico — riusano una proposta pendente; ogni azione semanticamente diversa resta visibile per una revisione distinta. Dopo approvazione resta valido l’usuale claim monouso del broker. L’operatore desktop propone piani strutturati limitati a una finestra non elevata e a elementi UI ammessi; il broker Windows rivendica il piano in modo monouso e ne verifica di nuovo processo, finestra ed elemento prima dell’azione. Login, password, OTP, banca, pagamenti, acquisti, recupero account e invii restano rifiutati. Il completamento del piano desktop ora invia il riepilogo nel corpo JSON autenticato, non nella query string. Inoltre il broker applica una redazione conservativa preventiva al testo accessibile prima di restituirlo al backend locale. L’esecuzione UI nativa non deve però essere dichiarata convalidata finché non sarà compilata e provata su Windows.

L’apertura di app richiede un eseguibile assoluto, senza argomenti; shell, host di script, installer e utility amministrative sono bloccati. L’apertura documento ammette soltanto formati non macro in una cartella autorizzata e usa l’associazione scelta dall’utente. Il broker UI Automation non usa mouse o tastiera globali: opera soltanto su elementi accessibili verificati della singola finestra approvata; non può trattare password, OTP, pagamenti, login, shell o finestre elevate.

Il modulo Android ADB richiede che l’utente imposti manualmente il percorso di `adb.exe` sotto `platform-tools`, rilevi il proprio dispositivo e accetti in Android il dialogo RSA di USB debugging. Il seriale e il percorso restano nelle impostazioni native locali. L’agente può solo proporre una diagnostica, sempre con approvazione monouso; il broker accetta unicamente letture allowlisted su versione Android, dimensioni schermo, spazio dati, memoria, batteria e conteggio app. Non può inviare una shell arbitraria, input touch/tastiera, aprire app, installare/disinstallare, trasferire file, abilitare connessioni wireless, usare root, raccogliere log completi o catturare schermi.

Il report **Verifica Jarvis** legge soltanto configurazione locale, presenza di credenziali nel keyring e health del backend. Non avvia broker, browser o ADB, non registra audio, non crea token Gemini e non manda dati al cloud. Indica esplicitamente con `Prova reale richiesta` i componenti che non possono essere convalidati nel sandbox.

Il browser Playwright mantiene il controllo SSRF esistente e, nel profilo desktop sicuro, applica un preflight locale: richiede HTTPS e rifiuta URL con credenziali o valori di sessione/token, host punycode e accorciatori. Esso non è un servizio antimalware o un verdetto di reputazione; nessun URL viene trasmesso a terzi. Login, dati personali, OTP, pagamenti, invii, pubblicazioni e azioni distruttive sono bloccati prima di Playwright.

Gemini Live usa il flusso ufficiale di token effimeri: il runtime Rust legge `GEMINI_API_KEY` solo dal keyring, richiede un token monouso vincolato a `gemini-3.1-flash-live-preview` e `AUDIO`, e il renderer usa quel token in memoria per la sessione WSS diretta. La chiave lunga non entra in TypeScript, URL, log o backend Python; token e audio non sono persistiti. Google elabora il microfono se l’utente abilita e avvia manualmente la sessione; TLS protegge il trasporto ma non è cifratura end-to-end. La sessione non dichiara tool, camera, schermo o Google Search e viene chiusa se il provider richiede tool calling.

Le risposte e le anteprime sono filtrate localmente per credenziali e categorie comuni di dati personali. La trascrizione Groq Whisper richiede un consenso separato e non riusa automaticamente il provider di inferenza: la chiave Groq resta nel portachiavi e non viene inoltrata al processo Python di chat. Questo filtro resta **best-effort**, non una promessa di DLP universale o di protezione da ogni forma di dato personale.

## Passaggi necessari prima della pubblicazione

| Passaggio | Dipendenza |
|---|---|
| Commit e push della working copy | Consenso esplicito dell’utente |
| Build privata con `.msi` e `.exe` | Commit su branch del fork e avvio manuale del workflow; nessuna tag o release pubblica |
| Firma code-signing | `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD`, `WINDOWS_TIMESTAMP_URL` configurati come segreti GitHub |
| Test installazione/avvio | PC Windows reale, con controllo manuale Defender/SmartScreen e portachiavi |
| Test ADB Android | PC Windows reale con Android USB sbloccato, USB debugging attivo e autorizzazione RSA approvata dall’utente; verificare device assente, `unauthorized`, `offline`, più device e diagnostica riuscita |
| Test Gemini Live | App Windows reale, consenso salvato, chiave Gemini nel keyring, token effimero, microfono, audio PCM, stop manuale, rete assente e rifiuto tool call; non conservare registrazioni o token nei log |
| Test browser controllato | Build Windows reale: lettura HTTPS, blocco login/credentiali/pagamenti/invio, blocco URL con segreti/IDN/accorciatore e conferma che l’SSRF continua a rifiutare destinazioni private |
| Hash dell’installer | Calcolabile solo dopo la build Windows finale |

> Finché non esistono certificato e test su Windows reale, l’installer deve essere presentato come **non firmato e non ancora convalidato sul dispositivo finale**, non come privo di avvisi o privo di bug.
