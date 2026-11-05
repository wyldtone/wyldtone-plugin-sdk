# Security

Please report security issues privately to the repository maintainers using
GitHub's private vulnerability reporting feature (once enabled). Do not post
working exploits or sensitive examples in public issues.

The reference host is for **trusted, local plugin testing**. The runtime's
current WASI configuration is minimal, but this does not make it a production
multi-tenant sandbox. Do not run untrusted uploaded WAPs on the marketplace web
server. Validate package entry sizes, set time/memory limits and use an isolated
unprivileged process before implementing any server-side inspection.
