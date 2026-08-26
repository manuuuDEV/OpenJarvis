# Fonti di build verificate

## Toolchain Node.js

La validazione frontend della release hardenizzata usa Node.js **v22.22.0** e npm **v11.19.0**, in coerenza con `frontend/package.json`.

| Elemento | Valore verificato |
|---|---|
| Archivio Node.js Linux x64 | `node-v22.22.0-linux-x64.tar.xz` |
| SHA-256 | `9aa8e9d2298ab68c600bd6fb86a6c13bce11a4eca1ba9b39d79fa021755d7c37` |
| npm | `11.19.0` |
| Integrità npm (SHA-512 SRI) | `sha512-SDd/hHg3KqHE5Ht2NHWxNYNtqCQ2pXAPLl6OtQhPyED5PHsRfrOtO199MZTIG2cQoQ1ZRI9t28shrD+2cr3AAw==` |

Le impronte provengono dalla distribuzione ufficiale Node.js. L’archivio Node è stato verificato localmente con SHA-256 prima dell’uso; npm è stato installato in un prefisso isolato con script dei pacchetti disabilitati.

## Fonti

[1]: https://nodejs.org/download/release/v22.22.0/SHASUMS256.txt "Node.js v22.22.0 — SHA-256 ufficiali"
[2]: https://nodejs.org/download/release/v22.22.0/ "Node.js v22.22.0 — archivio ufficiale"
[3]: https://registry.npmjs.org/npm/-/npm-11.19.0.tgz "npm v11.19.0 — archivio di distribuzione"
