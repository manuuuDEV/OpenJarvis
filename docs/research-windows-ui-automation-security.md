# Fonti — operatore desktop Windows controllato

## Microsoft UI Automation

Microsoft descrive **UI Automation** come framework di accessibilità che fornisce accesso programmatico alla maggior parte degli elementi dell’interfaccia desktop e consente tecnologie assistive e test automatizzati di manipolare la UI senza input standard. Questo consente un’implementazione basata su elementi e pattern UI, preferibile alla sola lettura di coordinate schermo.

Fonte: [Microsoft Learn — UI Automation](https://learn.microsoft.com/en-us/windows/win32/winauto/entry-uiauto-win32)

## Pattern e confini operativi

I control pattern UI Automation espongono operazioni discrete e dichiarate, come `Invoke` per pulsanti, `Value` per controlli con valore, `Selection`, `Scroll`, `Text` e `Window`. Un operatore sicuro può consentire solo un sottoinsieme di questi pattern su una finestra e un’applicazione preventivamente autorizzate, invece di consentire clic e input globali.

Fonte: [Microsoft Learn — UI Automation Control Patterns Overview](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-controlpatternsoverview)

## Privilegi elevati e UI protetta

La panoramica di sicurezza Microsoft spiega che prompt UAC e schermata di accesso sono protetti dalla comunicazione cross-process; un client UI Automation che deve accedere a UI a privilegio più alto richiede trust speciale, firma e `uiAccess`. Il profilo OpenJarvis deve quindi **non** richiedere `uiAccess`, non automatizzare UAC, non elevare privilegi e non tentare accesso a login desktop.

Fonte: [Microsoft Learn — UI Automation Security Overview](https://learn.microsoft.com/en-us/dotnet/framework/ui-automation/ui-automation-security-overview)

## Campi password

La proprietà `AutomationElement.IsPasswordProperty` restituisce un booleano che indica contenuto protetto. Nel progetto dovrà essere usata come veto tecnico: nessuna lettura, copia, scrittura o automazione del controllo quando la proprietà è vera, oltre a euristiche aggiuntive per OTP, pagamenti e login.

Fonte: [Microsoft Learn — AutomationElement.IsPasswordProperty](https://learn.microsoft.com/en-us/dotnet/api/system.windows.automation.automationelement.ispasswordproperty?view=windowsdesktop-10.0)

## Broker Rust e ricerca dell’elemento

L’esempio ufficiale `windows-rs` usa `CoInitializeEx`, `CUIAutomation`, `IUIAutomation::ElementFromHandle` e l’elemento della finestra; il broker può quindi vincolare ogni ricerca all’handle della finestra autorizzata anziché partire dal desktop globale. Microsoft raccomanda di iniziare la ricerca dalla finestra applicativa o da un contenitore basso, perché la ricerca tra tutti i discendenti del desktop può attraversare migliaia di elementi.

Fonti: [windows-rs — esempio UI Automation](https://github.com/microsoft/windows-rs/blob/master/crates/samples/windows/uiautomation/src/main.rs), [Microsoft Learn — Obtaining UI Automation Elements](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-obtainingelements), [Microsoft Learn — Rust for Windows](https://learn.microsoft.com/en-us/windows/dev-environment/rust/rust-for-windows)

## Precondizioni di invocazione e scrittura

`IUIAutomationInvokePattern::Invoke` invoca l’azione di un controllo (per esempio un pulsante) e può restituire subito; il broker deve quindi verificare la post-condizione prima di passare allo step successivo. `IUIAutomationValuePattern::SetValue` richiede che il controllo sia abilitato e non in sola lettura. Prima di entrambi, il broker deve rifiutare elementi `CurrentIsPassword` e gli elementi che ricadono nella policy semantica sensibile.

Fonti: [Microsoft Learn — Invoke](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationinvokepattern-invoke), [Microsoft Learn — SetValue](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationvaluepattern-setvalue), [Microsoft Learn — CurrentIsPassword](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationelement-get_currentispassword)

## Firme windows-rs verificate

La documentazione generata di `windows-rs` espone `IUIAutomationElement::SetFocus`, `FindFirst`, `CurrentProcessId`, `CurrentName`, `CurrentAutomationId`, `CurrentIsEnabled`, `CurrentIsPassword` e `GetCurrentPatternAs`. Per le azioni il wrapper espone `IUIAutomationInvokePattern::Invoke()` e `IUIAutomationValuePattern::SetValue(&BSTR)`, con `CurrentIsReadOnly`. Queste firme permettono al broker di usare elementi UI identificati, verificare processo/finestra/password/abilitazione e applicare solo Invoke o SetValue invece di mouse e tastiera globali.

Fonti: [windows-rs — IUIAutomationElement](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Accessibility/struct.IUIAutomationElement.html), [windows-rs — IUIAutomationInvokePattern](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Accessibility/struct.IUIAutomationInvokePattern.html), [windows-rs — IUIAutomationValuePattern](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/Accessibility/struct.IUIAutomationValuePattern.html)
