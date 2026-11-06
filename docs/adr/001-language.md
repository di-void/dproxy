# ADR-001: Implementation language

- **Status:** Proposed
- **Date:** 2026-11-02

## Context

dproxy sits on the request path of every call it serves. The SLOs in the [spec](../spec.md) require < 1 ms of added p99 latency and ≥ 10k rps on 4 cores. Tail latency matters more than mean throughput. The proxy parses untrusted input from the network, so memory safety is a security property here. The project is also a learning vehicle.

## Options considered

| Criterion | Rust | Go | C++ | Zig |
|---|---|---|---|---|
| Memory safety | Compile-time, no GC | GC, safe | Manual; parsing untrusted input is risky | Manual, with some safety checks |
| Tail latency | No GC pauses; predictable | GC pauses are sub-ms now but add to p99 | Best possible, if done right | Best possible, if done right |
| Concurrency model | async/await on tokio (work-stealing scheduler) | Goroutines, the simplest model here | Hand-rolled event loops or coroutines | Hand-rolled; async still unstable |
| HTTP/1.1 + HTTP/2 | `hyper` 1.x (mature, used by many production proxies) | `net/http` stdlib (excellent) | nghttp2, Boost.Beast | Immature |
| TLS | `rustls` (memory-safe, ALPN) | `crypto/tls` stdlib | OpenSSL / BoringSSL | Bindings to C libraries |
| Profiling | `samply`, `cargo flamegraph`, `perf`, `tokio-console`, `dhat` | `pprof` built in (best in class) | `perf`, VTune, sanitizers | `perf` |
| Prior art | Pingora, Linkerd2-proxy, sozu | Traefik, Caddy | Envoy, HAProxy (C) | Few |
| Learning value | High: async internals, ownership under concurrency | Low: `httputil.ReverseProxy` does most of the work | High, but most of the effort goes into footguns | High, but the ecosystem is too thin |

## Decision

We will use **Rust (edition 2024)** with this stack:

- `tokio` (runtime), `hyper` 1.x + `hyper-util` (HTTP/1.1 and HTTP/2, server and client)
- `rustls` + `tokio-rustls` (TLS, ALPN)
- `arc-swap` (config snapshot), `serde` + `toml` (config)
- `tracing` (logs), `metrics` + a Prometheus exporter (or `prometheus-client`)

Why Rust: no GC protects the p99 budget; the compiler enforces memory safety on untrusted input; hyper and rustls are production-grade; and Pingora and Linkerd2-proxy show this exact use case works in Rust.
Personal Goa: Understand async rust deeply

## Consequences

- **Good:** predictable latency, memory safety, a strong library ecosystem, and good profiling tools.
- **Bad / accepted costs:** async Rust is harder to learn (`Pin`, `Send` bounds, cancellation safety), compile times are slower, and hyper 1.x is low-level, so connection pooling and graceful shutdown take hand wiring.
- **Profiling plan:** run benchmarks and `perf`/flamegraphs on Linux (WSL2 is fine); `samply` also works on Windows for quick local checks.
- **Revisit if:** we spend more time fighting the borrow checker than shipping milestones by M2, or hyper blocks a required feature.
