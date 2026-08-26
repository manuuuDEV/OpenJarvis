# Provider multipli — fonti tecniche verificate

## Groq

Groq documenta un endpoint di trascrizione OpenAI-compatible `https://api.groq.com/openai/v1/audio/transcriptions` e i modelli `whisper-large-v3` e `whisper-large-v3-turbo`. La documentazione indica inoltre supporto per `webm` e altri formati comuni. Fonte: [GroqDocs — Speech to Text](https://console.groq.com/docs/speech-to-text).

## Provider OpenAI-compatible

| Provider | Dato verificato | Fonte |
|---|---|---|
| Together AI | Base URL `https://api.together.ai/v1`; streaming chat completions supportato. | [Together AI — OpenAI compatibility](https://docs.together.ai/docs/inference/openai-compatibility) |
| Hugging Face Inference Providers | Endpoint chat compatibile `https://router.huggingface.co/v1`; il routing automatico del provider è una funzione del servizio, pertanto l’app non lo usa come fallback implicito tra propri profili. | [Hugging Face — Inference Providers](https://huggingface.co/docs/inference-providers/en/index) |
| NVIDIA NIM | NIM LLM espone endpoint OpenAI-compatible, incluso `POST /v1/chat/completions`, con streaming. | [NVIDIA — NIM API Reference](https://docs.nvidia.com/nim/large-language-models/latest/api-reference.html) |
| SambaNova | Supporta client e streaming OpenAI-compatible; il base URL è fornito dalla console dell’utente. | [SambaNova — OpenAI compatibility](https://docs.sambanova.ai/docs/en/features/openai-compatibility) |
| Alibaba Model Studio | Interfaccia OpenAI-compatible; gli endpoint sono regionali e in alcuni casi dipendono dal workspace. | [Alibaba Cloud — Model Studio OpenAI-compatible Chat](https://www.alibabacloud.com/help/en/model-studio/compatibility-of-openai-with-dashscope) |
| Pollinations | Dichiara API OpenAI-compatible e base `https://gen.pollinations.ai`; la documentazione comprende testo, immagini, audio, realtime voice ed embeddings. | [Pollinations API](https://gen.pollinations.ai/docs) |

## Vincolo di sicurezza adottato

Le chiavi risiedono esclusivamente nel portachiavi del sistema operativo. Il backend riceve soltanto la chiave e l’identità del provider attivo. I provider con endpoint dipendente dall’account o dalla regione (SambaNova e Alibaba) richiedono un URL HTTPS limitato al dominio del provider, non un URL arbitrario.
