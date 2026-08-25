"""Outbound privacy policy for inference and tool-adjacent network boundaries.

This module intentionally does *not* claim that client-side encryption makes a
normal cloud LLM API end-to-end encrypted.  A provider must receive plaintext
(or decrypt it inside an independently verifiable confidential-computing
boundary) to perform standard inference.  Consequently, ``confidential_compute``
blocks generic APIs until remote attestation is implemented for a supported
provider profile.
"""

from __future__ import annotations

import ipaddress
from dataclasses import dataclass
from urllib.parse import urlsplit


PRIVACY_MODES = frozenset({"local_only", "explicit_external", "confidential_compute"})


class PrivacyPolicyError(PermissionError):
    """Raised before a request would cross a disallowed privacy boundary."""


def _normalise_provider(value: str) -> str:
    return value.strip().lower().replace("-", "_")


def is_loopback_endpoint(endpoint: str) -> bool:
    """Return whether an HTTP(S) endpoint resolves to a loopback hostname.

    DNS is deliberately not resolved here: privacy decisions must be stable and
    must not trigger a network request merely to inspect a configuration value.
    Private-LAN addresses are not treated as local because they cross a network
    boundary and can be routed to another machine.
    """

    try:
        hostname = urlsplit(endpoint).hostname
    except ValueError:
        return False
    if not hostname:
        return False
    hostname = hostname.rstrip(".").lower()
    if hostname == "localhost":
        return True
    try:
        return ipaddress.ip_address(hostname).is_loopback
    except ValueError:
        return False


@dataclass(frozen=True, slots=True)
class PrivacyPolicy:
    """Policy controlling outbound inference destinations.

    ``local_only`` is the secure default and permits only loopback endpoints.
    ``explicit_external`` permits only an explicitly allowlisted provider and
    requires HTTPS.  It protects data in transit, but does not hide plaintext
    prompts or completions from the selected provider.
    ``confidential_compute`` fails closed for generic provider APIs because
    OpenJarvis does not yet verify remote hardware attestation or bind keys to a
    measured confidential runtime.
    """

    mode: str = "local_only"
    approved_external_providers: tuple[str, ...] = ()
    require_tls: bool = True

    def __post_init__(self) -> None:
        normalised_mode = _normalise_provider(self.mode)
        if normalised_mode not in PRIVACY_MODES:
            raise ValueError(
                "privacy.mode must be one of "
                f"{sorted(PRIVACY_MODES)}, got {self.mode!r}"
            )
        object.__setattr__(self, "mode", normalised_mode)
        object.__setattr__(
            self,
            "approved_external_providers",
            tuple(
                sorted(
                    {
                        _normalise_provider(provider)
                        for provider in self.approved_external_providers
                        if provider and provider.strip()
                    }
                )
            ),
        )

    @classmethod
    def from_config(cls, config: object) -> "PrivacyPolicy":
        privacy = getattr(config, "privacy")
        raw_providers = getattr(privacy, "approved_external_providers", "")
        if isinstance(raw_providers, str):
            providers = tuple(item.strip() for item in raw_providers.split(","))
        else:
            providers = tuple(str(item).strip() for item in raw_providers)
        return cls(
            mode=getattr(privacy, "mode", "local_only"),
            approved_external_providers=providers,
            require_tls=bool(getattr(privacy, "require_tls", True)),
        )

    def allows_external_provider(self, provider: str) -> bool:
        """Return whether a named external provider is explicitly permitted."""

        provider = _normalise_provider(provider)
        return (
            self.mode == "explicit_external"
            and provider in self.approved_external_providers
        )

    @property
    def has_external_provider_consent(self) -> bool:
        """Whether the user enabled an external boundary and allowlisted one."""

        return self.mode == "explicit_external" and bool(self.approved_external_providers)

    def require_provider(self, provider: str) -> None:
        """Fail closed before a named external provider receives a prompt."""

        provider = _normalise_provider(provider)
        if self.mode == "confidential_compute":
            raise PrivacyPolicyError(
                "privacy.mode=confidential_compute blocks generic cloud APIs: "
                "OpenJarvis cannot verify a provider's remote attestation or "
                "keep plaintext outside its trusted inference boundary."
            )
        if self.mode != "explicit_external":
            raise PrivacyPolicyError(
                f"External provider {provider!r} is blocked by privacy.mode="
                f"{self.mode!r}. Set privacy.mode='explicit_external' and "
                "allowlist the provider only after accepting that it processes "
                "the plaintext prompt and completion."
            )
        if provider not in self.approved_external_providers:
            raise PrivacyPolicyError(
                f"External provider {provider!r} is not in "
                "privacy.approved_external_providers."
            )

    def require_endpoint(self, provider: str, endpoint: str) -> None:
        """Allow loopback endpoints or enforce the external-provider policy."""

        if is_loopback_endpoint(endpoint):
            return
        parsed = urlsplit(endpoint)
        if self.require_tls and parsed.scheme.lower() != "https":
            raise PrivacyPolicyError(
                f"External endpoint for {provider!r} must use HTTPS, got {endpoint!r}."
            )
        self.require_provider(provider)


__all__ = [
    "PRIVACY_MODES",
    "PrivacyPolicy",
    "PrivacyPolicyError",
    "is_loopback_endpoint",
]
