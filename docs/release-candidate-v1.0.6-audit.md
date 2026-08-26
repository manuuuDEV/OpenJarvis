# OpenJarvis Desktop Cloud 1.0.6 — audit finale del candidato

**Stato:** build privata Windows completata; **non** ancora validata su un PC Windows reale.
**Repository e commit:** [`manuuuDEV/OpenJarvis`, `75a3aaa`](https://github.com/manuuuDEV/OpenJarvis/commit/75a3aaa7bb74c3887f23cc50d9cc28c66591e9c9).
**Branch:** `release/v1.0.2-rc1`.
**Data dell’audit:** 26 agosto 2026.

## Conclusione breve

Il candidato **1.0.6** contiene una correzione di perimetro concreta: la versione desktop non accetta più un URL di backend o una API key legacy salvati dal renderer. Quando Tauri è attivo, il renderer usa soltanto l’endpoint loopback pubblicato dal processo Rust; i campi legacy **API URL** e **API key** sono nascosti dalla schermata Impostazioni desktop. Questo evita che un valore residuo in `localStorage` possa reindirizzare richieste o un token Bearer a un host arbitrario.

La build privata Windows del commit indicato ha completato test Rust, bundling NSIS/MSI e upload dell’artefatto nella CI.[1] Questo dimostra che il codice è compilabile dal runner Windows; **non** dimostra ancora il funzionamento con WebView2, Credential Manager, microfono, Windows UI Automation o un dispositivo Android reale.

> Non è corretto chiamare questa app “senza bug”, “sicura al 100%” o “la più privata del mercato”. Il report documenta ciò che è implementato e testato, oltre a ciò che rimane da provare.

## Correzione inclusa nella 1.0.6

| Componente | Correzione | Risultato verificabile |
|---|---|---|
| `frontend/src/lib/api.ts` | `selectApiBase()` dà priorità all’endpoint nativo loopback in Tauri. | Un valore `apiUrl` remoto persistito non viene usato dalla build desktop. |
| `frontend/src/lib/api.ts` | `getApiKey()` restituisce vuoto in Tauri. | Una API key legacy in storage non viene inviata da questa UI verso un endpoint configurato dal renderer. |
| `SettingsPage` | La build desktop mostra “Backend desktop” invece dei campi legacy URL/API key. | Il backend nativo non è configurabile dalla UI verso host remoti. |
| Regressione frontend | Aggiunti tre casi per endpoint nativo, fallback loopback e comportamento web separato. | La modifica è coperta da test automatico. |
| Test deployment/prompt | Riallineati alle scelte deliberate: CSP limitata al backend locale, auto-update disabilitato e policy desktop non aggirabile. | Le difese non risultano più falsamente considerate regressioni upstream. |

## Risultati di verifica

| Ambito | Esito | Significato e limite |
|---|---:|---|
| Suite Python completa `uv run pytest -q` | **7.965 passati, 115 falliti, 71 saltati, 204 warning** | La suite upstream completa resta non verde; non è stato nascosto questo risultato. |
| Deployment e prompt desktop | **14 passati** | Coprono CSP loopback, auto-update disabilitato e policy prompt obbligatoria. |
| Browser, azioni locali, approvazioni e ADB | **56 passati** | Coprono le policy Python in isolamento; non sostituiscono le API Windows reali. |
| Frontend sorgente | **13 file, 59 test passati** | Include la regressione sul confinamento del backend desktop. |
| Build frontend TypeScript/Vite | **Passata** | Il bundle viene prodotto; Vite segnala un chunk JavaScript principale grande, da ottimizzare separatamente ma non bloccante. |
| Rust/Tauri Linux | **36 passati** | Copre logica nativa testabile su Linux, non UI Automation Windows reale. |
| CI Windows | **Passata** | Il runner Windows ha eseguito build privata e upload di NSIS/MSI.[1] |

I 115 fallimenti Python residui appartengono soprattutto a integrazioni live senza credenziali/provider, engine locali e mining deliberatamente disabilitati, test AX tree senza Playwright installato, skill/inference engine non presenti e aspettative upstream su tool HTTP, Git, MCP, storage e timeout. Non sono stati resi artificialmente verdi. Di conseguenza, quei sottosistemi upstream non devono essere dichiarati verificati per questa release desktop.

## Matrice di capacità effettive

### Cloud, credenziali e privacy

| Capacità | Stato del codice | Limite da dichiarare |
|---|---|---|
| Provider selezionabili | Implementata una allowlist con **Groq, Google Gemini, OpenRouter, NVIDIA NIM, SambaNova Cloud, Alibaba Cloud Model Studio, OpenAI, Pollinations, Hugging Face e Together AI**. | Compatibilità di modello/endpoint e disponibilità dipendono dal provider e dalla chiave dell’utente. |
| Chiavi provider | Salvate tramite keyring del sistema operativo; non devono essere archiviate in repository, log o configurazioni applicative. | La persistenza effettiva in Windows Credential Manager richiede smoke test su Windows. |
| Provider attivo | Un solo provider attivo alla volta e nessun fallback implicito. | Un profilo incompleto impedisce l’utilizzo finché non viene configurato. |
| Cloud-only | Il profilo desktop usa inference cloud e ripristina configurazioni locali legacy al profilo cloud. | Residui di codice upstream locale non equivalgono a modelli installati o avviati dal bundle desktop. |
| Privacy trasporto | TLS/WSS protegge il transito verso il provider scelto. | Non è cifratura end-to-end contro il provider: prompt e risposte vengono elaborati dal provider autorizzato. |
| Trascrizione | Groq Whisper tramite relay nativo e consenso separato. | Non testata con microfono, account e rete reali. |
| Live audio | Gemini Live audio-only con token effimero monouso e rifiuto tool call. | Non testato con microfono/output audio e API key reali. |

### Browser, file, app e Android

| Area | Operazioni confinabili | Bloccato intenzionalmente |
|---|---|---|
| Browser | Policy HTTPS pubblica, difese SSRF/IDN/URL con credenziali e controlli sulle richieste. | Login, password, OTP, recupero account, banca, pagamenti, acquisti, pubblicazione, cancellazioni e typing sensibile. |
| File locali | Solo cartelle approvate; proposta, approvazione, claim monouso e audit. | Percorsi protetti Git/VCS, configurazioni agenti/IDE e percorsi fuori allowlist. |
| App e finestre Windows | UI Automation ristretta: verifica processo/finestra/elemento, inspect/read/invoke/set-text nell’ambito autorizzato. | Mouse/tastiera/clipboard globali, coordinate, `SendInput`, shell generica, script host, installazione software ed elevazione. |
| Android ADB | Diagnostica read-only sottoposta ad approvazione. | Shell arbitraria, input, apertura app, installazione, file transfer e root. |

“Controllare applicazioni” significa quindi **automazione UIA approvata e verificabile**, non controllo indiscriminato del PC. Il modello cloud non riceve una capacità generale di muovere il mouse, digitare ovunque o eseguire comandi arbitrari.

## Pattern esterni usati ed esclusi

Sono stati adottati selettivamente pattern di self-test senza effetti collaterali, policy browser, protezioni per Gemini Live, workspace protetto e deduplicazione di retry/approvazioni. Non sono stati importati framework completi quali Open Interpreter, OpenHands, Agent-S, CUA, browser-use, nut.js, pure-python-adb o mem0. L’esclusione evita di aggiungere server, privilegi, superfici di attacco o duplicazioni incompatibili con il profilo cloud-only e approval-gated.

## Artefatti Windows verificati

La CI ha prodotto l’artefatto privato `openjarvis-desktop-windows-cloud-8` per la run completata con successo.[1] L’archivio è stato scaricato e verificato localmente; gli hash seguenti si riferiscono ai file estratti.

| File | SHA-256 |
|---|---|
| `OpenJarvis_1.0.6_x64-setup.exe` | `35887d7fbec1a52502ef150c2873ad11ae3ff58b7ff452c1fa1836078118fc11` |
| `OpenJarvis_1.0.6_x64_en-US.msi` | `5f60c160762069c0fdc994469b19f0558cb13265844ea7f60b19a59ebf6d576f` |

Non è stato fornito un certificato di firma codice per questa build. L’installer va quindi trattato come **non firmato** finché una verifica con strumenti Windows non dimostri diversamente. SmartScreen può avvisare; non bisogna disabilitare Defender o altre protezioni in modo globale.

## Smoke test Windows ancora obbligatori

Prima di considerare il candidato utilizzabile, eseguire sul PC Windows una prova manuale breve e reversibile.

1. Scaricare l’artefatto dalla run CI privata e confrontare l’hash dell’EXE con quello indicato sopra.
2. Aprire l’app senza inserire chiavi e navigare: **Chat → Dashboard → Logs → Settings → Chat**. Nessuna pagina deve tornare autonomamente a Settings.
3. In Impostazioni verificare i dieci provider e l’assenza di campi API URL/API key per il backend desktop.
4. Salvare una chiave di test non sensibile, selezionare provider e modello, riavviare e verificare che la chiave non sia mostrata in chiaro e risulti ancora presente.
5. Eseguire “Verifica Jarvis” senza microfono, device o azioni locali; il self-test deve rimanere privo di effetti collaterali.
6. Solo con una cartella di prova e un’app innocua, verificare separatamente una proposta file approvata e una singola operazione UIA. Gemini Live e Android ADB devono essere provati in sessioni separate con consenso esplicito.

## Riferimenti

[1]: https://github.com/manuuuDEV/OpenJarvis/actions/runs/33002040945 "GitHub Actions — Desktop Build & Release, run 33002040945"
