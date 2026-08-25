# Privacy boundary and external inference

OpenJarvis is **local-only by default**. It does not send inference requests to an endpoint outside the local loopback interface unless the user explicitly enables and allowlists a provider.

> **A normal cloud model API is not end-to-end encrypted from the user to the model provider.** HTTPS/TLS encrypts the request in transit, but the selected provider must process the prompt and completion in plaintext (or decrypt them in its own trusted execution boundary) to run inference. Do not configure an external provider for data that must never be visible to that provider.

## Privacy modes

| Mode | Behaviour | Appropriate use |
|---|---|---|
| `local_only` | Default. Blocks all non-loopback inference endpoints and all generic cloud engines. | Highest practical privacy with local inference. |
| `explicit_external` | Allows only the named providers listed in `approved_external_providers`; requires HTTPS by default. | Deliberate use of a cloud API after accepting that it processes prompt/output plaintext. |
| `confidential_compute` | Fails closed. Generic APIs are blocked until OpenJarvis can verify remote attestation and bind request keys to a supported confidential-computing runtime. | Workloads that require a cryptographic provider-side trust boundary. |

## External provider configuration

Use only the provider needed for the current workload. Keep API keys in your operating system secret store or environment; never commit them to `config.toml` or a repository.

```toml
[privacy]
mode = "explicit_external"
approved_external_providers = "openai"
require_tls = true

[analytics]
enabled = false

[security]
# With an external provider, input containing detected PII or secrets is
# blocked instead of being forwarded after redaction.
mode = "block"
```

Recognized provider names are `openai`, `anthropic`, `google`, `openrouter`, `minimax`, `deepseek`, `codex`, `nim`, and `litellm`. Adding a key for a provider does not by itself grant consent: the provider must also be allowlisted.

## What is protected

The policy blocks accidental fallback to generic cloud engines, rejects an external `http://` endpoint when TLS is required, prevents a cloud provider not named in the allowlist from receiving prompts, and prevents NVIDIA NIM's default cloud endpoint from being probed or used without consent. It also forces PII/secret guardrails into `block` mode when external inference is enabled.

## What is not claimed

This policy does not create homomorphic encryption, end-to-end encryption against a standard cloud provider, or a guarantee that an external provider cannot inspect the plaintext it receives. A supported confidential-computing provider with independently verified remote attestation is required for that stronger property. Until such support exists, use `local_only` for requests that must remain exclusively on your device.

## References

1. [NIST — Confidential Computing](https://csrc.nist.gov/glossary/term/confidential_computing)
2. [Anthropic — Confidential Inference via Trusted Virtual Machines](https://www.anthropic.com/research/confidential-inference-trusted-vms)
