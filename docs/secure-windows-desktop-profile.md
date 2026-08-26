# OpenJarvis Desktop Secure — Profilo Windows cloud-only

## Finalità e confine operativo

Questa distribuzione desktop usa un **solo provider cloud autorizzato** alla volta e non installa né scarica modelli locali. Il backend è eseguito in locale e ascolta esclusivamente sull’host locale; le richieste al provider vengono inoltrate mediante HTTPS. Il provider selezionato deve elaborare i contenuti del prompt e della risposta nella propria infrastruttura: questa architettura protegge i dati in transito, ma non costituisce cifratura end-to-end.

Le credenziali del provider vengono mantenute nel portachiavi del sistema operativo. L’applicazione non registra analytics, non abilita fallback verso provider non configurati e non conserva le chiavi nella repository, nella configurazione applicativa o nei log.

| Area | Regola del profilo sicuro |
|---|---|
| Inferenza | Un provider cloud esplicitamente selezionato e un modello dichiarato. |
| Modelli locali | Non inclusi, non avviati e non scaricati. |
| Auto-aggiornamento | Disabilitato; gli aggiornamenti sono release manuali. |
| Repository runtime | Incluso nell’installatore; nessun clone remoto al primo avvio. |
| Telemetria | Analytics disabilitata nel profilo di avvio. |
| Credenziali | Solo portachiavi di Windows; nessun salvataggio in chiaro. |

## Azioni locali controllate

Le azioni locali sono abilitate solo dal desktop e non attivano il gruppo degli strumenti pericolosi generici. Il modello cloud può **proporre** un’azione, ma non la esegue direttamente. L’azione appare nella campanella delle approvazioni dell’interfaccia locale; l’utente deve rivedere il percorso e premere **Approve**. Le proposte scadono dopo un’ora, sono marcate come consumate dopo il tentativo di esecuzione e non possono essere riutilizzate.

| Funzione | Limite tecnico |
|---|---|
| Lettura file | Solo file testuali non sensibili nel workspace e in massimo otto cartelle esterne aggiunte manualmente nelle Impostazioni; massimo 256 KB per lettura. |
| Scrittura file | Solo nel workspace o nelle cartelle esterne esplicitamente approvate; solo testo; massimo 1 MB per azione; nessuna sovrascrittura eseguibile o script. |
| Cartelle | Creazione nel workspace o in una cartella esterna approvata, dopo approvazione separata. |
| Apertura applicazione | Un `.exe` Windows esistente tramite percorso assoluto, approvazione obbligatoria e nessun argomento. Shell, script host, installer e utility amministrative sono bloccati. |
| Apertura documento | Documento non macro già esistente (`.txt`, `.md`, `.csv`, `.docx`, `.xlsx`, `.pptx`, `.pdf` e formati equivalenti) nell’app predefinita scelta dall’utente, con approvazione obbligatoria. |
| Chiusura applicazione | Solo processi avviati in precedenza da questo profilo e identificati dal PID, con nuova approvazione. |
| Audit | Registro locale JSONL con ora, esito, percorso o processo; non salva contenuto file né credenziali. |

> Il controllo generico di mouse, tastiera, password, finestre, browser e transazioni non è incluso. È intenzionale: un modello cloud non deve poter inviare input indiscriminato dentro applicazioni con privilegi o dati sensibili. L’apertura documento non invia macro, argomenti, comandi, clic o tasti all’applicazione.

## Istruzioni operative dell’assistente

Ogni agente riceve una policy desktop sicura come prefisso fisso del prompt di sistema. La policy impone di trattare file, pagine, messaggi e output degli strumenti come dati non fidati; vieta di esporre o scrivere credenziali; richiede approvazioni locali monouso per le azioni controllate; vieta shell, esecuzione arbitraria, Docker, privilegi elevati e accesso file libero; e impone di non dichiarare cifratura end-to-end per l’inferenza cloud ordinaria.

Queste istruzioni migliorano la coerenza comportamentale, ma non sono l’unica protezione: i limiti a workspace, applicazioni, provider, output e approvazioni sono applicati anche tecnicamente dal backend e non dipendono dalla sola obbedienza del modello.

## Protezione di output e anteprime

Ogni risposta consegnata dall’API chat viene filtrata localmente prima della visualizzazione, della memoria e delle tracce. Il filtro rimuove caratteri di controllo, limita la dimensione a 64.000 caratteri e redige forme comuni di chiavi e credenziali, tra cui chiavi OpenAI, Anthropic, Google, GitHub, GitLab, Slack, AWS, blocchi di chiave privata e assegnazioni `api_key`, `token`, `password` o `secret`. Per ridurre ulteriormente l’esposizione accidentale redige anche pattern comuni di email, IBAN, codice fiscale italiano, numeri telefonici e numeri di pagamento.

Per evitare l’elusione tramite credenziali distribuite su più token, il percorso di streaming diretto accumula la risposta e la redige integralmente prima della consegna SSE. Ciò privilegia la sicurezza rispetto all’emissione progressiva dei token. Le anteprime delle azioni in attesa non espongono il contenuto completo: mostrano invece il fatto che è redatto, la dimensione e l’impronta SHA-256. I contenuti del workspace che contengono credenziali riconoscibili vengono redatti in lettura e bloccati in scrittura.

Questa è una difesa **best-effort** a pattern e non una garanzia DLP universale: può produrre falsi positivi e non rilevare ogni dato personale o segreto. Le informazioni che non possono essere condivise con il provider non devono entrare nel prompt né nei file o cartelle autorizzati.

## Cosa è escluso

L’applicazione non registra per impostazione predefinita shell, Docker, interprete di codice, REPL, Git, patching generico o scrittura file illimitata. Sono inoltre assenti sidecar, capability shell Tauri, plugin di processo, autostart e updater. Le azioni GitHub della pipeline sono fissate a revisioni immutabili.

## Firma Windows e installazione

La pipeline prepara gli installer `.msi` e `.exe` su un runner Windows e può firmarli solo se sono configurati i segreti `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD` e `WINDOWS_TIMESTAMP_URL`. Il certificato viene importato solo nel runner effimero; il file di configurazione generato è escluso dal controllo versione. Senza questi segreti, la build genera intenzionalmente un installer **non firmato**, soggetto a verifica manuale e a eventuali avvisi di reputazione Windows.

## Spazio richiesto

La sorgente runtime incorporata è circa **24 MB** senza cronologia Git. Una misurazione dell’ambiente Python necessario per backend, provider cloud e dipendenza nativa è circa **406 MB** nello spazio di build verificato. Per Windows è consigliabile riservare **almeno 2 GB liberi** per installazione, cache iniziali e futuri aggiornamenti manuali; non sono inclusi modelli locali.

## Verifiche della release candidate

La release candidate è stata sottoposta a test mirati delle policy privacy, del blocco degli strumenti pericolosi, della shell fail-closed, del workspace controllato e dell’approvazione UI a consumo singolo. Sono inoltre state validate la build frontend, i test Rust del desktop e l’assenza di configurazioni di updater, sidecar Ollama e capability shell nel manifest desktop.
