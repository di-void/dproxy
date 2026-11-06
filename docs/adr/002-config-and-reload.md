# ADR-002: Config format and reload model

- **Status:** Proposed
- **Date:** 2026-11-02

## Context

Operators edit one config file that defines listeners, routes, clusters, endpoints and health checks. SLO S6 requires a reload to drop **zero** requests at 10k rps. A bad config must never take the proxy down. Service discovery is out of scope, so the file is the only source of truth.

## Options considered

### Format

| Criterion | TOML | YAML | JSON | CUE / Dhall |
|---|---|---|---|---|
| Comments | Yes | Yes | No | Yes |
| Footguns | Few | Implicit typing (`no` parses as `false`), indentation | Trailing commas, no comments | Extra toolchain |
| Rust support | `serde` + `toml`, first class | `serde_yaml` is unmaintained; forks exist | `serde_json` | Weak |
| Deep nesting | Awkward with arrays of tables | Good | Good | Good |
| Familiar to ops | Medium | High (k8s, Envoy) | High | Low |

### Reload model

| Option | Pros | Cons |
|---|---|---|
| **A. Atomic snapshot swap in-process** (`ArcSwap<Config>`) | Lock-free reads, in-flight requests finish on the old config, no socket handoff | Changing a listener's bind address needs extra work |
| B. Restart the process with socket handoff (`SO_REUSEPORT` / fd passing) | Can change anything | Complex, platform-specific, harder to test |
| C. `RwLock<Config>` | Simple | A lock on the hot path; writers can stall readers and hurt p99 |

## Decision

**Format: TOML**, parsed with `serde` using `#[serde(deny_unknown_fields)]` so typos fail loudly.

**Reload: option A, atomic snapshot swap.**

1. **Triggers:** `POST /admin/reload` on the admin listener (all platforms), plus `SIGHUP` on Unix. File watching is deliberately left out, because editors write partial files.
2. **Pipeline:** `read → parse → validate → build → swap`. Validation is semantic: names are unique, every route points to an existing cluster, addresses parse, timeouts are > 0. Build compiles the route matchers and the load balancer state.
3. **On failure:** the old config stays live. The proxy logs the error, increments `dproxy_config_reload_failures_total`, and the admin endpoint returns `400` with the reason.
4. **On success:** a single `ArcSwap::store`. Each request calls `load_full()` once at the start and keeps that `Arc`, so in-flight requests finish on the config they started with.
5. **Versioning:** each snapshot carries a `generation: u64` and a content hash. Both are exposed via `GET /admin/config` and the `dproxy_config_generation` metric.
6. **Health state** lives outside the snapshot and is keyed by endpoint address, so unchanged endpoints keep their state across reloads. New endpoints receive no traffic until their first health check passes.
7. **Listeners:** route, cluster, endpoint and timeout changes hot-reload. Changes to a listener's address or port need a restart in v1. Swapping TLS certificates is supported via a rustls `ResolvesServerCert` that reads from the snapshot.

### Sample config

```toml
[admin]
bind = "127.0.0.1:9901"

[[listeners]]
name = "public"
bind = "0.0.0.0:8443"
tls = { cert = "certs/server.crt", key = "certs/server.key" }  # omit for plaintext / h2c

[[routes]]
listener = "public"
host = "api.example.com"     # "*" matches any host
path_prefix = "/"
cluster = "api"

[[clusters]]
name = "api"
lb = "p2c"                   # "round_robin" | "p2c"
endpoints = ["10.0.0.1:8080", "10.0.0.2:8080", "10.0.0.3:8080"]
timeouts = { connect_ms = 250, request_ms = 5000, idle_ms = 60000 }
retry = { max = 1, budget_percent = 20, on = ["connect-failure", "503"] }

[clusters.health_check]
path = "/healthz"
interval_ms = 1000
timeout_ms = 500
healthy_threshold = 2        # consecutive passes before an endpoint is marked up
unhealthy_threshold = 2      # consecutive fails before it is marked down (meets SLO S5: ≤ 3 s)

[clusters.outlier]
consecutive_5xx = 5
ejection_ms = 30000
```

## Consequences

- **Good:** reloads cost nothing on the hot path, a bad config can never go live, and the reload path is easy to test (unit-test `validate()` and `build()`).
- **Bad / accepted costs:** listener rebinds need a restart; while a reload is in flight, two snapshots exist in memory; and TOML gets verbose for large route tables.
- **Revisit if:** the route tables grow large enough that TOML hurts readability, or zero-downtime binary upgrades become a goal (then add option B).
