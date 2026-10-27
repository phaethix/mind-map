# Contributing

Thanks for taking the time to help. This document covers how to report a problem and how to get a change merged.

## Reporting a problem

- Use the issue forms. Blank issues are disabled on purpose: the fields in the bug report and feature request templates are the minimum needed to act on a report.
- One problem per issue. If you hit two unrelated bugs, file two issues.
- Fill in the version and the platform. Without them the report cannot be reproduced.
- Do not comment `+1` or `same here` — use the reaction on the issue instead. Comments are for new information.
- Check the [README](README.md) and the [latest release](https://github.com/phaethix/mind-map/releases) before filing: the problem may already be fixed.

Issues that stay incomplete for two weeks may be closed. They can be reopened once the missing details arrive.

## Labels

Every issue carries one type label, usually one or two area labels, and `needs triage` until it has been looked at.

- **Type** — `bug`, `enhancement`, `question`, `documentation`. The issue forms add `bug` or `enhancement` automatically. `duplicate`, `invalid` and `wontfix` are closing states and are only applied by the maintainer.
- **Area** — `area: editor`, `area: desktop`, `area: web`, `area: import-export`, `area: ai`, `area: build`. Applied during triage so issues can be filtered by the part of the project they touch.
- **Triage** — `needs triage` is added by the forms and removed once the issue has a type and an area.
- **Contribution** — `good first issue` and `help wanted` mark work that is open to outside contributors. `accessibility` marks a barrier for people with disabilities.

## Sending a pull request

1. **Open or find an issue first.** Anything beyond a small fix should have an issue that describes the problem, so the approach can be agreed before code is written.
2. **Fork and branch from `main`.** Keep the branch focused on one concern.
3. **Keep the diff reviewable.** Unrelated refactors, formatting sweeps and dependency bumps belong in their own pull request.
4. **Write conventional commits.** The history uses `feat:`, `fix:`, `perf:`, `docs:`, `style:`, `refactor:`, `test:`, `build:`, `ci:` and `chore:` — lowercase, imperative, no trailing period.
5. **Run the checks before pushing:**

   ```bash
   bun x prettier --check .
   cargo fmt --manifest-path desktop/src-tauri/Cargo.toml --check
   bun web/src/utils/fileLibrary.test.cjs
   bun web/src/utils/mermaidMindmap.test.cjs
   bun simple-mind-map/test/xmind-associative-line.test.cjs
   cd web && bun install --frozen-lockfile && npm run build
   ```

   The same checks run in CI on every pull request.

6. **Update the documentation.** Behaviour changes belong in the README or in a code comment next to the change.
7. **Describe the verification.** State which platform you tested on and what you ran. CI does not cover the desktop shell on macOS or Windows.

### Pull request titles

The merged title becomes the commit subject on `main`, so it follows the same rule as commits: `type: lowercase imperative summary`, one of `feat`, `fix`, `perf`, `docs`, `style`, `refactor`, `test`, `build`, `ci`, `chore`, `revert`. No trailing period. A CI check (`title`) rejects anything else, and it is required to merge.

## Merging

- Every pull request needs the `title`, `format` and `build` checks to pass, and every review conversation resolved, before it can be merged.
- Pull requests are squashed into a single commit whose subject is the pull request title. Keep the title accurate: it becomes the commit subject and the line the release notes list. The body stays on the pull request for reviewers.
- Force pushes and branch deletions on `main` are disabled.

## Build notes

- **Node 20.11+ and Bun 1.4+** are required. Dependencies are installed with `bun install --frozen-lockfile`, and `bun.lock` (root, `web/`, `desktop/`) is the only lockfile, while the build scripts still run on Node. `scripts/bootstrap.sh` installs both automatically on macOS and Linux; on Windows install them yourself and run `make` again.
- **Line endings.** `.gitattributes` pins the files the shell parses to LF. Windows working trees created before that file existed need one re-checkout or a fresh clone.
- **Desktop builds** are macOS-only today: `make` produces `Mind.app` in `~/Applications`.

## Review

- Expect questions and change requests. Small, focused pull requests get merged faster.
- A pull request that has been waiting on the author for a month may be closed. It can always be reopened.
- By contributing you agree that your work is released under the licence in [LICENSE](LICENSE).
