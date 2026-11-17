# Security policy

## Reporting a vulnerability

Please report suspected security vulnerabilities through
[GitHub's private vulnerability reporting](https://github.com/zerossB/metronomo-rust/security/advisories/new).
Do not disclose exploit details, credentials, or sensitive data in public issues
or pull requests while a report is being investigated.

Include the affected version or commit, operating system, reproduction steps,
expected impact, and a minimal proof of concept when possible. Remove personal
settings and credentials from attachments. Test against systems and audio devices
you own or have permission to use.

The maintainer will investigate and coordinate any fix and disclosure through the
private report. This project is maintained on a best-effort basis and does not
promise a fixed response time.

## Supported versions

Security fixes target the latest released version and the `main` branch. Older
versions may require an upgrade. The application is in early development; see the
[validation report](docs/validation/report.md) for tested platforms and limitations.

## Release integrity

Download installers from this repository's
[GitHub Releases](https://github.com/zerossB/metronomo-rust/releases) and verify
their `SHA256SUMS`. Newly published releases use immutable assets and tags;
corrections are distributed as a new version. Checksums detect damaged or
substituted downloads but do not establish the safety of the application itself.

## Dependency updates

Dependabot proposes dependency and GitHub Actions updates through pull requests.
Updates require review and passing checks; they are not merged automatically.
Third-party license supplements must be reviewed when locked dependencies change.
