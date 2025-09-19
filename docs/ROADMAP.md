# Project Roadmap

This roadmap prioritizes features and improvements of this MQTT server (Neon + Rust RMQTT). It focuses on high-impact gaps and leverage points that fit our architecture.


## Legend
- P0 (Now): High impact, clear path, unblocks adoption or parity
- P1 (Next): Valuable parity/UX, moderate scope or dependencies
- P2 (Later): Nice-to-have or larger design/infra effort

---

## P0 — Now

1) Publish authorization hook (ACL + packet mutation)
- What: expose `authorizePublish` to allow/deny and even mutate outgoing packets (e.g., block `$SYS/`, rewrite payload). We currently only provide subscribe ACL; publish ACL is missing.
- Scope:
  - TypeScript API: Add `onClientPublishAuthorize(session, packet) => { allow: boolean; topic?: string; payload?: Buffer; qos?: QoS; reason?: string }` (non-breaking optional).
  - Rust: Implement decision hook with oneshot + 5s timeout (deny on timeout/channel error). Default-deny `$SYS/` unless explicitly allowed.
  - Files: `src/rs/hooks.rs`, `src/lib.rs` (hooks mapping), `src/ts/api/hooks.ts`, `src/ts/api/MqttServer.ts` (wiring), tests under `test/` (client publish allow/deny + mutation).
- Acceptance: Hook can deny/allow client publishes; can mutate payload/topic; `$SYS/` publishes denied by default.

1) Client lifecycle hooks (connect/ready/disconnect/error)
- What: emit `client`, `clientReady`, `clientDisconnect`, `clientError`, `connectionError`. We currently expose only publish/subscribe events and auth/subscribe ACL.
- Scope:
  - TypeScript: Add optional hooks `onClientConnected`, `onClientReady`, `onClientDisconnected`, `onClientError`, `onConnectionError` (non-breaking optional).
  - Rust: Capture RMQTT equivalents and dispatch via `Channel`.
  - Files: `src/rs/hooks.rs`, `src/lib.rs`, `src/ts/api/hooks.ts` (types), tests.
- Acceptance: Hooks fire reliably with minimal payloads (clientId, username, remoteAddr, reason if applicable).

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

1)  Pluggable JS plugin system (nice-to-have)
- Why: Encourage community extensions (auth providers, enrichers, bridges) beyond ad-hoc hooks.
- Scope:
  - TS: Define plugin interface; lifecycle; registration API.
- Acceptance: Example plugin + docs; safe execution boundaries.

---

## Implementation notes (by file)
- Rust
  - `src/rs/hooks.rs`: Add publish ACL, pre-connect, forward filtering, lifecycle events; maintain oneshot+5s timeouts with deny-on-timeout.
  - `src/rs/server.rs`: Track sessions/metrics; apply connection policies; server-side subscriptions; `$SYS` publisher.
  - `src/lib.rs`: Extend JS-visible methods and hook registration mapping.
- TypeScript
  - `src/ts/api/hooks.ts`: Add hook types for publish ACL, lifecycle, pre-connect, forward filtering; document return shapes.
  - `src/ts/api/MqttServer.ts`: New methods/getters (`getStats`, `id`, `connectedClients`, server-side subscribe/unsubscribe when implemented); wire hooks.
  - `src/ts/api/config.ts`: Extend server config (policies, heartbeat, persistence, cluster options).
  - `src/ts/utils/validators.ts`: Validate new config fields (timeouts, TLS, persistence/cluster invariants).
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
