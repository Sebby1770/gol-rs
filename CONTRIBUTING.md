# Contributing to gol-rs

Thanks for hacking on gol-rs! It stays intentionally small and
**dependency-free** — please keep new features in the standard library unless
there's a very strong reason not to.

## Local checks (what CI runs)

```sh
cargo fmt --all --check          # formatting
cargo clippy --all-targets -- -D warnings
cargo test --all                 # unit tests, incl. RFC wire vectors
cargo build --release
```

`make check` runs fmt + clippy + test in one go.

## Branch & PR workflow

1. Branch off `main`: `git switch -c feat/short-description`.
2. Keep commits focused; write present-tense messages ("add SSE endpoint").
3. Push and open a PR. CI must be green before merge.
4. Prefer a linear history — rebase on `main` rather than merging it in.

## Cherry-picking a fix onto a release branch

When a bug fix on `main` needs to go onto an older release line:

```sh
git switch release/0.2
git cherry-pick <commit-sha>     # replay just that commit
# resolve conflicts if any, then:
git push
```

Use `git cherry-pick -x <sha>` to record the source commit in the message —
handy for tracing a backport later.

## Adding an endpoint or command

- Engine logic → `src/lib.rs` (add a unit test next to it).
- Wire/protocol code → `src/wire.rs` (add a test vector from the spec).
- HTTP/WebSocket routing → `src/server.rs`.
- Update `ARCHITECTURE.md`'s concept map and `CHANGELOG.md`'s *Unreleased*
  section in the same PR.
