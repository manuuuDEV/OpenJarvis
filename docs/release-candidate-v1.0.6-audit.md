# OpenJarvis Desktop Cloud 1.0.6 — audit di rilascio

**Stato del documento:** candidato di rilascio, in attesa di build Windows e smoke test su un PC Windows reale.
**Ambito:** fork `manuuuDEV/OpenJarvis`, branch `release/v1.0.2-rc1`.
**Data dell’audit:** 26 agosto 2026.

## Sintesi onesta

La versione **1.0.6** corregge un difetto di perimetro individuato durante il riesame: nella build Tauri, un vecchio valore `apiUrl` o `apiKey` salvato in `localStorage` poteva ancora essere considerato dal renderer. La build desktop ora ignora tali valori, usa esclusivamente l’endpoint locale avviato dal processo Rust e nasconde i campi legacy di URL/API key. Ciò non modifica gli endpoint dei provider cloud: essi continuano a essere configurati separatamente, con chiavi nel portachiavi del sistema operativo e con consenso esplicito.

> **Non è corretto dichiarare l’app “senza bug”, “al 100% verificata” o “la più sicura del mercato”.** I test del codice sono positivi per i confini introdotti, ma il comportamento di Windows UI Automation, keyring, microfono, Gemini Live e ADB richiede ancora prove con hardware e account reali.

| Area | Stato al candidato 1.0.6 | Evidenza disponibile |
|---|---|---|
| Confinamento renderer–backend desktop | Implementato e testato | Il renderer Tauri ignora URL/API key legacy e resta sul backend locale nativo; 3 regressioni frontend passate. |
| Inference cloud-only | Implementato nel profilo desktop | Il bootstrap nativo usa un profilo cloud; le configurazioni locali legacy vengono ripristinate al profilo cloud. |
| Provider e chiavi | Implementato nel codice | Dieci provider mostrati nelle Impostazioni; una sola scelta attiva; chiavi nel keyring OS. |
| Auto-update e telemetria | Disabilitati per il profilo | Workflow e configurazione non producono metadata di aggiornamento; non si fa affidamento su analytics automatiche. |
| Sicurezza browser/local/ADB | Implementata e testata in isolamento | 56 test mirati passati su policy browser, approvazioni, audit e ADB read-only. |
| Esecuzione Windows reale | Non ancora dimostrata per 1.0.6 | Necessari installazione e smoke test dell’utente dopo la build CI. |

## Correzione introdotta in 1.0.6

La UI upstream conservava una sezione **Connection** pensata per un’installazione web/server generica. In una build privacy-first questa possibilità è indesiderata: un valore residuo in storage non deve poter portare il renderer a inviare richieste e un eventuale token Bearer verso un host remoto. Per questo motivo la selezione dell’endpoint ora distingue esplicitamente la build desktop dalla versione web.

| Componente | Correzione | Effetto verificabile |
|---|---|---|
| `frontend/src/lib/api.ts` | `selectApiBase()` privilegia sempre l’endpoint nativo o il fallback loopback quando Tauri è attivo. | Un `apiUrl` remoto persistito non viene usato dal renderer desktop. |
| `frontend/src/lib/api.ts` | `getApiKey()` restituisce vuoto in Tauri. | Un’API key legacy nello storage non viene inviata a un host configurato dal renderer. |
| `frontend/src/pages/SettingsPage.tsx` | I campi legacy API URL/API key sono sostituiti da una nota informativa nella build desktop. | L’utente configura provider cloud, non un backend remoto arbitrario. |
| `frontend/src/lib/api.desktop-network-policy.test.ts` | Nuova regressione a tre casi. | Copre endpoint nativo, fallback loopback e comportamento web separato. |
| Test di deployment e prompt | Asserzioni riallineate alle decisioni deliberate. | La CSP chiusa e la policy desktop obbligatoria non sono più segnalate come regressioni upstream. |

## Funzioni presenti e loro confini

### Cloud, privacy e credenziali

Le Impostazioni espongono soltanto **Groq, Google Gemini, OpenRouter, NVIDIA NIM, SambaNova Cloud, Alibaba Cloud Model Studio, OpenAI, Pollinations, Hugging Face e Together AI**. Più chiavi possono essere salvate, ma il backend riceve un solo provider attivo alla volta e non applica fallback impliciti. Le chiavi sono gestite dal keyring nativo; la UI non deve salvarle nel repository, nei log o nei file di configurazione.

Il traffico verso il provider usa TLS/WSS. Questo protegge il trasporto, ma **non equivale a cifratura end-to-end contro il provider**: prompt e risposte sono elaborati nell’infrastruttura del provider autorizzato. Il consenso esplicito in Impostazioni resta quindi necessario.

| Capacità | Implementazione | Limite operativo |
|---|---|---|
| Provider cloud selezionabile | Allowlist frontend/nativa di 10 provider. | Compatibilità reale di ogni modello ed endpoint dipende dal provider e dalla sua chiave. |
| Chiavi dei provider | Keyring del sistema operativo. | La persistenza Windows deve essere provata su PC reale. |
| Provider attivo | Un profilo attivo esplicito, senza fallback automatico. | Un profilo incompleto blocca l’avvio della chat finché non viene configurato. |
| Trascrizione | Relay Groq Whisper con consenso dedicato. | Da provare con microfono, account e rete reali. |
| Live audio | Gemini Live audio-only con token effimero monouso e rifiuto delle tool call. | Da provare con key, microfono e output audio reali. |

### Browser, file, app e finestre

La funzione “controllare applicazioni” non significa controllo illimitato del PC. Il broker Windows è progettato per una UI Automation ristretta: richiede autorizzazione, token monouso, verifiche del processo/finestra/elemento e mantiene l’audit. Non espone `SendInput`, coordinate, mouse/tastiera globali o clipboard globale.

| Area | Ciò che il codice consente | Ciò che rimane bloccato intenzionalmente |
|---|---|---|
| Browser | Policy HTTPS pubblica, difese SSRF/IDN/URL con credenziali, blocco di azioni sensibili. | Login, OTP, password, recupero account, banca, pagamenti, acquisti, pubblicazione e cancellazioni. |
| File locali | Cartelle preventivamente approvate, proposta–approvazione–claim monouso–audit. | Percorsi protetti Git/VCS, configurazioni di agenti/IDE e percorsi esterni alle cartelle autorizzate. |
| App/finestre Windows | Broker UIA ristretto con inspect/read/invoke/set text entro gli elementi autorizzati. | Mouse/tastiera/clipboard globali, coordinate, shell generica, script host, installer ed elevazione privilegi. |
| Android ADB | Diagnostica read-only e approval-gated con allowlist. | Shell ADB arbitraria, input, apertura app, installazioni, file transfer e root. |

### Pattern esterni adottati con selezione

Sono stati riutilizzati concetti e non framework completi: self-test non distruttivo, policy browser, difese Gemini Live, protezione dell’area di lavoro e deduplicazione di retry/approvazioni. Non sono stati incorporati server o framework interi quali Open Interpreter, OpenHands, Agent-S, CUA, browser-use, nut.js, pure-python-adb o mem0, perché aggiungerebbero superfici di attacco, duplicazioni oppure violerebbero il profilo cloud-only/approval-gated.

## Risultati dei test

L’audit ha eseguito una suite completa Python e test mirati sul codice 1.0.6. Il risultato della suite completa è **non verde**: i fallimenti non devono essere nascosti né attribuiti alla build Windows come se fossero risolti.

| Comando o ambito | Esito | Interpretazione |
|---|---:|---|
| `uv run pytest -q` | 7.965 passati, 115 falliti, 71 saltati, 204 warning; 10 min 26 s | Suite upstream completa non verde. |
| Deployment CSP e prompt desktop | 14 passati | La CSP locale, l’auto-update disabilitato e la policy prompt obbligatoria sono coperti. |
| Sicurezza browser/local/approval/ADB | 56 passati | I confini Python rilevanti sono coperti in isolamento. |
| Frontend sorgente | 13 file, 59 test passati | Include la nuova regressione sull’endpoint desktop. |
| Build frontend | Passata | TypeScript e Vite generano il bundle. |
| Rust/Tauri Linux | 36 passati | Logica nativa testabile su Linux; non sostituisce la compilazione/uso Windows. |
| Build e bundle Windows 1.0.6 | Da eseguire | Deve avvenire nella CI Windows prima di qualsiasi installazione. |

I 115 fallimenti residui si concentrano in test upstream e di ambiente: integrazioni live senza credenziali/provider, motori locali/mining deliberatamente disabilitati, browser AX tree senza Playwright installato, skill/inference engine assenti, e alcune aspettative generiche di tool HTTP/Git/MCP/storage non coinvolte nel perimetro desktop corretto. Non sono stati “forzati” verdi perché correggerli richiederebbe cambiare funzionalità upstream o aggiungere dipendenze/servizi non necessari alla release cloud-only.

## Limiti e smoke test Windows obbligatori

La compilazione Linux e le unit test non provano automaticamente API Windows, WebView2, Credential Manager, microfono, device Android o UI Automation. Prima di definire la release utilizzabile occorre eseguire su Windows una prova manuale limitata e reversibile.

1. Verificare l’hash dell’installer 1.0.6 pubblicato dalla CI e installarlo solo dopo il confronto.
2. Aprire l’app senza inserire chiavi e navigare nell’ordine **Chat → Dashboard → Logs → Settings → Chat**. Nessuna pagina deve tornare autonomamente a Settings.
3. In Settings verificare la lista dei dieci provider e l’assenza dei campi legacy API URL/API key nella build desktop.
4. Salvare una sola chiave di test non sensibile, selezionare provider e modello, riavviare e verificare che la presenza della chiave sia conservata senza essere mostrata in chiaro.
5. Usare **Verifica Jarvis** senza microfono, device o azioni locali; il self-test deve restare privo di effetti collaterali.
6. Solo dopo, con un programma innocuo e una cartella di prova, verificare un’azione UIA e una proposta file con approvazione. Gemini Live e ADB vanno testati separatamente con consenso esplicito.

L’installer Windows resterà non firmato finché non viene fornito un certificato di firma codice. Dopo aver verificato l’hash, Windows potrebbe mostrare SmartScreen; non bisogna disattivare Defender o le protezioni del sistema in modo globale.

## Decisione di rilascio

Il candidato 1.0.6 è adatto a una **build privata Windows per test manuale**, non a una dichiarazione di prodotto definitivo. La build può essere pubblicata solo come artefatto privato, senza tag/release pubblico e senza trasformare i fallimenti upstream residui in una falsa certificazione di qualità.
