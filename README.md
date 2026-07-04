# gol-rs

Conway's Game of Life in Rust — **in your terminal, in your browser, or on your servers.**
One dependency-free engine, three ways to run it.

[![CI](https://github.com/Sebby1770/gol-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/Sebby1770/gol-rs/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/rust-edition%202024-orange)
![deps](https://img.shields.io/badge/dependencies-0-brightgreen)
![license](https://img.shields.io/badge/license-MIT-blue)

```
gol-rs  gen     0  pop   384  grid 60x30  (Ctrl-C to quit)
  ████  ██  ██████      ██  ████      ██████  ██  ████
  ██  ████      ██████  ██████      ██  ██████  ████
```

## What's new in 0.2

gol-rs grew from a terminal toy into a compact **systems** project — and it still
has **zero third-party dependencies**. The HTTP + WebSocket server (SHA-1,
base64, RFC 6455 framing) is all hand-rolled in `src/wire.rs`, so the whole thing
builds offline with nothing but the standard library.

- 🖥️ **`gol`** — the original ANSI terminal animation (unchanged default).
- 🌐 **`gol serve`** — stream one shared simulation to browsers over **WebSocket**,
  with **SSE** and **short-poll** fallbacks, an **RPC** control channel,
  **/healthz** + **/readyz** probes, **/metrics**, and **per-IP rate limiting**.
- ⏱️ **`gol bench`** — measure raw **throughput** (generations/sec, cell-updates/sec).

See **[ARCHITECTURE.md](ARCHITECTURE.md)** for how each backend concept maps to
real code, and **[CHANGELOG.md](CHANGELOG.md)** for the full 0.2 list.

## Quick start

```sh
# 1. Terminal animation
cargo run --release -- --pattern gosper --width 80 --height 25 --delay 60

# 2. Live in the browser  (then open http://127.0.0.1:8080)
cargo run --release -- serve --pattern gosper

# 3. Benchmark the engine
cargo run --release -- bench --width 512 --height 512 --gens 1000
```

Or with the `Makefile`: `make release`, `make test`, `make demo`.

## The web server

```sh
gol serve --addr 0.0.0.0:8080 --pattern gosper --width 120 --height 60
```

| Endpoint | What it does | Concept |
| --- | --- | --- |
| `GET /` | Canvas client (switch transport live) | — |
| `GET /ws` | Full-duplex WebSocket stream + control | **WebSockets, RPC** |
| `GET /api/stream` | Server-Sent Events stream | **long polling** |
| `GET /api/state` | One JSON snapshot of the board | **short polling** |
| `GET /healthz`, `/readyz` | Liveness / readiness | **availability** |
| `GET /metrics` | Prometheus counters | **QPS / throughput** |

Frames go on the wire as a packed 1-bit-per-cell bitset (base64) — a 120×60 board
is ~900 bytes, not 7 KB. The browser unpacks it straight onto a `<canvas>`.

## Run it anywhere

```sh
docker compose up --build       # 2 replicas behind an nginx load balancer → :8080
kubectl apply -f deploy/k8s/gol.yaml   # Deployment + Service + HPA + NetworkPolicy
```

The [AWS serverless roadmap](deploy/aws/README.md) sketches a `gol_step` Lambda,
S3 frame exports, an SQS render queue, a DynamoDB pattern registry and Kafka
event fan-out — all reusing this same engine.

## CLI reference

```sh
gol [OPTIONS]                 # animate (default)
gol serve [SERVE OPTIONS]     # stream over HTTP + WebSocket
gol bench [BENCH OPTIONS]     # throughput benchmark
gol --help
```

| Flag | Default | Description |
| --- | --- | --- |
| `-W`, `--width N` | `60` | grid width |
| `-H`, `--height N` | `30` | grid height |
| `-g`, `--gens N` | `500` | generations to run |
| `-d`, `--delay MS` | `80` | per-frame delay |
| `-s`, `--seed N` | time-based | RNG seed (random pattern) |
| `-p`, `--pattern NAME` | `random` | `random` \| `glider` \| `pulsar` \| `gosper` |
| `--density F` | `0.25` | 0–1, for `random` |

For `serve`, add `-a/--addr HOST:PORT`.

## Install

```sh
./install.sh                          # /usr/local/bin (sudo if needed)
PREFIX=$HOME/.local ./install.sh      # user-local
make install
```

## Development

`cargo test` runs the engine tests **and** the RFC test vectors for SHA-1,
base64 and the WebSocket handshake. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT — see [LICENSE](LICENSE).
