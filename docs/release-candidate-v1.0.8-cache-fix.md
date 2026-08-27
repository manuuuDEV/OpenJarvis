# OpenJarvis Desktop Cloud 1.0.8 — correzione cache UI Windows

**Codice dell’artefatto:** [`215ec4e`](https://github.com/manuuuDEV/OpenJarvis/commit/215ec4e).
**Branch:** `release/v1.0.2-rc1`.
**Stato:** CI Windows completata; attende la conferma visiva dell’utente sul suo PC.

## Difetto confermato

La registrazione dell’utente mostrava la versione binaria **1.0.7**, ma nell’interfaccia apparivano i campi legacy `API URL`, `API key` e `OPENJARVIS_API_KEY`; non compariva la sezione **Protezione apertura file e app**. Il collegamento del menu Start puntava a `C:\Users\mazza\AppData\Local\OpenJarvis\openjarvis-desktop.exe` con ProductVersion/FileVersion `1.0.7`.

Il sintomo dimostra che il problema non era un installer precedente. L’ipotesi tecnica riproducibile era l’uso di una PWA nel bundle Tauri: il suo service worker dispone di cache persistente e può servire asset frontend precedenti dopo un aggiornamento della app WebView.

## Correzione 1.0.8

| Modifica | Effetto atteso |
|---|---|
| PWA abilitata solo per build browser | Quando Tauri imposta `TAURI_ENV_PLATFORM`, Vite non genera `sw.js` né `registerSW.js` per il bundle desktop. |
| Nuova directory dati WebView `openjarvis-desktop-ui-v2` | Il WebView della 1.0.8 non riutilizza la cache/service worker legacy della precedente directory dati. |
| Regressioni di deployment | I test impongono la directory dati dedicata e l’uso condizionale della PWA. |
| Versione 1.0.8 | Distingue il binario corretto dalla 1.0.7 verificata nel video. |

Le API key del provider non dipendono da localStorage: nel profilo desktop sono archiviate nel keyring del sistema. La nuova directory WebView può quindi azzerare preferenze frontend non critiche e vecchi valori legacy, ma non deve eliminare le chiavi provider memorizzate dal processo nativo.

## Verifiche eseguite

| Verifica | Esito |
|---|---:|
| Regressioni deployment/PWA | **6 passate** |
| Test frontend | **Passati** |
| Bundle con `TAURI_ENV_PLATFORM=windows` | **Passato**; `dist` non contiene `sw.js` né `registerSW.js` e contiene “Protezione apertura file e app”. |
| Test Rust/Tauri | **36 passati** |
| CI Windows | **Passata**: validazione e artefatto privato completati.[1] |

## Artefatti verificati

| File | SHA-256 |
|---|---|
| `OpenJarvis_1.0.8_x64-setup.exe` | `2abc4e7d5723968405491ddece564e7fb16921209a859210266eb2712195869b` |
| `OpenJarvis_1.0.8_x64_en-US.msi` | `630e087d490a1e24f2c96ff187ad18812599fce5e303c3b0d8a13c87360caf09` |

## Prova Windows obbligatoria

1. Chiudere tutte le istanze OpenJarvis e installare la 1.0.8 dopo aver verificato l’hash.
2. Aprire dal menu Start e andare a **Settings**.
3. Confermare che la sezione Connection mostra **Backend desktop** e non i campi `API URL` / `API key`.
4. Scorrere la pagina e confermare la presenza di **Protezione apertura file e app**.
5. Testare la navigazione **Chat → Dashboard → Logs → Settings → Chat** e inviare una nuova registrazione solo se il comportamento è ancora errato.

L’installer rimane privo di firma codice. Non disabilitare Defender o SmartScreen in modo globale; in caso di avviso, verificare il nome e l’hash dell’artefatto.

## Riferimento

[1]: https://github.com/manuuuDEV/OpenJarvis/actions/runs/33028253897 "GitHub Actions — build privata Windows 1.0.8"
