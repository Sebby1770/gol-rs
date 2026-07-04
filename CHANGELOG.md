# Changelog

All notable changes to gol-rs are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project
adheres to [Semantic Versioning](https://semver.org/).

## [0.4.0] — 2026-07-04

### Added
- **Draw on the board.** Click or drag on the canvas to paint live cells; each
  stroke is sent as a `cell X Y` RPC frame over the WebSocket into the one
  shared world, so **every connected viewer sees what you draw**. Pointer
  events support mouse and touch; dragging deduplicates repeat cells; painting
  works while paused (sketch a pattern, then hit resume). The wire syntax is a
  pure parser (`parse_cell_command`) with unit tests; out-of-bounds and
  malformed input are ignored server-side.
  Verified live: painted a blinker into an empty paused world over a raw
  WebSocket, confirmed the exact bits via `/api/state`, resumed, and watched
  it oscillate.

## [0.3.1] — 2026-07-04

### Added
- **Live "server ops" panel in the web client.** The page now polls the
  Prometheus counters at `/metrics` every 2s and renders the backend's vitals
  right under the board: **generations/sec (throughput)**, **HTTP req/sec
  (QPS)**, frames pushed/sec, active connections, total WebSocket upgrades,
  **rate-limited (429) count**, and uptime — rates computed client-side from
  counter deltas. Verified against a live server: the panel's parser reads all
  exported metrics and its computed throughput matched the configured tick
  rate (23.5 gens/sec at a 40ms delay).

## [0.3.0] — 2026-07-03

### Performance
- **~3.2× faster engine.** `Grid::step()` now binds each row and its two
  neighbours as slices, so the interior of the board needs no wrap arithmetic
  at all — only the first and last column of each row pay for the torus.
  Measured with `gol bench` on a 512×512 board, 500 generations, release build:
  **790 → ~2,510 generations/sec** (205M → ~660M cell-updates/sec).
  A new `fast_step_matches_reference_on_random_soup` test proves the optimised
  path is cell-for-cell identical to the naive `count_neighbors` rule across
  25 generations of a dense random soup (exercising every wrap edge).

### Fixed
- The per-IP rate-limiter map now evicts buckets idle for over a minute once it
  grows past 1,024 entries, so a churn of distinct client IPs can no longer
  grow server memory without bound.

## [0.2.0] — 2026-07-02

The "off the terminal and onto the network" release. gol-rs is now a small
systems project: one shared engine driving a CLI, a streaming server, and a
benchmark, with real CI/CD and container/orchestration scaffolding.

### Added
- **Library crate** (`src/lib.rs`): the engine (grid, RNG, patterns, `step`,
  `pack`, `bench`) is now reusable by every front-end and unit-tested directly.
- **`gol serve`** — a zero-dependency HTTP + WebSocket server that streams a
  shared simulation to any number of browser clients:
  - **WebSocket** endpoint `/ws` with an RFC 6455 handshake and framing
    implemented from scratch (`src/wire.rs`: hand-rolled SHA-1 + base64).
  - **Server-Sent Events** at `/api/stream` (long-polling / streaming fallback).
  - **Short-poll** JSON snapshot at `/api/state`.
  - **RPC** control channel: `pause` / `resume` / `faster` / `slower` / `reseed`
    (and pattern names) sent as WebSocket text frames.
  - **`/healthz` + `/readyz`** liveness/readiness probes.
  - **`/metrics`** Prometheus-format counters (requests, connections, frames,
    generations, rate-limited, uptime).
  - **Per-IP token-bucket rate limiting** (returns `429`).
  - **Structured JSON-lines logging** to stderr.
- **`gol bench`** — headless throughput benchmark reporting generations/sec and
  millions of cell-updates/sec.
- **Browser client** (`web/index.html`) — canvas renderer with a live
  transport switch (WebSocket / SSE / short poll) and control buttons.
- **CI** (`.github/workflows/ci.yml`): rustfmt, clippy `-D warnings`, tests, and
  release builds across Ubuntu, macOS and Windows.
- **Release automation** (`.github/workflows/release.yml`): tagged builds attach
  per-platform binaries to the GitHub release.
- **Containers**: multi-stage `Dockerfile` (distroless runtime) and a
  `docker-compose.yml` that runs two replicas behind an nginx load balancer.
- **Kubernetes** manifests (`deploy/k8s/gol.yaml`): Deployment, LoadBalancer
  Service, HorizontalPodAutoscaler, and a NetworkPolicy.
- **AWS serverless roadmap** (`deploy/aws/README.md`) with an illustrative
  `gol_step` Lambda that reuses the shared engine.
- **`ARCHITECTURE.md`** mapping every backend concept to where it lives, and
  **`CONTRIBUTING.md`** documenting the branch/PR/cherry-pick workflow.
- New unit tests for the bitset packing and the wire primitives (SHA-1, base64,
  WebSocket accept key and frame parsing — all against published test vectors).

### Changed
- Split the former single `src/main.rs` into `lib.rs` + `main.rs`; the terminal
  animation is unchanged and remains the default `gol` invocation.
- Bumped edition-2024 compatibility (renamed the reserved `gen` identifier).
- README rewritten around the three run modes.

### Notes
- The crate is still **dependency-free** — the entire network stack is std-only.

## [0.1.0] — 2026-05-06

### Added
- Initial release: Conway's Game of Life in the terminal, toroidal grid,
  `random` / `glider` / `pulsar` / `gosper` patterns, deterministic seeding,
  ANSI renderer, Makefile, install script, and unit tests.

[0.3.0]: https://github.com/Sebby1770/gol-rs/releases/tag/v0.3.0
[0.2.0]: https://github.com/Sebby1770/gol-rs/releases/tag/v0.2.0
[0.1.0]: https://github.com/Sebby1770/gol-rs/releases/tag/v0.1.0
