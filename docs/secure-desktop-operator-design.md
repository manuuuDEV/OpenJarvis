# Progetto — Operatore desktop locale vincolato

## Obiettivo

L’operatore desktop permette all’assistente cloud di pianificare attività con finestre, browser, mouse e tastiera, ma esegue localmente soltanto istruzioni validate. Il modello **non** riceve un canale di controllo illimitato: descrive un piano strutturato, la componente Windows locale ne verifica ogni passo, applica i divieti e produce un audit redatto.

Microsoft UI Automation fornisce accesso programmatico alla maggior parte degli elementi desktop e pattern discreti come `Invoke`, `Value`, `Selection`, `Scroll` e `Window`; il progetto deve preferire questi elementi identificabili a coordinate globali fragili.[1][2]

## Classificazione dei passi

| Classe | Esempi | Regola di esecuzione |
|---|---|---|
| Osservazione locale | Elencare finestre, leggere titolo e struttura accessibile, verificare il testo di un documento non sensibile. | Esecuzione autonoma limitata ad app già autorizzate; testo e immagini sono trattati come dati non fidati. |
| Interazione reversibile | Mettere a fuoco una finestra, aprire una scheda, seguire un link, scorrere, compilare testo non sensibile non inviato. | Esecuzione autonoma entro budget e finestra vincolati; controllo pre/post-condizione. |
| Scrittura esterna | Incollare testo in un chatbot, inviare un messaggio, pubblicare un modulo, allegare un file. | Anteprima redatta e conferma utente per ogni destinazione e invio. |
| Sensibile o irreversibile | Login, password, OTP, recupero account, banca, investimenti, pagamenti, acquisti, eliminazioni, modifiche account, UAC. | Veto tecnico: l’operatore si arresta e chiede intervento manuale. Nessuna digitazione, lettura, copia o clic finale. |

## Vincoli tecnici non aggirabili

Il piano viene serializzato come dati con schema rigido e con un massimo iniziale di 12 passi e 5 minuti. Ogni passo contiene la finestra target, l’identità dell’applicazione, il selettore UI, l’azione ammessa, una precondizione e una post-condizione. Il broker nativo non accetta espressioni shell, macro, script, URL arbitrari, tasti speciali o coordinate fuori dal rettangolo dell’elemento precedentemente ispezionato.

L’operatore richiede un’associazione tra processo e applicazione autorizzata dall’utente. Il processo deve essere dello stesso utente, non elevato, e identificato da un percorso canonico e un’impronta del file. Il progetto non richiede `uiAccess`, non richiede elevazione e non tenta di automatizzare UAC o desktop di accesso. Microsoft documenta che tali superfici sono protette dalla comunicazione cross-process e che l’accesso a UI a privilegi più alti richiede trust e firma speciali.[3]

I controlli UI che segnalano `IsPassword` sono sempre non leggibili e non scrivibili. Il broker estende il veto con controlli semantici per password, PIN, OTP, 2FA, banca, pagamento, trasferimento, checkout, acquisto, conferma account e informazioni di carta. La proprietà Windows `IsPassword` restituisce vero quando il contenuto è protetto e fornisce quindi un segnale tecnico da applicare localmente.[4]

## Dati, cifratura e audit

La configurazione delle app autorizzate, le procedure registrate e le chiavi di cifratura sono mantenute localmente nel profilo Windows, con segreti nel portachiavi di sistema. Gli audit registrano ID piano, app, azione, ora, esito, ragione del blocco e impronte dei contenuti, ma non testo, password, cookie, schermate complete o dati di pagamento.

La cifratura protegge l’archiviazione locale e il collegamento backend–desktop; non autorizza da sola un’azione rischiosa. L’autorità deriva da: policy locale, utente proprietario della sessione, applicazione autorizzata, selettore vincolato, limite temporale, budget operativo e, quando richiesto, conferma esplicita.

## Esempio: ricerca e confronto in browser

Per il flusso “cerca una ricetta, apri il primo risultato, riassumi, confronta su Gemini”, l’operatore può aprire Chrome, navigare, leggere fonti pubbliche e produrre un riassunto locale. Il contenuto della pagina viene trattato come non affidabile e non può ridefinire la policy. Prima di incollare il riassunto in Gemini, il broker mostra destinazione, testo redatto e avviso di invio a terzi; l’utente deve confermare l’invio. Login a Google/Gemini, richieste di password o verifiche a due fattori arrestano invece il piano.

## Implementazione proposta

La prima iterazione include un broker Windows nativo con UI Automation, una policy engine locale, un ledger di piano monouso e comandi Tauri limitati. L’agente Python può solo creare una proposta strutturata. Il frontend mostra il piano, richiede l’eventuale consenso e invoca il broker; il backend cloud non può richiamare direttamente API di input del sistema operativo.

### Riferimenti

[1] [Microsoft Learn — UI Automation](https://learn.microsoft.com/en-us/windows/win32/winauto/entry-uiauto-win32)

[2] [Microsoft Learn — UI Automation Control Patterns Overview](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-controlpatternsoverview)

[3] [Microsoft Learn — UI Automation Security Overview](https://learn.microsoft.com/en-us/dotnet/framework/ui-automation/ui-automation-security-overview)

[4] [Microsoft Learn — AutomationElement.IsPasswordProperty](https://learn.microsoft.com/en-us/dotnet/api/system.windows.automation.automationelement.ispasswordproperty?view=windowsdesktop-10.0)

## Stato della prima implementazione

| Capacità | Stato corrente | Condizione per attivazione finale |
|---|---|---|
| Proposta di piano strutturato | Implementata e testata nel backend locale. | Nessuna: resta sempre una proposta a scadenza breve. |
| Veto login, password, OTP, banca, pagamenti, recupero e invii | Implementato e testato prima dell’approvazione. | Rieseguito anche dal broker nativo al momento dell’azione. |
| Identità di finestra | Il piano richiede il percorso assoluto dell’eseguibile Windows e il titolo della finestra. | Il broker deve confrontare processo, percorso canonico e finestra reale. |
| Invocazione di elementi UI | Progettata su UI Automation, senza coordinate globali. | Compilazione e test su Windows reale/runnner Windows. |
| Scrittura di testo non sensibile | Progettata per controlli abilitati, scrivibili e non password. | Compilazione e test su Windows reale/runnner Windows. |
| Mouse/tastiera globali | Non previsti. | Restano vietati; il broker opera solo su elementi UI verificati. |
| Invio a servizi terzi | Richiede conferma finale dell’utente. | Anteprima della destinazione e del contenuto redatto. |

## Broker nativo implementato nella release candidate

La release candidate ora include `desktop_broker.rs`, attivato soltanto su Windows. Al boot il desktop genera un token casuale di 256 bit, lo passa esclusivamente al backend locale e avvia il worker solo dopo che l’API locale risulta pronta. Il worker può vedere solo gli identificativi dei piani approvati, effettua un claim atomico e lo consuma anche dopo un errore; un secondo broker o un replay ricevono un conflitto.

Per ciascun piano il broker controlla nuovamente il percorso dell’eseguibile della finestra, associa la finestra al processo Windows e confronta il percorso del processo. La ricerca UI Automation è confinata alla finestra, quindi ricontrolla PID, nome e/o `AutomationId`, tipo di controllo, abilitazione e `CurrentIsPassword`. Può soltanto mettere a fuoco la finestra, ispezionare testo accessibile, invocare un controllo o impostare testo non sensibile tramite il pattern `Value`. Non usa coordinate, `SendInput`, mouse globale, tastiera globale, clipboard, shell, PowerShell, macro o privilegi elevati.

La compilazione e il collaudo del modulo `cfg(target_os = "windows")` restano intenzionalmente separati e richiedono una build Windows reale; non sono stati dichiarati completati in questo ambiente Linux.
