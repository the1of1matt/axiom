# Security Policy

## Supported versions

Security fixes are applied on a best-effort basis to the latest `main` branch and tagged releases.

## Reporting a vulnerability

Please email the maintainer privately (see the GitHub profile for the repository owner) or open a **private** security advisory on GitHub if available.

Do **not** file public issues that include exploit details for unfixed vulnerabilities.

Include:

- Axiom version (`axiom --version`)
- OS and architecture
- Steps to reproduce
- Impact assessment

## Scope notes

Axiom runs untrusted project scripts only after an explicit trust prompt. Treat trust as granting the project the ability to run package manager lifecycle scripts and start commands on your machine.
