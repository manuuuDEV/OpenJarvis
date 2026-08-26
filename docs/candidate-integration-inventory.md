# Inventario candidati dalla mappa utente

Questo inventario separa **repository**, **librerie/API** e **pattern concettuali**. Non costituisce ancora una decisione di integrazione né afferma che le funzioni elencate siano già presenti nella working copy OpenJarvis.

| ID | Candidato | Categoria | Capacità indicata dalla mappa | Stato da verificare |
|---|---|---|---|---|
| C01 | [browser-use](https://github.com/browser-use/browser-use) | Repository/libreria Python | Automazione browser guidata da LLM con pausa umana | Compatibilità e ridondanza rispetto alla sicurezza OpenJarvis |
| C02 | [Playwright](https://github.com/microsoft/playwright) | Libreria | Automazione DOM del browser | Necessità reale e modello di autorizzazione |
| C03 | [nut.js](https://github.com/nut-tree/nut-js) | Libreria Node | Mouse e tastiera globali Windows | Incompatibilità potenziale con il broker UI Automation restrittivo |
| C04 | [pywinauto](https://github.com/pywinauto/pywinauto) | Repository/riferimento | Automazione UI Windows | Pattern UI Automation e alternative native Rust |
| C05 | [UI-TARS](https://github.com/bytedance/UI-TARS) | Repository | Computer use visuale | Beneficio e rischio di screenshot/coordinate |
| C06 | [CUA](https://github.com/trycua/cua) | Repository | Astrazione driver di input | Eventuale pattern di interfaccia, non dipendenza monolitica |
| C07 | [Agent-S](https://github.com/simular-ai/Agent-S) | Repository | Separazione planning e grounding | Utilità per piani controllati |
| C08 | [os-ai-computer-use](https://github.com/777genius/os-ai-computer-use) | Repository | Adapter OpenAI-compatible multimodale | Compatibilità provider e sicurezza immagine |
| C09 | [NonMouse](https://github.com/takeyamayuki/NonMouse) | Repository | Gesti mano e calibrazione | Utilità in un’app desktop Windows |
| C10 | Virtual-Mouse | Repository/riferimento | Interaction box per cursore da webcam | Identità upstream e compatibilità da verificare |
| C11 | [MediaPipe](https://github.com/google-ai-edge/mediapipe) | Libreria | Landmark mano da webcam | Peso, privacy e integrazione opzionale |
| C12 | OneEuroFilter | Libreria/pattern | Smoothing a bassa latenza | Necessità solo se si adotta hand tracking |
| C13 | Groq Whisper API | API cloud | Trascrizione voce-testo | Già presente come relay isolato da verificare rispetto alla mappa |
| C14 | [Edge-TTS](https://github.com/rany2/edge-tts) | Libreria Python | Sintesi vocale | Privacy, affidabilità e valore per turn-based voice |
| C15 | Gemini Live API | API cloud WebSocket | Conversazione audio bidirezionale | Sicurezza chiavi/token e stato preview |
| C16 | [mem0](https://github.com/mem0ai/mem0) | Repository | Memoria a lungo termine/RAG | Sovrapposizione con memoria esistente e privacy |
| C17 | Google Safe Browsing API | API cloud | Verifica URL per minacce | Stato API, privacy e fail-closed |
| C18 | VirusTotal API | API cloud | Reputazione file/URL | Privacy dei campioni e limiti di servizio |
| C19 | [pure-python-adb](https://github.com/Swind/pure-python-adb) | Libreria Python | Bridge ADB Android | Confronto con broker ADB Rust ristretto |
| C20 | [JARVIS-OS-V.2](https://github.com/MAL19INDUSTRIES/JARVIS-OS-V.2) | Repository | HUD, telemetria e pattern live | Estrarre solo pattern utili, non UI/framework |
| C21 | GLM-5.2 via NVIDIA NIM | Modello/API cloud | Consulto specializzato in background | Verificare modello, endpoint, costi e isolamento provider |
| C22 | Managed Agents / Claude Fable | Pattern | Sub-agenti gerarchici | Confronto con agent manager esistente |
| C23 | Pollinations | API cloud | Generazione immagini | Separare capacità immagine e testo, stato prezzi/endpoint |
| C24 | Client OpenAI-compatible / Anthropic | Librerie/provider pattern | Registro universale provider | Confronto con il router cloud già implementato e allowlist UI |

## Criteri obbligatori di selezione

Un candidato sarà proposto per integrazione solo se soddisfa simultaneamente i seguenti criteri: produce un vantaggio misurabile rispetto al codice esistente; si adatta senza introdurre un server parallelo o una dipendenza monolitica; mantiene il profilo cloud-only, la gestione nativa delle chiavi, le approvazioni locali e la redazione; dispone di licenza e documentazione compatibili; non richiede controlli globali non verificabili o l’esposizione di dati sensibili.

I candidati che duplicano una capacità già funzionante, richiedono credenziali nel renderer, introducono controlli indiscriminati o necessitano di un’architettura Electron separata saranno invece classificati come **non adottare** oppure **estrarre soltanto il pattern**.


## Ricerca upstream — gruppo browser e automazione desktop

| Candidato | Fonte verificata | Dato rilevante | Valutazione preliminare |
|---|---|---|---|
| browser-use | [GitHub][1] | MIT; framework Python che può controllare browser, compilare form e riusare profili/autenticazioni. | **Non adottare come dipendenza ora**: sovrappone un canale browser ad alto privilegio e la gestione di sessioni/autenticazione confligge con il requisito di non automatizzare login o invii. Il pattern di approvazione umana può essere studiato. |
| Playwright | [GitHub][2] | Apache-2.0; API per automazione e testing browser, include accessibilità strutturata tramite MCP/CLI. | **Valutare come futura integrazione limitata**: utile per un browser isolato, read-only e approval-gated. Non adottare finché non esista una policy di target, login e invio equivalente al broker desktop. |
| nut.js | [GitHub][3] | Automazione UI cross-platform con mouse, tastiera, clipboard, OCR e image matching; pacchetti precompilati legati a piani a pagamento. | **Non adottare**: reintrodurrebbe controllo globale a coordinate, tastiera/mouse e clipboard, escluso dal profilo sicuro. |
| pywinauto | [GitHub][4] | BSD-3; usa Win32/UI Automation ma include anche emulazione mouse/tastiera e necessita di stack Python/Windows aggiuntivo. | **Non adottare come libreria**: il broker Rust usa già UI Automation nativa con limiti più stretti. Tenere solo come riferimento per test e compatibilità. |

[1]: https://github.com/browser-use/browser-use "browser-use/browser-use"
[2]: https://github.com/microsoft/playwright "microsoft/playwright"
[3]: https://github.com/nut-tree/nut.js "nut-tree/nut.js"
[4]: https://github.com/pywinauto/pywinauto "pywinauto/pywinauto"


## Ricerca upstream — gruppo computer use visuale

| Candidato | Fonte verificata | Dato rilevante | Valutazione preliminare |
|---|---|---|---|
| UI-TARS | [GitHub][5] | Apache-2.0; agente VLM per grounding con coordinate e output di mouse/tastiera; il progetto stesso segnala rischio di uso improprio e costo computazionale rilevante. | **Estrarre solo pattern**: separazione tra pianificazione, grounding e verifica. Non ospitare modelli locali e non adottare coordinate/input globale nella release sicura. |
| CUA | [GitHub][6] | MIT; driver desktop in background, VM/sandbox, input globale e gestione di fleet/ambienti. | **Non adottare**: è un’infrastruttura separata e introduce capacità di input/shell/sandbox fuori dalla superficie controllata. Il concetto di driver separato è già applicato nel broker nativo. |
| Agent-S | [GitHub][7] | Apache-2.0; framework Python per computer use con screenshot, PyAutoGUI, grounding model e opzione di esecuzione Python/Bash locale, che la documentazione qualifica come rischiosa. | **Estrarre solo pattern**: planning/reflection/contesto minimo possono ispirare il gestore agenti; non installare il framework né le capacità PyAutoGUI o shell. |
| os-ai-computer-use | [GitHub][8] | Apache-2.0; desktop agent con mouse/tastiera globali, screenshot, clipboard, typing, drag e backend/GUI distinti. | **Non adottare**: duplica un’architettura completa e reintroduce precisamente input e cattura schermo che il profilo corrente limita. |

[5]: https://github.com/bytedance/UI-TARS "bytedance/UI-TARS"
[6]: https://github.com/trycua/cua "trycua/cua"
[7]: https://github.com/simular-ai/Agent-S "simular-ai/Agent-S"
[8]: https://github.com/777genius/os-ai-computer-use "777genius/os-ai-computer-use"


## Ricerca upstream — gruppo hand tracking e webcam

| Candidato | Fonte verificata | Dato rilevante | Valutazione preliminare |
|---|---|---|---|
| NonMouse | [GitHub][9] | Apache-2.0; mouse gestuale webcam con hotkey globale, cursor movement, click, right click e scroll. | **Non adottare come funzionalità corrente**: utile solo come riferimento UX/calibrazione; reintroduce input globale vietato nel profilo. |
| Virtual-Mouse | [GitHub][10] | MIT; esempio compatto MediaPipe/OpenCV con cursore, click, drag e scroll tramite gesti. | **Non adottare**: è un prototipo di input globale; il concetto di interaction box può essere rivalutato soltanto per una modalità accessibilità esplicita e disconnessa dall’agente cloud. |
| MediaPipe | [GitHub][11] | Apache-2.0; task cross-platform on-device. La documentazione segnala che l’input viene elaborato sul device, ma che possono essere trasmesse metriche d’uso, per le quali serve consenso informato. | **Candidato condizionale**: utile se l’utente richiede davvero hand tracking; va isolato in processo opzionale, con consenso webcam e policy esplicita sulle metriche. |
| One Euro Filter | [GitHub][12] | MIT; semplice implementazione/pseudocodice dell’algoritmo di smoothing. | **Estrarre pattern, non dipendenza**: se si implementa hand tracking, il filtro può essere riscritto nel modulo nativo o Python già previsto. |

[9]: https://github.com/takeyamayuki/NonMouse "takeyamayuki/NonMouse"
[10]: https://github.com/whitehatboy005/Virtual-Mouse "whitehatboy005/Virtual-Mouse"
[11]: https://github.com/google-ai-edge/mediapipe "google-ai-edge/mediapipe"
[12]: https://github.com/jaantollander/OneEuroFilter "jaantollander/OneEuroFilter"


## Ricerca upstream — gruppo voce, memoria, ADB e telemetria

| Candidato | Fonte verificata | Dato rilevante | Valutazione preliminare |
|---|---|---|---|
| Edge-TTS | [GitHub][13] | Il modulo Python usa il servizio TTS online di Microsoft senza API key; non è sintesi locale. | **Candidato da valutare**: può migliorare la lettura delle risposte nella modalità a turni, ma richiede consenso distinto perché il testo è inviato a un servizio esterno. Non va etichettato come locale o privato end-to-end. |
| mem0 | [GitHub][14] | Apache-2.0 ma grande stack con SDK, server self-hosted/cloud e dipendenza da LLM/embedding; il README distingue funzioni proprietarie della piattaforma gestita. | **Non adottare come dipendenza**: la memoria OpenJarvis esiste già; la priorità è migliorare privacy, consenso e retrieval locale esistente, non introdurre server, Docker o nuove chiavi. |
| pure-python-adb | [GitHub][15] | MIT; espone shell, installazione/disinstallazione, push/pull, forwarding, screenshot e connect remoto. | **Non adottare**: il broker ADB Rust ristretto esiste appositamente per impedire queste capacità generiche. |
| JARVIS-OS-V.2 | [GitHub][16] | MIT ma architettura Python/PyQt, FastAPI/Next.js, Docker e web stack separati; include Gemini Live con chiave in `.env`. | **Estrarre solo pattern**: il self-test side-effect-safe e i report QA possono essere utili; non adottare UI, stack Docker/web o gestione segreti `.env`. |

[13]: https://github.com/rany2/edge-tts "rany2/edge-tts"
[14]: https://github.com/mem0ai/mem0 "mem0ai/mem0"
[15]: https://github.com/Swind/pure-python-adb "Swind/pure-python-adb"
[16]: https://github.com/MAL19INDUSTRIES/JARVIS-OS-V.2 "MAL19INDUSTRIES/JARVIS-OS-V.2"


## Ricerca upstream — gruppo servizi live, sicurezza URL e immagini

| Candidato | Fonte verificata | Dato rilevante | Valutazione preliminare |
|---|---|---|---|
| Gemini Live API | [Google AI][17] | Preview; WebSocket stateful, audio PCM continuo; Google raccomanda token effimeri per client-to-server in produzione. | **Priorità alta, ma solo come progetto dedicato**: il bridge deve restare nativo/locale o usare token effimeri; non esporre chiavi Gemini nel WebView. |
| Google Safe Browsing v4 | [Google Developers][18] | V4 è deprecata; Lookup invia URL in chiaro, Update è più privato ma richiede database e aggiornamenti. | **Non integrare V4**: valutare separatamente la migrazione a Web Risk o un’altra fonte, con valutazione privacy e spazio disco. |
| VirusTotal API v3 | [VirusTotal][19] | API REST per report/scansioni di file, URL, domini e IP; upload file è una funzione esplicita. | **Candidato limitato**: usare in futuro solo lookup hash/URL con consenso esplicito. Non caricare file dell’utente o inviare URL sensibili per default. |
| Pollinations | [GitHub][20] | MIT; `gen.pollinations.ai` è endpoint unificato con chiavi e crediti; il progetto avverte di non esporre chiavi segrete lato client e supporta più media. | **Già configurato come provider candidato**: mantenere endpoint canonico, chiavi in keyring e consenso per il contenuto inviato. Non dichiarare testo o immagini gratuiti o privati senza controllo attuale del piano. |

[17]: https://ai.google.dev/gemini-api/docs/live "Google AI — Gemini Live API overview"
[18]: https://developers.google.com/safe-browsing/v4 "Google Developers — Safe Browsing APIs v4"
[19]: https://docs.virustotal.com/reference/overview "VirusTotal — API v3 Overview"
[20]: https://github.com/pollinations/pollinations "pollinations/pollinations"


## Ricerca upstream — gruppo consulto specialistico e orchestrazione

La ricerca ufficiale NVIDIA ha identificato la documentazione NIM LLM e la pagina del modello GLM-5.2. NVIDIA descrive NIM LLM come API compatibile OpenAI; la pagina Build indica che l’accesso ai modelli richiede account e chiave API. Il risultato sul modello GLM-5.2 lo presenta per workflow agentici, coding e ragionamento a lungo orizzonte. Questi dati non dimostrano né disponibilità gratuita permanente né vantaggio nel caso d’uso OpenJarvis.

| Candidato | Fonte verificata | Dato rilevante | Valutazione preliminare |
|---|---|---|---|
| GLM-5.2 via NVIDIA NIM | [NVIDIA NIM API][21] e [NVIDIA Build][22] | NIM espone API compatibile OpenAI; GLM-5.2 è catalogato come modello per workflow agentici/coding/ragionamento, con accesso soggetto a account/chiave. | **Candidato per esperimento opt-in**, non per consulto automatico: deve restare un provider scelto esplicitamente nell’UI esistente, senza doppie chiavi backend o sub-agenti nascosti. |
| Managed agents / Claude Fable | Pattern, non repository o SDK indicato nella mappa | Il valore da valutare è la decomposizione planner/reviewer e la minima divulgazione del contesto. | **Estrarre pattern**: confrontarlo con `agent_manager_routes.py` e i managed agent esistenti prima di qualunque modifica. |
| Router OpenAI-compatible/Anthropic | Pattern di client provider | L’implementazione esistente ha già router cloud, allowlist e selezione provider esplicita. | **Non duplicare**: valutare solo lacune concrete quali metadati capacità, modelli e policy per sub-agenti. |

[21]: https://docs.nvidia.com/nim/large-language-models/latest/api-reference.html "NVIDIA NIM — LLM API Reference"
[22]: https://build.nvidia.com/z-ai/glm-5.2 "NVIDIA Build — z-ai/glm-5.2"


## Confronto con la working copy OpenJarvis

| Capacità candidata | Evidenza presente nel repository | Esito del confronto |
|---|---|---|
| Playwright/browser automation | `pyproject.toml` dichiara l’extra `browser = ["playwright>=1.40"]`; `src/openjarvis/tools/browser.py` fornisce navigazione, click, typing, screenshot ed estrazione con controllo SSRF; `browser_axtree.py` espone la struttura di accessibilità. | **Già implementato**: non installare browser-use/Playwright di nuovo. L’unico miglioramento possibile è rendere il browser desktop soggetto a policy/approvazioni più esplicite. |
| Memoria a lungo termine | `memory/service.py` estrae fatti in background; `memory/store.py` conserva provenienza, livelli di fiducia, quarantena, deduplica e filtri fail-closed per il recall. | **Già implementato**: non aggiungere mem0. Si possono valutare miglioramenti mirati di retrieval o consenso memoria, se emerge un caso d’uso verificato. |
| Agenti gestiti/orchestrazione | `agents/manager.py` usa persistenza SQLite per agenti, task, checkpoint, code messaggi e log; `agents/hybrid/conductor.py` segue già un planner statico a DAG con piano strutturato. | **Già implementato**: usare Agent-S/Managed Agents solo come riferimento per metriche e controlli, non come framework. |
| Sintesi vocale | `tools/text_to_speech.py` registra backend TTS e salva l’audio generato; i backend correnti dichiarati includono Cartesia, Kokoro e OpenAI TTS. | **Parzialmente implementato**: Edge-TTS può essere valutato come backend opzionale, con consenso cloud e senza chiamata implicita. |
| Desktop/Android | I tool `controlled_desktop.py`, `controlled_local.py` e `controlled_android_adb.py`, con broker Rust nativi, applicano approvazioni, allowlist, token e audit. | **Non sostituire** con nut.js, CUA, Agent-S o pure-python-adb; eventuali pattern devono rafforzare il broker, non aggirarlo. |
| Provider cloud | Il router e le Impostazioni già applicano allowlist, chiave nel keyring e provider attivo esplicito. | **Non duplicare** il registro provider universale; GLM/NIM resta solo una scelta utente entro il provider NVIDIA già ammesso. |

Questo confronto riduce le integrazioni da valutare a quattro piste non ridondanti: **Gemini Live sicuro**, **Edge‑TTS consensuale**, **controllo URL/malware privacy-first aggiornato**, e **self-test side-effect-safe per la release Windows**. Hand tracking resta una capacità accessibilità separata, da proporre solo se l’utente la desidera esplicitamente.
