# WAP signatures and licensing — compatibility proposal (not implemented)

**Status: experimental design proposal.** This document reserves a direction for
future interoperability; it is **not** a DRM implementation, a supported ABI, a
security guarantee, or a mandatory requirement for beta WAP plugins. Existing
unsigned WAP packages remain valid under the current WAP format v1.

## Two independent trust relationships

1. **Publisher signing key:** The plugin publisher signs an exact, canonical
   inventory of the immutable WAP payload. The publisher's private key stays
   with the publisher. Hosts can verify the package against a trusted
   publisher public key. A cryptographically valid signature alone does not
   establish that the publisher is trusted.
2. **License signing key:** A licensing authority signs a license grant. Its
   private key stays on the issuing service (ideally in a restricted signing
   service/HSM), never inside the WAP or host. The plugin embeds or otherwise
   securely obtains the corresponding trusted **public** key and verifies
   license claims independently of host enforcement.

These key pairs MUST be distinct, with separately managed rotation and
revocation. The host may make license bytes available to the plugin; it should
not be the ultimate authority for whether those bytes grant access.

## Optional manifest declaration (proposal)

Future WAP manifests MAY contain an optional `licensing` object conforming
to [the proposed JSON schema](../schemas/licensing-declaration-v1.schema.json).
This does not change the core WAP `format_version` or require an existing
plugin to implement licensing. Example (illustrative, not currently emitted):

```json
{
  "licensing": {
    "schema_version": 1,
    "license_required": true,
    "license_key_ids": ["publisher-a-license-2027"],
    "license_delivery": "host_provided",
    "license_format": "jws-compact-eddsa-v1"
  }
}
```

The declared key IDs are identifiers, **not trusted keys**. A plugin must
ship with its own trusted license verification keys (or an authenticated
means to obtain them); accepting a public key supplied inside an untrusted
license would let an attacker sign their own licenses.

The licensing declaration is a *hint for integration and capabilities*, not
proof of enforcement. An absent declaration means `unspecified / legacy`,
not necessarily "licensed" or "unlicensed".

## Publisher signature envelope (future work)

- Define a deterministic UTF-8 manifest of file paths, lengths and SHA-256
  digests covering **every immutable file** shipped in the WAP (including
  `plugin.wasm`, `manifest.json`, artwork and static resources).
- Canonicalize both path spelling and ordering; reject duplicate ZIP paths,
  unsafe paths, path aliases, unexpected unsigned entries, and duplicate
  signature metadata. Signature metadata itself must be excluded in a
  precisely documented way to avoid self-reference.
- Sign the canonical inventory with a specified algorithm and encoding
  (Ed25519 is a candidate); version the signed envelope and bind it to the
  intended WAP identity and plugin version.
- Establish explicit publisher-key trust and rotation policies outside the
  package. Package verification requires a trusted publisher key; a key
  bundled alongside a package cannot authenticate itself.
- Do not simply sign the ZIP archive bytes without specifying a reproducible
  byte representation: ZIP metadata can vary between repackaging operations.
- Preserve independently mutable files (for example, license grants) **outside**
  the signed static WAP payload.

Exact wire format, filename and signing commands are NOT finalized.

## Signed license grant (future work)

A candidate is an Ed25519-signed, versioned, size-bounded token, such as a
strictly specified JWS Compact Serialization with EdDSA. It should bind at
least to:

- grant format version, issuer, signing-key identifier and intended audience;
- exact publisher/product/plugin ID, and any applicable version constraints;
- license ID, entitlements, and explicit time validity (`not_before`,
  `expires_at` or a deliberate perpetual entitlement);
- optional user/device/installation binding where required by product policy.

Treat all license bytes and claims received from a host as **untrusted** until
the plugin verifies the signature using a trusted license public key, checks
algorithm/key-ID allowlists, validates all critical fields, and applies its
own entitlement and expiry policy. Never accept an arbitrary algorithm or
algorithm `none` from the token header.

A plugin cannot safely assume that host time is trustworthy; offline expiry,
clock rollback resistance, activation, revocation and renewal require
separate product decisions. Offline clients cannot guarantee immediate
revocation. Avoid network I/O or heavy signature verification on the real-time
audio thread.

## Third-party hosts and optional license ABI v1

The **proposed optional ABI has now been defined** in
[`abi/include/wasm_plugin.h`](../abi/include/wasm_plugin.h).
It uses four **optional WASM exports** rather than mandatory host imports,
so existing WAPs can continue to instantiate in existing hosts.

A licensing-aware plugin exports all four functions:
- `plugin_license_abi_version()` returns `1`;
- `plugin_license_requested()` returns `1` when it wants the host to
  provide the signed license bytes;
- `plugin_license_prepare_load(size)` reserves a buffer in WASM memory;
- `plugin_license_commit_load(size)` tells the plugin that the bytes are
  ready to validate.

The plugin can also request delivery by setting
`WASM_PLUGIN_EVENT_LICENSE_REQUESTED` (`0x04`) in the existing optional
pending-events bitmask. The host must **never** poll or deliver on the
real-time audio thread. Transfer is limited to **65,536 bytes** and the
host must bounds-check all WASM memory access.

For the intended single-file installation, a future Marketplace may embed
`license.jwt` in the WAP ZIP **after** publisher signing. The signature
envelope must explicitly exclude this one mutable license entry, while
authenticating all other allowed static files. A license-aware host extracts
these bytes, passes them through the ABI only when requested, and does not
decide whether the grant is valid. The plugin verifies the license signature
and entitlement itself. The license bytes must be cryptographically bound
to the specific plugin identity to prevent interchange.

Absence of the ABI means an **older/unlicensed plugin**, not an error.
Incomplete exports from a plugin that attempts to implement this ABI are
invalid. An older host is not required to provide licenses; a licensed
plugin must decide how to handle that (demo mode, refuse activation, etc.).

**Implementation status:** The ABI declarations and protocol are now
documented but the SDK runtime, WAP packager, reference host, compiler
adapters and plugin implementations are **not yet wired up to use them**.
A small independent WASM ABI compatibility fixture exists, but there is no
end-to-end integration test with a licensed WAP; no license verification
library is shipped. Thus the current beta keeps working exactly as before.
Host-side integration and end-to-end compatibility testing must precede
declaring the extension stable.

## Security boundary and realistic expectations

This design provides cryptographic integrity and license authenticity when
implemented correctly, but **does not make a client-side plugin unpatchable**.
In an untrusted third-party host, an attacker can modify or substitute WASM,
patch the verification code, or intercept host calls. A host that voluntarily
verifies publisher signatures can detect changes; another host may choose
not to. There is no absolute DRM enforcement guarantee without a trusted
execution environment.

For the beta, keep the legacy unsigned and unlicensed path working. Before
shipping DRM, finalize threat model, key provisioning, canonical signing
format, cryptographic implementation, test vectors and license host ABI.
