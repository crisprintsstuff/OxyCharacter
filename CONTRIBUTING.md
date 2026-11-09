# Contributing to OxyCharacter

Thanks for helping. OxyCharacter is a clean-room, open-source character
creator in Rust (MIT OR Apache-2.0).

## Before you start

- Read [docs/ROADMAP.md](docs/ROADMAP.md) for what's done and what's next, and
  [docs/PLAN.md](docs/PLAN.md) for the design.
- Read [docs/CLEAN_ROOM.md](docs/CLEAN_ROOM.md). Contributions must not use
  material from commercial character creators (code, formats, assets, UI) or
  copyleft code. Log external sources you consulted.
- Larger or user-facing design changes start as an ADR in `docs/adr/` (copy
  the style of the existing ones) or an issue for discussion.

## Setup

```bash
# Rust: rustup picks up rust-toolchain.toml automatically
tools/fetch-assets.sh          # downloads CC0 data (~130 MB) and builds the 50 MB bundle
cargo run --release -p oxy-app
```

Linux needs a Vulkan or OpenGL driver; without a GPU see
[docs/TESTING.md](docs/TESTING.md) for software rendering.

## Making a change

1. Branch from `main`; open a pull request when ready.
2. Run the checks in [docs/TESTING.md](docs/TESTING.md) §1 (and §2–§5 when
   your change touches those areas). Look at a screenshot for UI changes.
3. Add tests with the change (see AGENTS.md "Workflow").
4. Update docs in the same PR: ROADMAP (status + "Last updated"), CHANGELOG
   (*Unreleased*), README for user-visible behaviour.
5. Sign off your commits (`git commit -s`, [Developer Certificate of
   Origin](https://developercertificate.org/)) and explain *why* in the
   message body.
6. In the PR description, say what you verified and how, and what you didn't.

## Assets

Only original, CC0 or CC-BY content, each with a row in
[assets/LICENSES.md](assets/LICENSES.md). Large or third-party data is fetched
by a script, not committed.

## Cutting a release

Releases are git tags. [`.github/workflows/release.yml`](.github/workflows/release.yml)
runs when a tag matching `v*` is pushed: it builds the packages and publishes
a GitHub Release. It does not run on an ordinary push. A manual run
(`workflow_dispatch`, from the Actions tab) builds the same artifacts, uploads
them to that run, and does not publish.

1. Bump `version` in the workspace [`Cargo.toml`](Cargo.toml). `0.0.1-alpha`
   is a valid Cargo version. Update `Cargo.lock` by building or testing.
2. Move the *Unreleased* section of [CHANGELOG.md](CHANGELOG.md) to
   `## [x.y.z] — YYYY-MM-DD` and leave a new empty *Unreleased* section.
3. Write `docs/release-notes/vX.Y.Z.md`. The workflow uses that file, named
   exactly like the tag, as the GitHub Release body. Include download and
   install steps and the hero screenshot.
4. Update [docs/ROADMAP.md](docs/ROADMAP.md) (version and "Last updated").
5. Merge to `main`.
6. Tag the merge commit and push **only** the tag:

   ```sh
   git tag -a vX.Y.Z -m "OxyCharacter vX.Y.Z"
   git push origin vX.Y.Z
   ```

   Do not force-push a tag. Tags whose names contain `-alpha`, `-beta` or
   `-rc` are published as pre-releases.

The character bundle shipped in the packages is built by
`tools/fetch-assets.sh` from the commit in
[`tools/release/makehuman.rev`](tools/release/makehuman.rev). Bump that file
when a release should pick up newer CC0 data, and record the revision in
[assets/LICENSES.md](assets/LICENSES.md).

macOS Developer ID signing and notarization are optional. Set these
repository secrets to enable them; leave them unset and the workflow ad-hoc
signs the app and skips notarization:

| Secret | Purpose |
|---|---|
| `APPLE_CERTIFICATE_P12_BASE64` | base64 `.p12` of the Developer ID Application certificate |
| `APPLE_CERTIFICATE_PASSWORD` | password for that `.p12` |
| `APPLE_SIGNING_IDENTITY` | optional; defaults to the identity inside the `.p12` |
| `APPLE_NOTARIZE_APPLE_ID` | Apple ID email, for notarization |
| `APPLE_NOTARIZE_PASSWORD` | app-specific password |
| `APPLE_TEAM_ID` | 10-character Team ID |

Unsigned (ad-hoc) builds need the Gatekeeper workaround in the release notes
and in `tools/release/INSTALL.txt`.

## Licence

By contributing you agree that your work is licensed under MIT OR Apache-2.0,
like the rest of the project ([LICENSE-MIT](LICENSE-MIT),
[LICENSE-APACHE](LICENSE-APACHE)).
