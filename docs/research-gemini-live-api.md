# Gemini Live API — verifica per OpenJarvis desktop

## Stato verificato

La Gemini Live API è indicata da Google come **Preview**. Offre un canale stateful WebSocket sicuro (WSS) per interazioni continue audio, immagini e testo. L’input audio è PCM raw a 16 bit, 16 kHz, little-endian; l’output audio è PCM raw a 16 bit, 24 kHz, little-endian. [1]

Il modello che Google mostra per questo scenario è **`gemini-3.1-flash-live-preview`**. Per collegamenti diretti dal client, Google raccomanda token effimeri invece di una API key ordinaria; anche i token effimeri sono Preview. [1] [2]

## Conseguenza per l’architettura desktop

| Aspetto | Vincolo applicato in OpenJarvis | Motivazione |
|---|---|---|
| Chiave Gemini a lungo termine | Deve restare nel portachiavi del sistema operativo. Non viene inviata a React/WebView, JavaScript browser o URL. | Una chiave client-side può essere estratta. [2] |
| Collegamento Live dal renderer | Non abilitato nella release candidate. | Richiede token effimero o un bridge nativo verificato; un WebSocket con API key dal renderer violerebbe l’isolamento della chiave. [1] [2] |
| Token effimero | Deve essere creato da un backend/bridge autenticato, limitato a un uso, durata breve e vincolato al modello e alle sole risposte audio. | È il flusso client-to-server raccomandato da Google. [2] |
| Conversazione presente | Dettatura **a turni**: registrazione, stop, trascrizione Groq, revisione nel campo chat, invio manuale e risposta testuale. | È concreta, non richiede la chiave Gemini nel renderer e non viene chiamata impropriamente “live”. |

> La protezione TLS riguarda il trasporto. Groq o Google, quando selezionati e autorizzati, elaborano rispettivamente audio o contenuto nel proprio confine cloud. Questa architettura non dichiara cifratura end-to-end verso il provider.

## Implementazione differita per il vero live audio-audio

Per abilitare Gemini Live in modo coerente con la postura privacy-first servono un broker nativo Tauri o un servizio locale autenticato che legga la chiave Gemini unicamente dal portachiavi, crei un token effimero `v1beta/auth_tokens` limitato a `gemini-3.1-flash-live-preview`, consegni al renderer solo il token breve, e gestisca la revoca/chiusura della sessione. Il renderer dovrà catturare PCM compatibile, usare il token solo nella sessione WSS autorizzata e non persistere l’audio.

Questa parte non è implementata nella release candidate: non è sicuro dichiarare disponibile una conversazione Live prima di validare il bridge e il comportamento su Windows.

## Riferimenti

[1]: https://ai.google.dev/gemini-api/docs/live "Google AI for Developers — Gemini Live API overview"
[2]: https://ai.google.dev/gemini-api/docs/ephemeral-tokens "Google AI for Developers — Ephemeral tokens"

## Token effimeri: protocollo verificato il 26 agosto 2026

Google documenta il provisioning tramite `POST https://generativelanguage.googleapis.com/v1beta/auth_tokens`, autenticato dal runtime nativo con `x-goog-api-key`. Il token può essere vincolato a un solo avvio di sessione (`uses: 1`), a una breve `newSessionExpireTime` e a una `expireTime` limitata; può anche essere limitato a `models/gemini-3.1-flash-live-preview`, a `responseModalities: ["AUDIO"]` e alla ripresa sessione.

Il token restituito è usabile soltanto da Live API v1beta. In un WebSocket nativo deve essere inviato come `access_token` oppure nell’header `Authorization` con schema `Token`. Il renderer OpenJarvis non riceverà né persisterà la chiave Gemini standard: il runtime Rust la legge dal keyring e il token temporaneo, se necessario, deve rimanere a uso singolo e con vincoli server-side.

[3]: https://ai.google.dev/gemini-api/docs/live-api/ephemeral-tokens "Google AI — Ephemeral tokens"

## Esempio ufficiale WebSocket e audio

L’esempio ufficiale `gemini-live-ephemeral-tokens-websocket` usa un WebSocket diretto dal client a `BidiGenerateContentConstrained` con `access_token` e invia un messaggio `setup` contenente il modello, `generationConfig.responseModalities: ["AUDIO"]`, configurazione voce e `realtimeInputConfig`. Per l’audio invia `realtimeInput.audio` come `inlineData` con `mimeType: "audio/pcm"` e base64. Il campione cattura microfono a PCM16 16 kHz e riproduce l’output PCM16 24 kHz. L’audio e il token non devono essere salvati.

OpenJarvis non adotterà il server Python di esempio o il suo file `.env`: il provisioning resterà un comando Tauri nativo che legge `GEMINI_API_KEY` esclusivamente dal keyring. Inoltre non abiliterà camera, schermo, Google Search o function calling nel canale Live iniziale.

Fonti: [Gemini Live vanilla JS example](https://github.com/google-gemini/gemini-live-api-examples/blob/main/gemini-live-ephemeral-tokens-websocket/README.md), [geminilive.js](https://raw.githubusercontent.com/google-gemini/gemini-live-api-examples/main/gemini-live-ephemeral-tokens-websocket/frontend/geminilive.js), [mediaUtils.js](https://raw.githubusercontent.com/google-gemini/gemini-live-api-examples/main/gemini-live-ephemeral-tokens-websocket/frontend/mediaUtils.js).

## Endpoint v1beta e messaggi definitivi

La guida Google del 23 luglio 2026 specifica che, con token effimeri, la connessione deve usare `wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContentConstrained?access_token={token}`. Il primo messaggio è `setup` con `model: "models/gemini-3.1-flash-live-preview"` e `responseModalities: ["AUDIO"]`. I chunk in ingresso devono essere `realtimeInput.audio` con `mimeType: "audio/pcm;rate=16000"` e base64 di PCM16 little-endian. L’audio ricevuto è base64 dentro `serverContent.modelTurn.parts[].inlineData`; l’output nativo è PCM16 a 24 kHz.

L’implementazione OpenJarvis iniziale non dichiara tool calling, Google Search, camera, screen capture o function calls nel setup Live. Qualsiasi campo `toolCall` in risposta deve essere ignorato e la sessione può essere chiusa: la conversazione Live non diventa un bypass delle approvazioni, dei broker o delle policy locali.

Fonte: [Google AI — Get started with Gemini Live API using WebSockets](https://ai.google.dev/gemini-api/docs/live-api/get-started-websocket).
