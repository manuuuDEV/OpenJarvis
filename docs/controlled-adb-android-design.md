# Modulo ADB Android controllato — design v1

## Obiettivo

Consentire a OpenJarvis Desktop di eseguire una **diagnostica software Android in sola lettura** su un singolo dispositivo che l’utente ha collegato, sbloccato e autorizzato tramite ADB. Il modello cloud non riceve un terminale ADB né il seriale completo del dispositivo, e non può eseguire comandi arbitrari.

## Limiti applicati tecnicamente

| Area | Regola v1 |
|---|---|
| Autorizzazione Android | L’utente deve attivare USB debugging e accettare personalmente il dialogo RSA sul dispositivo. Un device `unauthorized`, `offline` o assente viene rifiutato. |
| Identità del dispositivo | L’utente sceglie localmente un seriale ADB tra i device rilevati. Il seriale resta nella configurazione nativa; il modello può proporre soltanto una scansione sul device già autorizzato. |
| Binario ADB | Solo percorso locale esplicitamente scelto dall’utente, esistente, assoluto e con nome `adb.exe`; nessuna stringa comando, shell, `cmd`, PowerShell o eseguibile generico. |
| Comandi | Allowlist nativa fissa: `getprop` selezionati, `wm size`, `df /data`, `dumpsys meminfo`, `dumpsys battery`, `pm list packages -3`. Ogni comando usa `adb -s <serial> shell` con argomenti fissi. |
| Output | Timeout per singolo comando, limite byte, parsing locale e restituzione del solo riepilogo diagnostico. Nessun output raw, seriale, SSID, logcat, screenshot o lista completa delle app viene inoltrato al modello. |
| App Android | Nessun tap/swipe/input, nessuna apertura generica di app, installazione/disinstallazione, push/pull file, port forwarding, wireless pairing, root o `su`. |
| Approvazione | Se la scansione è richiesta dall’agente, viene messa in coda come azione ad alto rischio e richiede approvazione one-shot nell’app prima del broker nativo. |

## Diagnostica prodotta

La scansione può riportare versione Android, produttore/modello, risoluzione, spazio dati libero/totale, stato batteria, indicatori memoria e conteggio delle app di terze parti. Può segnalare, in linguaggio prudente, spazio insufficiente o batteria bassa; non formula diagnosi hardware, non rileva malware in modo certificato e non modifica il dispositivo.

> ADB è un canale di debug potente. In questo progetto è intenzionalmente trattato come una superficie ad alto rischio: la regola non è "il modello può usare ADB", ma "il broker nativo può eseguire solo una piccola lista di letture predeterminate su un device che l’utente ha scelto".

## Riferimenti

[1]: https://developer.android.com/tools/adb "Android Developers — Android Debug Bridge (adb)"
[2]: https://developer.android.com/tools/dumpsys "Android Developers — dumpsys"
