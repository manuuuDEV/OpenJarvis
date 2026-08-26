# Guardiano di esecuzione Windows — candidato 1.0.7

**Stato:** implementato nel codice e coperto da test automatici; richiede ancora smoke test su Windows reale con Microsoft Defender configurato.
**Ambito:** aperture di documenti e applicazioni **avviate dal canale controllato di OpenJarvis**.
**Non è:** un nuovo motore antivirus, un driver kernel, un sostituto di Microsoft Defender/SmartScreen o un controllo universale di tutti i processi Windows.

## Obiettivo e comportamento

Il guardiano rende obbligatorio un preflight locale prima che un’azione `local_app_open` o `local_document_open`, già approvata dall’utente, possa aprire un file. Il controllo viene eseguito dal backend locale e non viene affidato al modello cloud. Se il controllo non può completarsi, non viene consentito un fallback silenzioso.

| Passaggio | App e installer `.exe` proposti da Jarvis | Documenti approvati |
|---|---|---|
| Verifica percorso | L’azione esiste già solo per un percorso assoluto e un file esistente. | Il documento deve rientrare in cartelle approvate e in una allowlist di formati non macro. |
| Impronta locale | SHA-256 calcolato localmente per audit/correlazione. | SHA-256 calcolato localmente per audit/correlazione. |
| Origine file | Lettura locale, quando presente, di `Zone.Identifier` (Mark-of-the-Web). | Lettura locale, quando presente, di `Zone.Identifier`. |
| Scansione obbligatoria | Scansione personalizzata Microsoft Defender con argomenti fissi e senza shell. | Scansione personalizzata Microsoft Defender con argomenti fissi e senza shell. |
| Reputazione | Controllo di fiducia Defender per eseguibili. | Non applicato: i documenti normalmente non hanno una firma/reputazione di app. |
| Esito inconcludente | **Blocco.** L’azione è consumata e non può essere ripetuta automaticamente. | **Blocco.** L’azione è consumata e non può essere ripetuta automaticamente. |

Il report visualizzato nell’interfaccia di approvazione contiene decisione, stato della scansione Defender, reputazione, provenienza Mark-of-the-Web e SHA-256. Non contiene il contenuto del file e non invia file o hash a provider cloud.

## Controllo di sistema sempre attivo

OpenJarvis aggiunge una pagina read-only nelle Impostazioni che legge la salute antivirus aggregata da Windows Security Center. SmartScreen è riportato come **gestito dal sistema operativo**: l’API utilizzata non espone a una normale app un esito SmartScreen per singolo file. L’app non modifica stato Defender, SmartScreen, esclusioni, quarantena, firme o policy.

Microsoft Defender e SmartScreen restano i livelli da mantenere attivi quando OpenJarvis è chiuso. La protezione in tempo reale di Defender e i controlli reputazionali di SmartScreen appartengono a Windows; OpenJarvis non tenta di replicarli né di aggirarli.[1] [2]

## Limiti obbligatori da dichiarare

| Limite | Conseguenza |
|---|---|
| Il guardiano opera soltanto su azioni di apertura lanciate da OpenJarvis. | Non intercetta gli avvii manuali da Esplora file, browser o altre app. |
| Non esiste una garanzia di scansione in dieci secondi. | I file restano bloccati fino a un esito valido o a un timeout; un timeout è un blocco. |
| La scansione Defender può dipendere da configurazione/policy/elevazione del PC. | Se il comando non è disponibile o non completa, il guardiano blocca l’azione e mostra la motivazione. |
| Un file pulito o reputato non è una garanzia assoluta di assenza di rischio. | L’esito positivo consente il solo canale OpenJarvis; non certifica il file. |
| Nessun file viene mandato a servizi reputazionali cloud dal guardiano. | La privacy è maggiore, ma non esiste qui un secondo motore cloud indipendente. |
| Non gestisce password, OTP, login, pagamenti o modifiche di sicurezza. | Tali flussi restano fuori dal canale di automazione controllata. |

## Prove Windows necessarie

1. Verificare in **Sicurezza di Windows** che Defender e SmartScreen siano attivi; non disabilitare protezioni globali.
2. Aprire OpenJarvis 1.0.7 e controllare nella sezione **Protezione apertura file e app** che il guardiano risulti obbligatorio e che lo stato antivirus sia leggibile.
3. Con una cartella di prova e un file innocuo, chiedere l’apertura controllata e approvare: deve comparire il report di scansione prima dell’apertura.
4. Disabilitare solo in un ambiente di test controllato la disponibilità del comando Defender, oppure simulare un errore senza modificare le protezioni reali: l’apertura deve essere bloccata con un report, non deve proseguire.
5. Provare un eseguibile non riconosciuto solo in una VM o sandbox separata; non usare malware reale. Il comportamento atteso è blocco se la verifica di fiducia Defender non riesce.

## Riferimenti

[1]: https://learn.microsoft.com/en-us/defender-endpoint/run-scan-microsoft-defender-antivirus "Microsoft Defender — scansioni on-demand e protezione real-time"

[2]: https://learn.microsoft.com/en-us/windows/security/operating-system-security/virus-and-threat-protection/microsoft-defender-smartscreen/ "Microsoft Defender SmartScreen — reputazione dei file scaricati"
