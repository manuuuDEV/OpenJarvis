# Roadmap di integrazione selettiva dai repository candidati

## Decisione architetturale

La valutazione non raccomanda di importare repository completi. OpenJarvis possiede già un runtime Tauri/Rust + Python, broker nativi, provider cloud espliciti e tool controllati. Aggiungere framework completi di computer-use, ADB o memoria introdurrebbe dipendenze ridondanti, superficie d’attacco e percorsi paralleli non soggetti alle stesse approvazioni.

> La regola applicata è: **integrare soltanto una capacità assente o chiaramente migliore, come pattern nativo e con i confini di sicurezza già esistenti**.

## Matrice decisionale

| Priorità | Candidato/pattern | Decisione | Motivazione |
|---|---|---|---|
| 1 | Self-test side-effect-safe di JARVIS-OS-V.2 | **Implementato come pattern nativo** | `Verifica Jarvis` controlla localmente contratti, health e configurazioni senza avviare broker, browser, ADB, registrazione o token. Gli elementi Windows/device/live restano esplicitamente `Prova reale richiesta`. [1] |
| 2 | Gemini Live API | **Implementato come canale audio nativo + WSS diretto** | Il runtime crea token effimeri monouso vincolati al modello e ad `AUDIO`; la WebView riceve soltanto il token temporaneo in memoria. Non dichiara tools, camera, schermo o Search e chiude la sessione se arriva un tool call. Richiede ancora test reale. [2] |
| 3 | Playwright | **Hardening implementato, nessuna seconda libreria** | Nel profilo desktop, navigazione solo HTTPS, preflight locale su URL strutturalmente sensibili e blocco tecnico di login, identità, credenziali, pagamenti, invii, pubblicazioni ed eliminazioni. [3] |
| 4 | Agent-S, UI-TARS, CUA | **Estrarre solo planning/reflection e verifica post-azione** | Il loro valore è concettuale; le implementazioni assumono screenshot, coordinate, input globale, modelli locali/grounding e talvolta shell locale, incompatibili con il broker sicuro. [4] [5] [6] |
| 5 | Edge-TTS | **Non integrare finché non sia verificata licenza e politica dati** | È un TTS online esterno; non migliora la privacy e l’app ha già backend TTS. La licenza/distribuzione e il consenso per inviare testo richiedono una verifica separata. [7] |
| 6 | Safe Browsing v4 | **Non adottare** | Google dichiara v4 deprecata. Il lookup invia l’URL in chiaro, mentre il percorso più privato richiede database e aggiornamenti locali. [8] |
| 7 | VirusTotal | **Non integrato per scelta privacy-first** | È stato implementato invece un preflight locale che non invia URL a terzi. Un lookup di reputazione richiede in futuro consenso esplicito e una chiave/account separati; upload file resta escluso. [9] |
| 8 | MediaPipe + One Euro Filter | **Backlog di accessibilità opzionale** | MediaPipe può elaborare input sul device, ma richiede consenso informato per metriche; i progetti candidati controllano globalmente il cursore, funzione da tenere separata dall’agente cloud. [10] [11] [12] |
| 9 | pure-python-adb | **Non adottare** | Espone shell, push/pull, installazione, screenshot e rete ADB; il broker Rust esistente li blocca intenzionalmente. [13] |
| 10 | mem0 | **Non adottare come dipendenza** | OpenJarvis ha già memoria persistente con provenienza, quarantena e recall controllato; mem0 aggiunge SDK/server/cloud e dipendenze LLM/embedding ulteriori. [14] |
| 11 | browser-use, nut.js, os-ai-computer-use | **Non adottare** | Duplicano il browser o riaprono input globale, sessioni/account e automazione a coordinate che il profilo sicuro deve evitare. [15] [16] [17] |
| 12 | GLM-5.2/NVIDIA NIM | **Mantenere come provider scelto dall’utente** | NVIDIA NIM è compatibile OpenAI e GLM può essere un modello selezionato dal provider NVIDIA; non deve diventare un sub-agente automatico o ricevere una seconda chiave nascosta. [18] |
| 13 | Pollinations | **Mantenere l’integrazione attuale, con policy capacità aggiornate** | L’endpoint è multipurpose e basato su chiavi/crediti; occorre distinguere testo, immagini e altri media, senza affermare gratuità o privacy universale. [19] |

## Pacchetti di lavoro proposti

| Pacchetto | Contenuto nativo OpenJarvis | Impatto sicurezza | Dipendenze nuove | Esito raccomandato |
|---|---|---|---|---|
| A — Self-test release | Audit locale side-effect-safe, report JSON redatto, indicatori `LIVE CHECK REQUIRED` per Windows, ADB e voce. | Riduce i falsi positivi di prontezza. | Nessuna. | **Implementato; test Linux riusciti.** |
| B — Browser controllato | Policy per navigazione HTTPS, preflight URL e blocco azioni sensibili. | Riduce automazione browser non intenzionale. | Nessuna nuova libreria. | **Implementato; test policy riusciti.** |
| C — Gemini Live nativo | Consenso, token effimero nativo, mic PCM, WSS diretto, stop manuale e chiusura su tool call. | Alto impatto: richiede test e threat model dedicati. | Nessuna nuova crate; WebView usa WebSocket/AudioContext standard. | **Implementato; smoke test reale ancora obbligatorio.** |
| D — URL reputation opt-in | Preflight locale di URL, senza lookup esterno. | Evita invio involontario di URL riservati. | Nessuna. | **Implementato in forma locale; reputazione di terzi non adottata.** |
| E — Hand tracking accessibilità | Processo locale opt-in senza accesso cloud, kill switch e indicatore webcam. | Alto: input globale e privacy webcam. | MediaPipe/modello distribuito. | **Backlog; non integrare ora.** |
| F — Integrità workspace e retry | Esclusione preventiva di metadati VCS/agente/IDE dalle root controllate; una proposta pendente identica viene riusata. | Riduce l’esposizione involontaria di configurazioni di sviluppo e lo spam/rischio operativo dei retry senza bypassare TTL, approvazione o claim monouso. | Nessuna. | **Implementato come pattern nativo; 18 test diretti e 38 test di regressione sicurezza riusciti.** |

## Elementi esclusi esplicitamente

Nessun pacchetto sopra abilita: input mouse/tastiera globale, coordinate arbitrarie, clipboard, screenshot cloud, shell o script generici, root, ADB generico, file transfer, installazione app, login, OTP, acquisti, pagamenti, invii o automazione di account di terzi. Le funzioni eventualmente aggiunte devono continuare a passare dai broker locali, dalle approvazioni monouso, dall’audit e dalla redazione già implementati.

## Criterio applicato durante l’implementazione

I pacchetti A, B, C, D e F sono stati realizzati soltanto dopo aver verificato che non richiedessero privilegi aggiuntivi, server paralleli o nuove dipendenze monolitiche. Il pacchetto C conserva consenso informato e richiede comunque uno smoke test reale con account, microfono, rete e Windows prima di poter essere dichiarato operativo. Il pacchetto E resta escluso dalla release candidata poiché l’input globale e la webcam necessitano di una decisione di prodotto distinta.

## Riferimenti

[1]: https://github.com/MAL19INDUSTRIES/JARVIS-OS-V.2 "JARVIS-OS-V.2"
[2]: https://ai.google.dev/gemini-api/docs/live "Gemini Live API overview"
[3]: https://github.com/microsoft/playwright "Playwright"
[4]: https://github.com/bytedance/UI-TARS "UI-TARS"
[5]: https://github.com/trycua/cua "CUA"
[6]: https://github.com/simular-ai/Agent-S "Agent-S"
[7]: https://github.com/rany2/edge-tts "edge-tts"
[8]: https://developers.google.com/safe-browsing/v4 "Google Safe Browsing v4"
[9]: https://docs.virustotal.com/reference/overview "VirusTotal API v3"
[10]: https://github.com/google-ai-edge/mediapipe "MediaPipe"
[11]: https://github.com/takeyamayuki/NonMouse "NonMouse"
[12]: https://github.com/jaantollander/OneEuroFilter "One Euro Filter"
[13]: https://github.com/Swind/pure-python-adb "pure-python-adb"
[14]: https://github.com/mem0ai/mem0 "mem0"
[15]: https://github.com/browser-use/browser-use "browser-use"
[16]: https://github.com/nut-tree/nut.js "nut.js"
[17]: https://github.com/777genius/os-ai-computer-use "os-ai-computer-use"
[18]: https://docs.nvidia.com/nim/large-language-models/latest/api-reference.html "NVIDIA NIM LLM API"
[19]: https://github.com/pollinations/pollinations "Pollinations"
