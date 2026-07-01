# gol-rs on AWS — serverless roadmap

The `gol serve` binary is a stateful, long-lived streaming server, so it belongs
on a container platform (ECS/Fargate or the [k8s manifests](../k8s/gol.yaml)).
But several capabilities are a natural fit for **serverless** and are sketched
here as a concrete, honest roadmap — real service names, real event flow, not
yet wired into the crate.

## Target architecture

```
                   ┌─────────────┐   pattern PNGs / RLE files
   browser ──HTTP──▶ ALB / CloudFront ──▶ ECS Fargate (gol serve, N tasks)
                   └─────────────┘             │
                                               │ publishes "generation" events
                                               ▼
                                        Kafka (MSK) topic  ──▶ analytics consumers
                                               │
   "compute N steps" ─── API GW ──▶ Lambda (gol_step) ──▶ result
                                               │
                             ┌─────────────────┼─────────────────┐
                             ▼                 ▼                 ▼
                        DynamoDB           S3 bucket           SQS queue
                    (shared pattern    (rendered frame     (async render
                     registry, TTL)     GIF/PNG exports)      job backlog)
```

| AWS service | Role in gol-rs | Photo concept |
|-------------|----------------|---------------|
| **Lambda** | Stateless "compute N generations of this board" function | Serverless, Lambda |
| **DynamoDB** | Shared pattern registry / high-score board (single-digit-ms reads) | DynamoDB, sharding, partitioning |
| **S3** | Store exported frames, GIFs and pattern libraries | S3 |
| **SQS** | Backlog of async GIF-render jobs, decoupled from the web tier | SQS |
| **MSK (Kafka)** | Fan-out of generation events to analytics/consumers | Kafka / RabbitMQ |
| **ALB + Fargate** | Load-balanced, auto-scaling container tier | Load balancer, deployments, containerisation |
| **CloudFront** | Edge cache / caching proxy for the static client | Caching proxy, availability |

## Example Lambda handler (illustrative — not built by `cargo build`)

A pure, stateless step function reuses the same engine that powers the CLI and
the WebSocket server, so the physics are identical everywhere:

```rust
// Cargo.toml (separate crate): lambda_runtime = "0.13", gol-rs = { path = ".." }
use gol_rs::{Grid, Rng, seed_pattern};
use lambda_runtime::{service_fn, Error, LambdaEvent};
use serde_json::{json, Value};

async fn handler(event: LambdaEvent<Value>) -> Result<Value, Error> {
    let p = event.payload;
    let (w, h) = (p["w"].as_u64().unwrap_or(64) as usize,
                  p["h"].as_u64().unwrap_or(64) as usize);
    let steps = p["steps"].as_u64().unwrap_or(100);
    let pattern = p["pattern"].as_str().unwrap_or("random");

    let mut grid = Grid::new(w, h);
    let mut rng = Rng::new(p["seed"].as_u64().unwrap_or(1));
    seed_pattern(&mut grid, pattern, &mut rng, 0.3).map_err(Error::from)?;
    for _ in 0..steps { grid.step(); }

    Ok(json!({ "population": grid.population(), "generation": steps }))
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(service_fn(handler)).await
}
```

Deploy with [`cargo lambda`](https://www.cargo-lambda.info/):

```sh
cargo lambda build --release --arm64
cargo lambda deploy gol-step
```

Because the engine lives in `src/lib.rs`, the CLI, the streaming server and this
Lambda all share one battle-tested `Grid::step()` — no rules drift between tiers.
