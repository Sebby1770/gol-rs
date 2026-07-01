# Architecture & concept map

gol-rs started as a single-file terminal toy. It is now a small but complete
**systems** project: one dependency-free Rust engine that drives a CLI, a
hand-rolled HTTP + WebSocket streaming server, and a benchmark — plus the
container/orchestration/CI scaffolding to run it for real.

This document maps every concept from the project brief to **where it actually
lives** in the repo. Each row is honestly labelled:

- ✅ **implemented** — real, running, tested code in this repo
- 🧩 **configured** — a real, valid config/manifest you can apply
- 🗺️ **roadmap** — designed and documented, not yet wired in

## Module layout

```
src/lib.rs      the engine: Grid, Rng, patterns, step(), pack(), bench()  (+ unit tests)
src/wire.rs     from-scratch SHA-1, base64, RFC 6455 WebSocket framing     (+ RFC test vectors)
src/server.rs   the zero-dep HTTP + WebSocket + SSE server, metrics, rate limiting
src/main.rs     CLI: `gol` (animate) · `gol serve` · `gol bench`
web/index.html  canvas client: WebSocket / SSE / short-poll + RPC controls
```

The key design decision: **the rules live in one place** (`Grid::step`), so the
terminal you watch, the browser you stream to, and any future Lambda all run
byte-for-byte identical physics.

## Concept map

| Concept (from the brief) | Status | Where |
|---|---|---|
| **Web sockets** | ✅ | `src/server.rs` `/ws` + `src/wire.rs` — RFC 6455 handshake & framing by hand |
| **Long / short polling** | ✅ | `/api/stream` (SSE, long-lived) and `/api/state` (stateless short poll) |
| **RPC** | ✅ | Text control frames over `/ws`: `pause`/`resume`/`faster`/`slower`/`reseed` |
| **Rate limiting** | ✅ | Per-IP token bucket in `src/server.rs` (`allow()`), returns `429` |
| **Error logging** | ✅ | Structured JSON-lines logs to stderr (`log()` in `src/server.rs`) |
| **QPS / throughput** | ✅ | `gol bench` prints generations/sec & cell-updates/sec; `/metrics` counters |
| **Availability** | ✅🧩 | `/healthz` + `/readyz` probes; 2 replicas in compose & k8s |
| **Caching proxy** | ✅🧩 | Packed 1-bit-per-cell wire format; `Cache-Control` headers; CloudFront in roadmap |
| **Optimisation** | ✅ | Double-buffered grid, `pack()` bitset (8× smaller frames), `--release` LTO profile |
| **CI/CD** | 🧩 | `.github/workflows/ci.yml` — fmt + clippy(-D warnings) + test + build on 3 OSes |
| **deployments** | 🧩 | `.github/workflows/release.yml` builds & attaches per-platform binaries on tag |
| **Containerisation / Docker** | 🧩 | Multi-stage `Dockerfile` → distroless runtime image |
| **Docker Staging** | 🧩 | `docker-compose.yml` — 2 replicas + nginx, a local staging stack |
| **Load balancer** | 🧩 | nginx round-robin (`deploy/nginx.conf`) and k8s `Service: LoadBalancer` |
| **Kubernetes** | 🧩 | `deploy/k8s/gol.yaml` — Deployment, Service, HPA, NetworkPolicy |
| **Firewall** | 🧩 | k8s `NetworkPolicy` (ingress-only :8080); `X-Content-Type-Options` header |
| **git / github** | 🧩 | This repo + CI; branching & cherry-pick workflow in `CONTRIBUTING.md` |
| **cherry pick** | 🧩 | Backport recipe in `CONTRIBUTING.md` |
| **Serverless / Lambda** | 🗺️ | `deploy/aws/README.md` — a `gol_step` Lambda reusing `gol_rs::Grid` |
| **S3 / SQS / DynamoDB** | 🗺️ | `deploy/aws/README.md` — frame exports, async render queue, pattern registry |
| **Kafka / RabbitMQ** | 🗺️ | `deploy/aws/README.md` — generation events fanned out to consumers |
| **CLOUD** | 🗺️🧩 | ECS/Fargate + ALB + CloudFront target diagram in `deploy/aws/README.md` |
| **Encryption** | 🗺️ | TLS terminated at the LB/ingress (wss://); the client already auto-selects `wss` |
| **Database / Embedded database** | 🗺️ | Pattern registry via embedded `sled`/SQLite (local) or DynamoDB (cloud) |
| **Sharding / partitioning** | 🗺️ | Partition a huge world into tiles across replicas; shard registry by pattern id |
| **FTP** | 🗺️ | Bulk pattern-library import over SFTP (`deploy/aws/README.md`) |
| **TensorFlow** | 🗺️ | Learn/classify oscillators & spaceships from streamed generations (offline) |
| **PyCharm** | — | IDE choice; this is a Rust project, so RustRover / VS Code + rust-analyzer |

## Why hand-roll the WebSocket stack?

Pulling in `tokio` + `tungstenite` would be the production choice, but it would
also make the project a *lesson in adding dependencies* rather than a lesson in
*how the protocol works*. `src/wire.rs` implements SHA-1, base64 and RFC 6455
framing in ~250 lines, each covered by the specification's own published test
vectors. The whole crate still compiles offline with `cargo build` and nothing
else. The production path (async runtime, TLS, epoll) is the documented next
step — see the roadmap rows above.
