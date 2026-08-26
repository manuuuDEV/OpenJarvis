# Confronto iniziale: OpenJarvis e alternative per un assistente desktop stile Jarvis

## Criteri non negoziabili

L’utente richiede un’app Windows installabile, cloud-only per l’intelligenza, spazio disco contenuto, provider scelti esplicitamente, chiavi nel portachiavi OS, voce e strumenti controllati. Le alternative sono quindi valutate su privacy effettiva, sicurezza tecnica delle azioni, potenza agentica, maturità, licenza, costo di migrazione e compatibilità con tali vincoli. Non vengono considerate equivalenti le soluzioni che ottengono più privacy soltanto scaricando modelli locali o che ottengono più potenza dando all’agente shell/filesystem/browser senza confini.

## Fonti upstream esaminate

| Progetto | Fonte | Riscontro verificato | Prima conclusione |
|---|---|---|---|
| OpenJarvis | [Repository upstream][1] | Apache-2.0; framework local-first con backend Python, desktop Tauri, agenti, memoria, skills ed eval. L’upstream installa/usa Ollama e modelli locali per default, comportamento che la working copy ha intenzionalmente sostituito con cloud-only. | **Base corretta per la working copy**, perché il runtime e l’installer Windows sono già stati adattati; non è nativamente cloud-only privacy-first. |
| Open Interpreter | [Repository upstream][2] | Apache-2.0; coding agent Rust/Codex-compatible con sandbox, provider/model switching, MCP/ACP e computer-use per browser/app native. | **Più forte per coding harness**, ma non è un assistente personale Windows completo e il suo computer-use/sandbox richiede una policy più ampia di quella accettata per OpenJarvis. |
| OpenHands Agent Canvas | [Repository upstream][3] | MIT; control center self-hosted per agenti e automazioni, backend multipli, Docker/VM/host. La documentazione avverte che il server senza sandbox ha accesso completo al filesystem e mostra automazioni schedulate/webhook. | **Potente per sviluppo software**, ma peggiore per la postura desktop personale: runtime più complesso, servizi continui e maggiore superficie/privilegio. |
| OpenVoiceOS | [Repository upstream][4] | Apache-2.0; piattaforma voce/skills orientata a smart speaker e device voice-centric; supporta persona/LLM tramite plugin. | **Buona alternativa per wake word e voice assistant**, non un sostituto per desktop agent, browser/ADB/UI Automation Windows. |
| Leon AI | [Repository upstream][5] | MIT; assistente personale con strumenti, contesto, memoria e agentic execution; il ramo 2.0 è Developer Preview e la documentazione nuova non è pronta. | **Concettualmente vicino a Jarvis**, ma non abbastanza stabile/documentato per giustificare una migrazione dalla working copy già estesa. |
| Home Assistant Assist | [Documentazione ufficiale][6] [7] | Può elaborare integralmente voce, STT, intent e TTS su hardware proprio; è specializzato in smart home e richiede una pipeline locale per tale massima privacy. | **Migliore scelta per privacy della sola voce/domotica**, ma incompatibile con il requisito cloud-only/no modelli locali e non è un desktop agent generale. |

> Nessun progetto risulta “migliore” in ogni dimensione. La privacy massima è ottenibile con pipeline voce/modelli locali, mentre la potenza agentica generale spesso richiede strumenti locali molto più privilegiati. Sono compromessi opposti al profilo cloud-only con operazioni limitate scelto per OpenJarvis.

## Riferimenti

[1]: https://github.com/open-jarvis/OpenJarvis "open-jarvis/OpenJarvis"
[2]: https://github.com/OpenInterpreter/open-interpreter "Open Interpreter"
[3]: https://github.com/OpenHands/OpenHands "OpenHands Agent Canvas"
[4]: https://github.com/OpenVoiceOS/ovos-core "OVOS Core"
[5]: https://github.com/leon-ai/leon "Leon AI"
[6]: https://www.home-assistant.io/voice_control/ "Home Assistant Assist"
[7]: https://www.home-assistant.io/voice_control/voice_remote_local_assistant/ "Home Assistant — fully local voice assistant"


## Verifica dei progetti esplicitamente denominati “Jarvis”

La ricerca ha mostrato soprattutto progetti storici o dimostrativi. Non è emerso un repository “Jarvis” mantenuto che superi OpenJarvis nel compromesso richiesto tra desktop Windows, agenti moderni, privacy cloud-only controllata e confini tecnici delle azioni.

| Progetto | Fonte verificata | Riscontro | Decisione |
|---|---|---|---|
| sukeesh/Jarvis | [Repository][8] | MIT; si definisce esplicitamente “Personal Non-AI Assistant”; CLI con plugin e task predeterminati. | Non comparabile per potenza LLM/agentica; non sostituisce OpenJarvis. |
| Jarvis Desktop Voice Assistant | [Repository][9] | MIT; assistente Python semplice, con 41 commit, azioni di sistema/web/screenshot e logica predefinita. | Non comparabile per maturità, sicurezza e controlli. L’accesso a funzioni di sistema è molto più libero del profilo richiesto. |

I repository etichettati “Jarvis” sono spesso prototipi a comandi predefiniti. Il nome non garantisce né sicurezza né capacità: il confronto va condotto sull’architettura, non sul branding.

[8]: https://github.com/sukeesh/Jarvis "sukeesh/Jarvis"
[9]: https://github.com/kishanrajput23/Jarvis-Desktop-Voice-Assistant "Jarvis Desktop Voice Assistant"


## Confronto con la working copy Windows cloud-only

La working copy non è l’upstream OpenJarvis immutato. Essa ha già sostituito il preset local-first con provider cloud espliciti, chiavi in keyring, un singolo provider attivo passato al backend, trascrizione Groq separata, broker UI/ADB allowlisted, approvazioni monouso, redazione e policy browser. Perciò cambiare base ora non sarebbe un semplice download: richiederebbe riprogettare e ritestare tutti quei confini.

| Criterio | Working copy OpenJarvis | Open Interpreter | OpenHands | Leon 2.0 | OVOS / Home Assistant Assist |
|---|---|---|---|---|---|
| Assistente personale desktop Windows | **Sì, già adattato** | Prevalentemente coding/TUI | Developer control center | Obiettivo dichiarato, ma preview | Voice/smart-home, non desktop generale |
| Cloud-only con provider espliciti | **Sì, implementato nella working copy** | Possibile ma non è il suo confine di sicurezza | Possibile, ma multi-backend/servizi persistenti | Supporta locale/remoto, integrazione da valutare | Assist cloud possibile, ma il vantaggio privacy deriva dal locale |
| Privacy massima senza cloud | No: scelta dell’utente è cloud-only | Variabile | Variabile | Variabile | **Sì per la voce locale**, ma richiede modelli/pipeline locali |
| Azioni PC limitate con gate tecnico | **Sì: broker UI, browser, file e ADB confinati** | Più flessibile, quindi più superficie | Può avere accesso pieno host senza sandbox | Da verificare nel preview | Non è progettato per UI Windows generale |
| Potenza per coding autonomo | Buona ma volutamente limitata | **Più alta** | **Più alta** | In evoluzione | Bassa/non pertinente |
| Voce / wake word | Groq a turni + Gemini Live implementato, non ancora smoke-testato | Non obiettivo principale | Non obiettivo principale | In evoluzione | **Più maturo per voice-first**, spesso con componenti locali |
| Costo di migrazione oggi | Nessuno | Alto: sostituzione runtime e policy | Molto alto: stack/servizi/sandbox diversi | Alto: preview e riscrittura | Alto e cambio di prodotto (smart home/voice) |
| Giudizio per i requisiti attuali | **Scelta raccomandata** | Da prendere come riferimento per il coding, non come sostituto | Da prendere come riferimento per sandbox, non come sostituto | Da monitorare | Da usare solo come integrazione domotica/voce locale separata |

### Esito tecnico

OpenJarvis era una scelta iniziale ragionevole per il tipo di prodotto richiesto: ha già desktop Tauri, backend Python, agenti, memoria, skills, test e un progetto Windows. Tuttavia, **l’upstream non è “più privato” di default per il tuo profilo**: è local-first e normalmente installa Ollama/modelli, mentre la privacy della working copy deriva dalle modifiche fatte per cloud-only e dai vincoli tecnici.

Non esiste oggi un’alternativa che sia contemporaneamente più potente di Open Interpreter/OpenHands, più privata di una pipeline Home Assistant interamente locale, e più sicura della working copy senza compromessi. La soluzione corretta non è migrare: è mantenere OpenJarvis come base, completare build e verifiche Windows, e adottare in modo selettivo pattern esterni (sandbox di coding, discipline di test e capacità voice) soltanto quando non indeboliscono il perimetro.

Una futura integrazione con Home Assistant può essere valutata come **connettore di domotica autorizzato e separato**, senza trasformare OpenJarvis in Home Assistant o installare modelli locali. Per un ambiente di sviluppo isolato, Open Interpreter/OpenHands possono essere usati fuori dall’app, in una VM/sandbox dedicata, ma non dovrebbero sostituire il broker Windows della release personale.


## Pattern candidati verificati per integrazione selettiva

| Origine | Pattern | Stato nella working copy | Decisione |
|---|---|---|---|
| Open Interpreter, Apache-2.0 | Modalità read-only/workspace-write, approvazione separata e percorsi protetti come `.git` e configurazioni agente. [10] | OpenJarvis già richiede approvazione, limita il workspace e blocca script/eseguibili, ma non proteggeva esplicitamente tutti i metadati Git/configurazioni agente dentro una root autorizzata. | **Adattare subito**: negare accesso attraverso i tool controllati a directory VCS e configurazioni agentiche; nessuna copia di codice o dipendenza. |
| Leon, MIT | Registro dello stato tool `installed`/`enabled`/`available`, argomenti validati, protezione duplicati e budget di iterazioni. [11] | OpenJarvis ha registri, self-test, limiti di piani e claim anti-replay, ma il ciclo sperimentale/benchmark è distinto dal desktop sicuro. | **Estrarre come roadmap**: rafforzare stati di disponibilità e osservazioni strutturate senza introdurre agenti continui/pulse o accesso shell. |
| OpenHands, MIT | Isolamento del lavoro in sandbox/VM e difesa in profondità per agenti con shell/filesystem. [12] | La working copy blocca deliberatamente shell e sandbox Docker, quindi non ha un codice agente da “sandboxare” nel prodotto personale. | **Non importare runtime**: tenere il pattern come opzione esterna per coding in VM, non come capacità nell’app. |

[10]: https://www.openinterpreter.com/docs/terminal/sandbox "Open Interpreter — Sandbox & Approvals"
[11]: https://github.com/leon-ai/leon/blob/develop/core/context/ARCHITECTURE.md "Leon 2.0 Architecture"
[12]: https://github.com/OpenHands/OpenHands/blob/main/docs/SELF_HOSTING.md "OpenHands Self-Hosting"


## Adattamento approvato: workspace protetto

L’integrazione selettiva non copia Open Interpreter. Aggiunge alla funzione `_resolve_workspace_path` una verifica deterministica del percorso relativo alla root autorizzata. Verranno bloccate le directory di controllo versione (`.git`, `.hg`, `.svn`) e le directory di configurazione/esecuzione agentica o IDE (`.agents`, `.claude`, `.codex`, `.cursor`, `.vscode`, `.idea`), oltre ai file di istruzioni/manifest per agenti (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `MCP.json`). Il blocco vale sia per lettura sia per modifica, perché contenuti e configurazioni possono includere token o istruzioni non affidabili.

Il tool di elenco filtrerà anche questi elementi quando elenca una root autorizzata, così non costituiranno un canale laterale. I file ordinari di progetto restano utilizzabili dopo l’approvazione esistente. I test copriranno lettura diretta negata, scrittura negata e invisibilità nell’elenco, insieme a una verifica positiva su file normali.
