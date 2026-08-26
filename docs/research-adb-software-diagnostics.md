# ADB Android — diagnostica software controllata

## Fatti verificati

Android Debug Bridge (ADB) è un client-server che permette a un computer di comunicare con il demone `adbd` sul dispositivo. Per l’uso USB occorre abilitare USB debugging; da Android 4.2.2 in poi il dispositivo presenta un dialogo RSA che deve essere confermato sul telefono prima che il computer possa inviare comandi ADB. La documentazione precisa inoltre che, in presenza di più dispositivi, i comandi devono indicare esplicitamente il seriale con `-s`. [1]

`dumpsys` raccoglie informazioni da servizi di sistema e la documentazione Android raccomanda di richiedere singoli servizi perché il dump globale è eccessivamente verboso. Le aree documentate includono memoria (`meminfo`), batteria (`batterystats`), input, rete e performance UI; alcuni output possono contenere nomi app, SSID, identificatori o altri dati personali. [2]

## Profilo proposto per OpenJarvis

| Categoria | Consentito di default | Escluso tecnicamente |
|---|---|---|
| Identificazione device | `adb devices -l`, solo dispositivi nello stato `device`; seriale mostrato in forma ridotta/redatta | Connessione a IP arbitrari, pairing Wi-Fi automatico, selezione implicita del primo device |
| Inventario software | `getprop` allowlisted, `wm size`, `df /data`, `pm list packages -3`, `dumpsys meminfo`, `dumpsys battery`, `dumpsys package` limitato | `adb shell` generica, `su`, root, install/disinstallazione, push/pull file |
| Diagnostica | Letture bounded con timeout, limite output e redazione prima di UI/audit | `logcat` integrale, `bugreport`, screenshot/screencap, registrazione audio/video, dump globale `dumpsys` |
| Apertura app | Da valutare come operazione distinta con approvazione one-shot e package ID convalidato | Tap/swipe/input text, coordinate, invio tasti, login/account, pagamenti, password, OTP |

> Questo profilo può diagnosticare problemi software comuni — memoria insufficiente, spazio ridotto, versione Android, app utente, crash/ANR in summary limitati, stato batteria e servizi — ma non deve essere presentato come antivirus, riparazione automatica o diagnosi hardware certificata.

## Riferimenti

[1]: https://developer.android.com/tools/adb "Android Developers — Android Debug Bridge (adb)"
[2]: https://developer.android.com/tools/dumpsys "Android Developers — dumpsys"
