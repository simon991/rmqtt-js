# Project Roadmap

This roadmap prioritizes features and improvements of this MQTT server (Neon + Rust RMQTT). It focuses on high-impact gaps and leverage points that fit our architecture.


## Legend
- P0 (Now): High impact, clear path, unblocks adoption or parity
- P1 (Next): Valuable parity/UX, moderate scope or dependencies
- P2 (Later): Nice-to-have or larger design/infra effort

---

## P0 — Now

1) Basic metrics and introspection
- What: Expose important metrics such as `connectedClients`. So far, we don’t surface metrics beyond `running`.
- Scope:
  - TS API: Add `getStats()` returning `{ connectedClients: number; startTime: number; uptimeMs: number; }`.
  - Optional `$SYS` heartbeat publisher toggle via config: `{ enableSysHeartbeat?: boolean; heartbeatIntervalMs?: number }`.
  - Files: `src/rs/server.rs` (track active sessions), `src/ts/api/MqttServer.ts`, `src/ts/api/config.ts`, `src/ts/utils/validators.ts`.
- Acceptance: `getStats()` returns accurate values; `$SYS/<id>/heartbeat` published when enabled.

---

## P1 — Next

4) Pre-connect throttle/limits & connection policies
- What: Add support for `preConnect` and connection limits: `connectTimeout`, `keepaliveLimit`, `queueLimit`, `maxClientsIdLength`.
- Scope:
  - TS Config: Add optional connection policy fields: `{ connectTimeoutMs?, keepaliveLimit?, queueLimit?, maxClientIdLength? }`.
  - Hook: `onPreConnect(info) => { allow: boolean; reason?: string }` for IP-based throttling/blacklist.
  - Rust: Implement pre-connect hook with 5s timeout; ensure policy enforcement.
  - Files: `src/ts/api/config.ts`, `src/ts/utils/validators.ts`, `src/lib.rs` (parse), `src/rs/server.rs` (apply), `src/rs/hooks.rs` (preConnect).
- Acceptance: Policies enforced; pre-connect hook can throttle/deny early.

1) Persistence configuration (durable sessions/messages)
- What: Support pluggable persistence for QoS>0, retained, will, and subscriptions (e.g., memory, Redis, Mongo). We do not expose persistence configuration from TS.
- Scope:
  - TS Config: `persistence?: { engine: 'memory' | 'rocksdb' | 'redis' | 'mongodb'; options?: Record<string, unknown>; }` (exact engines depend on RMQTT capabilities).
  - Rust: Map config to RMQTT persistence; document supported backends; ensure retained/QoS1/2 durability.
  - Tests: restart broker and verify retained/QoS1/2 state recovery.
- Acceptance: Selected persistence backend is applied; durable behavior proven in tests.

1) MQTT v5 publish options and reason codes
- What: expose many packet-level details; RMQTT supports MQTT v5. Our TS API only exposes `{ qos, retain }`.
- Scope:
  - TS: Extend `publish()` options with MQTT v5 fields: `messageExpiryInterval`, `contentType`, `responseTopic`, `correlationData`, `userProperties`, `payloadFormatIndicator`.
  - Rust: Marshal properties to RMQTT’s publish path.
  - Validation: Type-check fields; degrade gracefully when not supported by clients.
- Acceptance: v5 properties round-trip where applicable; no regressions for v3 clients.

1) Enhanced subscribe ACL semantics
- What: `authorizeSubscribe` should be able to negate a subscription (return null) and change QoS/topic; returns SubAck=128 for rejected filters.
- Scope:
  - TS: Allow `onClientSubscribeAuthorize` to return `{ allow: boolean }` or `{ qos: QoS }` or negate (implicit `allow=false`) with an optional reason.
  - Rust: Map decisions to proper SubAck reason codes (128) and optional QoS override.
- Acceptance: Clients receive correct SubAck behaviors and reason codes.

1) Server introspection getters
- What: Expose `.id`, `.connectedClients`, `.closed`. Our API could surface stable getters.
- Scope:
  - TS: Add getters: `server.id`, `server.connectedClients`, `server.closed` (alias of `!running`).
  - Rust: Generate a stable id and track connected count.
- Acceptance: Getters return consistent values; tests validate.

1) Rate limiting and safety limits
- What: Broker-side policies to protect resources and improve reliability (inspired by EMQX/Mosquitto/VerneMQ best practices).
- Scope:
  - TS Config: `{ rateLimits?: { connRatePerSec?, maxConnections?, publishMsgsPerSec?, publishBytesPerSec?, maxInflight?, maxQueued?, maxPacketSizeBytes? } }`.
  - Rust: Enforce at connection/publish paths; apply per-client counters; drop/deny with clear reason.
  - Tests: flood-publish and oversize packet tests; ensure graceful denials with v5 reason codes when possible.
- Acceptance: Limits enforced with predictable behavior; no broker crash; clear client feedback on violations.

1) MQTT v5 subscription options
- What: Support MQTT 5 subscription features: `noLocal`, `retainAsPublished`, `retainHandling`, and `subscriptionIdentifier`.
- Scope:
  - TS: Extend subscribe-related types and tests; surface subscription identifiers in publish hook payloads.
  - Rust: Marshal options to RMQTT; include identifiers in forwarded PUBLISH where applicable.
- Acceptance: Options honored end-to-end; identifiers visible to clients that request them.

1) Observability: Prometheus metrics (phase 1)
- What: Expose a basic Prometheus endpoint with broker counters (connections, publishes, acks, drops, retained, inflight, queues) and hook timings.
- Scope:
  - TS Config: `{ metrics?: { prometheus?: { enabled: boolean; port?: number; host?: string } } }`.
  - Rust: Small HTTP endpoint (Tokio hyper/axum) exporting metrics; or integrate with RMQTT if provided.
- Acceptance: Scrapeable metrics locally; minimal overhead; docs include sample Grafana panel list.

1) Shared subscriptions enablement & docs
- What: RMQTT supports `$share/{group}/{filter}`. Ensure compatibility, add tests and documentation.
- Scope:
  - Tests: Multiple consumers in a shared group receive load-balanced messages.
  - Docs: Example with two subscribers sharing a group.
- Acceptance: Works as expected with QoS1/2; documented.

---

## P2 — Later

9) Server-side subscribe with backpressure
- What: support `.subscribe(topic, deliverfunc, callback)` with backpressure to process messages server-side.
- Scope:
  - TS: `server.subscribe(topic, (msg, done) => void)` and `server.unsubscribe(topic, fn)` bypassing ACL (admin-level), with optional QoS.
  - Rust: Register server-side subscription and deliver via channel; respect backpressure semantics.
- Acceptance: Server can consume topics reliably and stop cleanly.

1)  Authorize-forward hook
- What: Support `authorizeForward` to filter/modify retained or pre-delivery packets.
- Scope:
  - TS: `onAuthorizeForward(session, packet) => MqttMessage | null`.
  - Rust: Apply to retained delivery and pre-delivery path.
- Acceptance: Hook can drop/transform messages before forwarding.

1)  Clustering / multi-node scaling
- What: We should expose RMQTT’s clustering/bridge capabilities.
- Scope:
  - TS Config: `cluster?: { enabled: boolean; backend: 'redis' | 'nats' | 'custom'; options?: ... }` (subject to RMQTT support).
  - Docs: deployment patterns for HA and horizontal scaling.
- Acceptance: Multi-node tests/benchmarks demonstrate cross-instance routing.


1) Client lifecycle hooks (connect/connack/connected/disconnected) — Delivered
- Status: Implemented and tested: `onClientConnect`, `onClientConnack`, `onClientConnected`, `onClientDisconnected`, plus session lifecycle (`onSessionCreated`, `onSessionSubscribed`, `onSessionUnsubscribed`, `onSessionTerminated`) and message delivery lifecycle (`onMessageDelivered`, `onMessageAcked`, `onMessageDropped`).
- Follow-ups:
  - Populate `MessageFrom.clientId/username` when origin is a client (enhanced attribution).
  - Enrich `MessageFrom.type` with more granular variants that mirror RMQTT origins (e.g., admin, bridge, last-will) so hook consumers can distinguish automated publishers from real clients without extra heuristics.
  - Thread real `node` id into `SessionInfo`/`MessageFrom` when clustering/multi-node is enabled (currently placeholder `1`).

1)  Pluggable JS plugin system (nice-to-have)
- Why: Encourage community extensions (auth providers, enrichers, bridges) beyond ad-hoc hooks.
- Scope:
  - TS: Define plugin interface; lifecycle; registration API.
- Acceptance: Example plugin + docs; safe execution boundaries.

1)  Full $SYS topics and admin API
- What: Publish broker metrics/events to `$SYS/#` topics and provide a minimal admin/read-only API.
- Scope:
  - TS Config: `{ sysTopics?: { enabled: boolean; namespace?: string } }`.
  - Rust: Emit on client connect/disconnect, subscribe/unsubscribe counters, heartbeat, retained counts.
  - Admin: Optional HTTP endpoints to fetch stats (read-only) mirroring Prometheus counters.
- Acceptance: `$SYS` visible when enabled; admin endpoints return JSON; docs list topics.

1)  PROXY protocol v1/v2 and forwarded IPs
- What: Preserve original client IP/port from load balancers; support `X-Forwarded-For` on websockets.
- Scope:
  - TS Config: `{ proxy?: { protocol?: 'none' | 'v1' | 'v2'; trustForwarded?: boolean } }`.
  - Rust: Parse Proxy Protocol headers and store in session info; expose to hooks.
- Acceptance: Remote address reflects original source behind LB when enabled.

1)  Advanced delivery controls
- What: Add delayed publish (`$delayed/...`), topic rewrite, auto-subscription, P2P messaging patterns supported by RMQTT.
- Scope:
  - TS Config toggles; examples and tests where feasible.
- Acceptance: Documented and validated where supported; disabled by default.

---

## Implementation notes (by file)
- Rust
  - `src/rs/hooks.rs`: Add publish ACL, pre-connect, forward filtering, lifecycle events; maintain oneshot+5s timeouts with deny-on-timeout.
  - `src/rs/server.rs`: Track sessions/metrics; apply connection policies; server-side subscriptions; `$SYS` publisher.
  - `src/lib.rs`: Extend JS-visible methods and hook registration mapping.
- TypeScript
  - `src/ts/api/hooks.ts`: Add hook types for publish ACL, lifecycle, pre-connect, forward filtering; document return shapes.
  - `src/ts/api/MqttServer.ts`: New methods/getters (`getStats`, `id`, `connectedClients`, server-side subscribe/unsubscribe when implemented); wire hooks.
  - `src/ts/api/config.ts`: Extend server config (policies, heartbeat, persistence, cluster options, rate limits, metrics, auth backends, proxy, bridges).
  - `src/ts/utils/validators.ts`: Validate new config fields (timeouts, TLS, persistence/cluster invariants, rate limits, metrics, auth, proxy).
- Tests
  - Add targeted integration tests using `mqtt` client; ensure unique ports; leverage `waitForPort`.

## Prioritization summary
- P0: Publish ACL (with $SYS default deny), lifecycle hooks, basic metrics/heartbeat.
- P1: Connection policies & preConnect, persistence config, MQTT v5 publish options & SubAck semantics, introspection getters.
- P2: Server-side subscribe/backpressure, authorize-forward, clustering, $SYS suite + admin API, shared subscriptions, plugin system.

## Notes on compatibility & stability
- All new hooks should be optional; absence implies RMQTT defaults.
- Maintain 5s timeouts and deny-by-default on JS-dependent decisions (auth, subscribe ACL, publish ACL, preConnect).
- Keep public TS API stable; evolve via optional fields and new methods/getters to avoid breaking changes.
