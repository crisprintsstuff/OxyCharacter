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

## Licence

By contributing you agree that your work is licensed under MIT OR Apache-2.0,
like the rest of the project ([LICENSE-MIT](LICENSE-MIT),
[LICENSE-APACHE](LICENSE-APACHE)).
