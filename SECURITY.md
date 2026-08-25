# Security Policy

## Supported branch

Security fixes are developed on the repository's default branch and released through reviewed, immutable commit SHAs. Users should install only a commit they have independently reviewed.

## Reporting a vulnerability

Do **not** open a public issue for a suspected vulnerability, exposed credential, privacy bypass, supply-chain compromise, or remote-code-execution issue. Instead, contact the repository owner privately through GitHub's private vulnerability reporting feature, if enabled, or through a private channel agreed with the maintainer.

A useful report includes the affected commit SHA, reproduction steps, impact, any proof of concept that avoids accessing third-party data, and suggested mitigations. Reports should not contain real API keys, wallet credentials, user data, or private prompts.

## Privacy boundary

OpenJarvis defaults to `privacy.mode = "local_only"`. External inference requires explicit provider allowlisting. HTTPS protects data in transit only; it does not make a normal cloud inference API end-to-end encrypted against the selected provider. See [`docs/privacy-boundary.md`](docs/privacy-boundary.md).

## Disclosure process

The maintainer should acknowledge a complete private report within seven days, investigate without requesting exploit publication, prepare a tested fix, and coordinate disclosure with the reporter. Security releases should include the fixed commit SHA, affected versions, mitigation guidance, and a concise impact description without publishing sensitive exploit details prematurely.
