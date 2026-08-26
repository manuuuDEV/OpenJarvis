# OpenJarvis Desktop Cloud 1.0.7 — audit del guardiano di esecuzione

**Codice dell’artefatto:** [`c9278c9`](https://github.com/manuuuDEV/OpenJarvis/commit/c9278c9).
**Branch:** `release/v1.0.2-rc1`.
**Stato:** build privata Windows completata; **non** ancora verificata su un PC Windows reale con Microsoft Defender configurato.

## Cosa aggiunge la 1.0.7

La release introduce un **guardiano obbligatorio** per le aperture di applicazioni e documenti proposte dal canale locale controllato di OpenJarvis. Dopo l’approvazione dell’utente, ma prima dell’apertura, l’app calcola l’impronta SHA-256 in locale, legge la provenienza Mark-of-the-Web quando disponibile e richiede una scansione personalizzata di Microsoft Defender. Per gli eseguibili verifica inoltre lo stato di fiducia tramite Defender.

Un controllo non disponibile, in errore, scaduto o inconcludente produce un **blocco fail-safe**. L’approvazione viene consumata e non può essere riusata per ritentare automaticamente. Il pannello di approvazione mostra decisione, stato Defender, reputazione, origine e hash; il contenuto del file non viene inviato a provider cloud.

| Area | Stato 1.0.7 | Limite onesto |
|---|---|---|
| Aperture di app/documenti avviate da Jarvis | Guardiano implementato e coperto da test. | Richiede smoke test Windows; dipende da Microsoft Defender configurato e disponibile. |
| Installer o programmi `setup.exe` avviati dal canale controllato | Passano dallo stesso preflight di un eseguibile. | Il canale non espone shell, argomenti arbitrari, `msiexec` o elevazione. |
| App avviate manualmente dall’utente mentre Jarvis è chiuso | Non intercettate dal guardiano OpenJarvis. | La protezione deve essere fornita da Defender/SmartScreen o endpoint security installato. |
| Stato antivirus | Riquadro read-only in Impostazioni basato su Windows Security Center. | SmartScreen è riportato come gestito dal sistema; non esiste qui un’API di verdict per file. |
| Modifica di Defender/SmartScreen, esclusioni o quarantena | Non implementata. | L’app non disabilita né altera protezioni Windows. |

> Il guardiano non è un antivirus indipendente e non può certificare che un file pulito sia privo di rischio. È una barriera aggiuntiva e conservativa per il solo canale di apertura controllata di Jarvis.

## Risultati di verifica

| Verifica | Esito | Nota |
|---|---:|---|
| Nuovi test guardiano e approvazioni locali | **12 passati** | Coprono guardiano disabilitato, scan/trust, blocco fail-safe, consumo monouso e endpoint read-only. |
| Regressione sicurezza, deployment e prompt | **516 passati, 7 saltati** | Copre browser policy, azioni locali, approvazioni, ADB, prompt e CSP desktop. |
| Frontend sorgente e bundle | **Passati** | Test frontend passati e Vite ha prodotto il bundle; resta un avviso di chunk principale grande. |
| Rust/Tauri Linux | **36 passati** | Conferma il bootstrap nativo compilabile nell’ambiente Linux. |
| CI Windows | **Passata** | Runner Windows: validazione e bundle privato completati.[1] |
| Suite Python completa | **7.972 passati, 115 falliti, 71 saltati** | I fallimenti upstream preesistenti restano non risolti; non sono presentati come verdi. |

I fallimenti della suite completa continuano a concentrarsi in engine/provider live privi di credenziali, test browser senza Playwright, skill/engine non presenti e tool upstream HTTP/Git/MCP/storage. Le nuove regressioni di sicurezza sono verdi, ma ciò non rende verificati tutti i sottosistemi storici del repository.

## Artefatti Windows verificati

| File | SHA-256 |
|---|---|
| `OpenJarvis_1.0.7_x64-setup.exe` | `3bf756824cb6ff6dc94ffa441651de1fb3ab386b576c24258401a8f02063f4e3` |
| `OpenJarvis_1.0.7_x64_en-US.msi` | `9b9bc1e866b2bc02e842a78b4b73e80bf4938846b685799e2c7e36c6d73113e5` |

L’artefatto è privato e l’installer non dispone di una firma codice fornita dall’utente. SmartScreen può avvisare: verificare l’hash e non disabilitare Defender o le protezioni Windows in modo globale.

## Smoke test Windows richiesti

1. Verificare l’hash dell’EXE, installare la 1.0.7 e controllare che la navigazione **Chat → Dashboard → Logs → Settings → Chat** non reindirizzi a Settings.
2. In Impostazioni, aprire **Protezione apertura file e app**. Il guardiano deve risultare obbligatorio e lo stato Windows Security deve essere leggibile.
3. Con una cartella di prova e un file innocuo, richiedere un’apertura controllata, approvare e verificare che il report appaia prima dell’apertura.
4. Non usare malware reale. Qualsiasi prova di file sconosciuto va fatta solo in VM/sandbox e non deve mai richiedere di disabilitare Defender/SmartScreen.
5. Verificare separatamente chiavi provider, Gemini Live, UI Automation e ADB: la compilazione CI non prova microfono, Credential Manager, UIA o Android reali.

## Riferimenti

[1]: https://github.com/manuuuDEV/OpenJarvis/actions/runs/33021772589 "GitHub Actions — Desktop Build & Release, run 33021772589"
