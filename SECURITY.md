# Security policy

## Reporting a vulnerability

Report privately through GitHub's [private vulnerability
reporting](https://github.com/nglmercer/java-path-rs/security/advisories/new).
Please do not open a public issue for an undisclosed vulnerability.

Include the crate version, the feature flags in use, your platform, and a
reproduction if you have one. Expect an acknowledgement within a week.

## Threat model

This crate downloads and unpacks archives from the network and inspects
directories that may be attacker-influenced. The properties below are treated
as security guarantees; a regression in any of them is a vulnerability, not a
bug.

**Provisioning**

* Downloaded archives are SHA-256 verified before installation. A release with
  no published checksum is refused unless the caller opts in with
  `allow_unverified(true)`.
* A checksum mismatch deletes the artifact instead of leaving it to be reused.
* Archives are only renamed into place after verification.
* Archive entry paths are validated before anything is written; entries that
  would escape the extraction root are rejected. Link targets are validated
  too — resolved relative to the link's own directory for symlinks, and to the
  archive root for hard links.
* Extraction happens in a staging directory, and the tree is validated against
  the request there, so a failed install never reaches the final location.
* Provider-supplied `file_name` and `release_name` must each be a single
  ordinary path component. Providers are not trusted.

**Discovery and inspection**

* Inspection does not execute the `java` launcher by default, so scanning a
  directory does not run code found in it. `InspectOptions::thorough()` opts
  in explicitly.
* Subprocesses are spawned with `std::process::Command` and separate
  arguments; no string-concatenated shell commands.
* `#![forbid(unsafe_code)]` crate-wide.

## Scope

Not vulnerabilities: the contents of a JDK you asked to install, defects in
the upstream Adoptium API, and the behaviour of a JDK already installed on
the machine.
