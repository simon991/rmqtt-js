---
applyTo: "src/rs/**/*.rs"
---

## Rust (Neon + RMQTT) guidance for Copilot

Keep these patterns consistent across the Rust side of the bridge:

- Tokio runtime and channels
  - The RMQTT server runs on a dedicated Tokio runtime in a background thread created in `MqttServerWrapper::new`.
  - JS→Rust commands go over `std::sync::mpsc::Sender<ServerMessage>`; Rust→JS uses `neon::event::Channel`.
  - Use the existing `ServerMessage` enum to extend behavior. Prefer adding new variants over ad-hoc threads.

- Shared server state
  - Use `SharedServerState` for readiness. Guard publish paths with `wait_for_ready(5000)`.
  - Do not block the Node event loop. Use `rt.spawn(...)` for async tasks.

- Hooks
  - Store JS callbacks in `HOOK_CALLBACKS` and dispatch with `Channel::try_send`.
  - Decision hooks (auth, subscribe ACL) must use oneshot + 5s timeout: on timeout or channel error → deny.
  - Map Rust structures to JS objects explicitly; keep field names aligned with TS interfaces.

- Errors and logging
  - Use WARN/ERROR logging sparingly for user-significant events only.
  - Use `SendResultExt::into_rejection` to ensure JS promises are rejected when channel sends fail.

- Exports
  - Add new JS-visible methods as `fn js_<name>(cx: FunctionContext) -> JsResult<...>` on `MqttServerWrapper`.
  - Export them from `#[neon::main]` with names like `mqttServer<PascalName>`.

- Protocol builders
  - Extend listener handling in `start_server` by matching on `listener_config.protocol`.
  - Validate QoS using `rmqtt::types::QoS::try_from(u8)`; on invalid value, log ERROR and drop the message.

- Testing & reliability
  - Avoid panics; propagate errors up to JS via promise rejections where feasible.
  - Keep defaults safe: if JS hooks aren’t registered, return `(true, acc)` to defer to RMQTT defaults.
