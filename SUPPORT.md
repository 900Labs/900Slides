# Support

## Getting help

- **Bug reports and feature requests:** [GitHub Issues](https://github.com/900Labs/900Slides/issues)
- **Security vulnerabilities:** See [SECURITY.md](SECURITY.md) — do not open a
  public issue. Email **security@900labs.com**.

## Before you file an issue

1. Update to the latest version on the `main` branch.
2. Search existing issues to avoid duplicates.
3. Run the quality gate and confirm it passes on your machine:

   ```bash
   ./scripts/verify-local.sh
   ```

4. If you are reporting a file-format issue (PPTX round-trip, import, or
   export), attach a **sanitized** fixture. Do not attach files containing
   personal or confidential data. Invent placeholder content that reproduces
   the problem.

## What to include in a bug report

- 900Slides version (from the About view or `git describe`).
- Operating system and version.
- Rust and Node.js versions.
- Steps to reproduce.
- Expected behavior and actual behavior.
- Console output or error messages.
- A sanitized `.pptx` fixture if the issue involves file handling.

## Building from source

See the [README](README.md) for build instructions and prerequisites.

## Community

900Slides is part of the [900 Labs](https://www.900labs.com/) family of
open-source, local-first tools. Visit [900labs.com](https://www.900labs.com/)
for other projects.
