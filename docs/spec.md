# dproxy: Spec v0.1

> Status: Draft · Owner: Dee · Last updated: 2026-11-02
> One page. If a feature isn't listed under **In scope**, it's out.

## 1. Problem

Put one process in front of a set of HTTP backends. It should spread traffic across them, take unhealthy ones out of rotation, and make every request observable. It should add almost no latency of its own, and it should keep serving when a backend dies.

## 2. Goals

- A Layer 7 reverse proxy for **HTTP/1.1 and HTTP/2**, both downstream (clients to proxy) and upstream (proxy to backends).
- **Health checking:** active probes, plus passive ejection of backends that keep failing.
- **Load balancing** across the endpoints in a cluster.
- **Observability:** metrics, access logs and request IDs are on from day one, not added later.
- Config reloads with **zero dropped requests** (see [ADR-002](adr/002-config-and-reload.md)).

## 3. Scope

| In scope (v1) | Out of scope (v1) |
|---|---|
| HTTP/1.1 with keep-alive, HTTP/2 over TLS (ALPN), h2c with prior knowledge | WAF / request inspection |
| TLS termination (rustls), loading certificates from files | Response caching |
| Routing on host + path prefix to a cluster | Service discovery (DNS SRV, Consul, k8s): endpoints are static, from config |
| Load balancing: round robin, power-of-two-choices least-request | HTTP/3 / QUIC |
| Active HTTP health checks + passive outlier ejection | WebSockets / CONNECT tunnelling (stretch goal) |
| Timeouts (connect, request, idle), retries for idempotent requests with a retry budget | Auth, rate limiting, plugins (Lua/WASM) |
| Header handling: `X-Forwarded-*`, `Via`, request ID, stripping hop-by-hop headers | mTLS to upstreams, upstream TLS verification options |
| Prometheus `/metrics`, structured JSON access logs, admin API | Distributed tracing export (OTLP): planned for later |
| Graceful shutdown (drain), hot config reload | Multi-node / clustering of the proxy itself |

## 4. Vocabulary

These terms follow Envoy's so that readers who know Envoy can map them straight across.

- **Listener**: a bound address and port, plus optional TLS settings.
- **Route**: a match rule (host, path prefix) that points to a cluster.
- **Cluster**: a named group of interchangeable backends, with an LB policy and a health check policy.
- **Endpoint**: a single backend `host:port` inside a cluster.

## 5. SLOs (exit gates)

Every later milestone must pass this table before it's called done. A regression blocks the merge.

| # | SLO | Target | How it's measured |
|---|---|---|---|
| S1 | Proxy-added latency, p99 | **< 1 ms** | p99(through proxy) − p99(direct to backend), same constant load, same payload |
| S2 | Proxy-added latency, p50 | < 200 µs | Same method as S1 |
| S3 | Throughput | **≥ 10k rps on 4 cores** while S1 still holds | Constant-rate load at 10k rps, proxy pinned to 4 cores |
| S4 | Success rate, 1 of 3 backends down | **≥ 99.9%** | Kill 1 of 3 backends mid-run; count non-2xx responses over the whole run |
| S5 | Failure detection time | Backend ejected in ≤ 3 s | From the kill to the endpoint being marked unhealthy in metrics |
| S6 | Config reload | 0 failed requests | Reload 10× during a run at 10k rps |
| S7 | Memory | < 100 MB RSS at 10k concurrent connections | Read RSS at steady state |
| S8 | Graceful shutdown | In-flight requests finish within the drain timeout (default 30 s) | Send SIGTERM during load; count requests reset |

**Benchmark setup** (keep it fixed so results stay comparable over time):

- Linux (bare metal, VM or WSL2), never native Windows. Record the CPU model and kernel version.
- Load generator: a constant-rate tool such as `wrk2`, `vegeta` or `oha -q`. These avoid coordinated omission. Plain `wrk` doesn't, so don't use it.
- The load generator, proxy and backends run on separate cores (`taskset`). Backend: a trivial static 200 response with a 1 KiB body.
- 100 keep-alive connections. Each run is 60 s, after a 10 s warm-up.
- Results go in `bench/results/<date>-<git-sha>.md`.

## 6. Non-functional

- No `unsafe` in dproxy's own code without an ADR.
- No panics on the request path. A malformed request gets a 4xx; it never crashes the process.
- One static binary, one config file.

## 7. questions

- Should a retry go to a different endpoint than the one that just failed? Yes
