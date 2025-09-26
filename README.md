<div align="center">

 # rmqtt-js

[![npm version](https://img.shields.io/npm/v/rmqtt-js.svg)](https://www.npmjs.com/package/rmqtt-js) [![CI](https://github.com/simon991/rmqtt-js/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/simon991/rmqtt-js/actions/workflows/ci.yml) [![npm downloads](https://img.shields.io/npm/dm/rmqtt-js.svg)](https://www.npmjs.com/package/rmqtt-js) [![license](https://img.shields.io/npm/l/rmqtt-js.svg)](./LICENSE)

Blazing‑fast, experimental MQTT broker for Node.js — powered by Rust RMQTT and Neon.

</div>

## Table of contents

- Highlights
- Install
- Quick start
- Powerful hooks
- Configuration
- Pub/Sub API
- Example app
- Client cheat sheet
- TypeScript types
- Architecture (how it works)
- Testing
- Troubleshooting
- License

## Highlights

- High performance core: Built on Rust + Tokio with the battle‑tested RMQTT engine
- First‑class TypeScript API: Clean, promise‑based API with strong types
- Multi‑protocol listeners: TCP, TLS, WebSocket, and WSS — run them all at once
- Real‑time hooks: Observe publish/subscribe/unsubscribe events in JavaScript
- Pluggable auth: Implement authentication in JavaScript with a simple callback
- Subscribe ACLs: Allow/deny subscriptions in JS and optionally override granted QoS
- Full QoS and retention: QoS 0/1/2 and retained messages supported end‑to‑end
- Minimal, actionable logs: Only WARN/ERROR logs for user‑significant issues

## Install

 ```bash
 npm install rmqtt-js
 ```

 This package ships prebuilt binaries for common platforms (linux-x64/arm64, darwin-x64/arm64, win32-x64). If a prebuild for your platform isn’t available, install will build from source:
 - Node.js 18+ recommended (engines allow >=14)
 - Rust toolchain is required when building from source
   - macOS: Xcode Command Line Tools (`xcode-select --install`)
   - Linux: `build-essential`, `pkg-config`
   - Windows: Visual Studio Build Tools (MSVC), Rustup default toolchain

### Binary size note

Release binaries published to npm are built in CI with Rust symbols stripped (`RUSTFLAGS="-C strip=symbols"`) to reduce package size. Local development builds (e.g. `npm run build`) are not stripped so that you retain full debug info when profiling or inspecting with tools like `lldb`.

## Quick start

TypeScript
```ts
import { MqttServer, QoS } from "rmqtt-js";

const server = new MqttServer();

// Optional: simple demo auth (password must be "demo")
server.setHooks({
  onClientAuthenticate: ({ username, password }) => ({
    allow: Boolean(username) && password === "demo",
    superuser: false,
  }),
});

await server.start(MqttServer.createBasicConfig(1883));

// Publish a message
await server.publish("sensor/temperature", "23.7", { qos: QoS.AtLeastOnce });
```

JavaScript (CommonJS)
```js
const { MqttServer, QoS } = require("rmqtt-js");

(async () => {
  const server = new MqttServer();
  await server.start(MqttServer.createBasicConfig(1883));
  await server.publish("hello/world", Buffer.from("hi"), { qos: QoS.AtMostOnce });
})();
```

## Powerful hooks

Listen to broker events in real time — only invoked if you register them.

### Hooks overview

| Hook                                           | Type                | When it fires                                 | JS return                                    | Default if not set             | Timeout/error |
| ---------------------------------------------- | ------------------- | --------------------------------------------- | -------------------------------------------- | ------------------------------ | ------------- |
| `onClientConnect(info)`                        | notification        | Server receives CONNECT                       | `void`                                       | —                              | —             |
| `onClientAuthenticate(auth)`                   | decision            | A client connects                             | `{ allow, superuser?, reason? }`             | RMQTT defaults                 | deny          |
| `onClientSubscribeAuthorize(session, sub)`     | decision            | Client subscribes                             | `{ allow, qos?, reason? }`                   | RMQTT defaults                 | deny          |
| `onClientPublishAuthorize(session, packet)`    | decision + mutation | Client publishes                              | `{ allow, topic?, payload?, qos?, reason? }` | allow (except `$SYS/*` denied) | deny          |
| `onMessagePublish(session, from, msg)`         | notification        | Any publish observed                          | `void`                                       | —                              | —             |
| `onClientSubscribe(session, sub)`              | notification        | Client subscribes                             | `void`                                       | —                              | —             |
| `onClientUnsubscribe(session, unsub)`          | notification        | Client unsubscribes                           | `void`                                       | —                              | —             |
| `onMessageDelivered(session, from, msg)`       | notification        | Before delivering a message to a client       | `void`                                       | —                              | —             |
| `onMessageAcked(session, from, msg)`           | notification        | After client acknowledges a delivered message | `void`                                       | —                              | —             |
| `onMessageDropped(session, from?, msg, info?)` | notification        | Message could not be delivered (dropped)      | `void`                                       | —                              | —             |
| `onSessionCreated(session)`                    | notification        | Session created                               | `void`                                       | —                              | —             |
| `onClientConnected(session)`                   | notification        | Client connected                              | `void`                                       | —                              | —             |
| `onClientConnack(info)`                        | notification        | CONNACK (success/fail)                        | `void`                                       | —                              | —             |
| `onClientDisconnected(session, info?)`         | notification        | Client disconnected                           | `void`                                       | —                              | —             |
| `onSessionSubscribed(session, sub)`            | notification        | Client subscribed                             | `void`                                       | —                              | —             |
| `onSessionUnsubscribed(session, unsub)`        | notification        | Client unsubscribed                           | `void`                                       | —                              | —             |
| `onSessionTerminated(session, info?)`          | notification        | Session terminated                            | `void`                                       | —                              | —             |

Auth (JS → Rust decision flow)
```ts
server.setHooks({
  onClientAuthenticate: ({ username, password, clientId, remoteAddr }) => {
    const allow = Boolean(username) && password === "demo";
    return { allow, superuser: false, reason: allow ? "ok" : "bad creds" };
  },
});
```

Subscribe ACLs with QoS override
```ts
server.setHooks({
  onClientSubscribeAuthorize: (session, sub) => {
    const user = session?.username ?? "";
    const allowed = sub.topicFilter.startsWith(`${user}/`) || sub.topicFilter.startsWith("server/public/");
    return allowed ? { allow: true, qos: 1 } : { allow: false, reason: "not authorized" };
  },
});

Promise-based Subscribe ACL
server.setHooks({
  onClientSubscribeAuthorize: async (session, sub) => {
    // e.g., async lookup against a policy service
    await new Promise(r => setTimeout(r, 50));
    const user = session?.username ?? "";
    const allowed = sub.topicFilter.startsWith(`${user}/`);
    // You can also override QoS (0/1/2); invalid values are ignored and a WARN is logged.
    return allowed ? { allow: true, qos: 1 } : { allow: false };
  },
});
```

Publish/Subscribe notifications
```ts
server.setHooks({
  onMessagePublish: (_session, from, msg) => {
    console.log(`Message ${msg.topic}`, msg.payload.toString());
  },
  onClientSubscribe: (_session, sub) => console.log("SUB", sub),
  onClientUnsubscribe: (_session, unsub) => console.log("UNSUB", unsub),
});
```

Message delivery notifications
```ts
server.setHooks({
  onMessageDelivered: (_session, from, message) => {
    console.log(`Delivered: ${message.topic} (from ${from.type})`);
  },
  onMessageAcked: (_session, from, message) => {
    console.log(`Acked: ${message.topic} (from ${from.type})`);
  },
  onMessageDropped: (_session, from, message, info) => {
    console.warn(`Dropped: ${message.topic} (from ${from?.type ?? 'unknown'}) reason=${info?.reason ?? 'n/a'}`);
  },
});
```

Lifecycle notifications
```ts
server.setHooks({
  onClientConnected: (session) => console.log("CONNECTED", session.clientId),
  // Emitted when broker sends CONNACK (success or failure). Filter non-success if desired.
  onClientConnack: (info) => console.log("CONNACK", info.clientId, info.connAck),
  onClientDisconnected: (session, info) => console.log("DISCONNECTED", session.clientId, info?.reason),
  // Subscribe/session lifecycle
  onSessionSubscribed: (session, sub) => console.log("SUBSCRIBED", session.clientId, sub.topicFilter),
});
```

Notes:
- `onClientConnack` fires for both success and non-success outcomes. To handle errors only, check `info.connAck !== "Success"`.
- Typical `connAck` values include: `"Success"`, `"BadUsernameOrPassword"`, `"NotAuthorized"`, and other broker reasons depending on protocol version.
- The `node` field currently uses a single-node placeholder value of `1`. If/when multi-node/clustering is introduced, this will reflect the real node id.

Publish ACL with optional mutation
```ts
server.setHooks({
  onClientPublishAuthorize: (session, packet) => {
    // Deny system topics unless explicitly allowed
    if (packet.topic.startsWith("$SYS")) {
      return { allow: false, reason: "system topic" };
    }
    // Example: rewrite topic and uppercase payload
    return {
      allow: true,
      topic: `users/${session?.username ?? "anon"}/out`,
      payload: Buffer.from(packet.payload.toString("utf8").toUpperCase()),
      qos: 0,
    };
  },
});
```

Hook semantics:
- Not registered → not called (RMQTT defaults apply)
- Auth timeouts or callback errors → deny with WARN
- Subscribe ACL timeouts or callback errors → deny with WARN
 - Publish hook: invalid mutation fields (e.g., qos not 0/1/2 or topic with wildcards +/#) are ignored and the original values are used; a WARN is logged.

## Configuration

Quick helpers
```ts
// Single TCP listener
const basic = MqttServer.createBasicConfig(1883);

// Multi‑protocol listeners (TLS/WSS require cert+key)
const multi = MqttServer.createMultiProtocolConfig({
  tcpPort: 1883,
  wsPort: 8080,
  tlsPort: 8883,
  wssPort: 8443,
  address: "0.0.0.0",
  // tlsCert: "/path/to/cert.pem",
  // tlsKey: "/path/to/key.pem",
  allowAnonymous: true,
});
```

Full shape (TypeScript)
```ts
interface ListenerConfig {
  name: string;
  address: string;      // e.g. "0.0.0.0"
  port: number;         // 1–65535
  protocol: "tcp" | "tls" | "ws" | "wss";
  tlsCert?: string;     // required for tls/wss
  tlsKey?: string;      // required for tls/wss
  allowAnonymous?: boolean; // default: true
}

interface ServerConfig {
  listeners: ListenerConfig[];
  pluginsConfigDir?: string;
  pluginsDefaultStartups?: string[];
}
```

## Pub/Sub API

```ts
await server.publish(topic: string, payload: string | Buffer, options?: {
  qos?: QoS;       // 0 | 1 | 2; default 0
  retain?: boolean // default false
});
```

Notes:
- Publishing requires the server to be running.
- QoS values are validated; invalid values are rejected with an ERROR log.
- Payloads are sent as bytes; strings are encoded as UTF‑8.

## Example app

This repo ships with a comprehensive example showing auth, ACLs, and server‑side publishing.

Run it:
```bash
npm run example
```

What it demonstrates:
- TCP server on port 1883
- JavaScript authentication (password = "demo")
- Subscribe ACL: only <username>/* and server/public/* are allowed; granted QoS capped at 1
- Real‑time logs for publish/subscribe/unsubscribe
- Periodic heartbeat publishing

The full source lives in `examples/simple-server.ts`.

See also: `examples/CHEATSHEET.md` for quick mqtt.js and Python client snippets (TCP/WS/TLS/WSS).

## TypeScript types

```ts
export enum QoS { AtMostOnce = 0, AtLeastOnce = 1, ExactlyOnce = 2 }

export interface SessionInfo { node: number; remoteAddr: string | null; clientId: string; username: string | null; }
export interface MessageFrom { type: "client" | "system" | "bridge" | "admin" | "lastwill" | "custom"; node: number; remoteAddr: string | null; clientId: string; username: string | null; }
export interface MessageInfo { dup: boolean; qos: QoS; retain: boolean; topic: string; payload: Buffer; createTime: number; }
export interface SubscriptionInfo { topicFilter: string; qos: QoS }
export interface UnsubscriptionInfo { topicFilter: string }

export interface MqttMessage {
  topic: string;
  payload: Buffer;
  qos: QoS;
  retain: boolean;
}

export interface AuthenticationRequest { clientId: string; username: string | null; password: string | null; protocolVersion: number; remoteAddr: string; keepAlive: number; cleanSession: boolean; }
export interface AuthenticationResult { allow: boolean; superuser?: boolean; reason?: string }
export interface SubscribeAuthorizeResult { allow: boolean; qos?: number; reason?: string }
export interface PublishAuthorizeResult { allow: boolean; topic?: string; payload?: Buffer; qos?: number; reason?: string }

export interface HookCallbacks {
  onClientAuthenticate?(auth: AuthenticationRequest): AuthenticationResult | Promise<AuthenticationResult>;
  onClientSubscribeAuthorize?(session: SessionInfo | null, sub: SubscriptionInfo): SubscribeAuthorizeResult | Promise<SubscribeAuthorizeResult>;
  onClientPublishAuthorize?(session: SessionInfo | null, packet: MqttMessage): PublishAuthorizeResult | Promise<PublishAuthorizeResult>;
  onMessagePublish?(session: SessionInfo | null, from: MessageFrom, msg: MessageInfo): void;
  onClientSubscribe?(session: SessionInfo | null, sub: SubscriptionInfo): void;
  onClientUnsubscribe?(session: SessionInfo | null, unsub: UnsubscriptionInfo): void;
  onMessageDelivered?(session: SessionInfo | null, from: MessageFrom, message: MessageInfo): void;
  onMessageAcked?(session: SessionInfo | null, from: MessageFrom, message: MessageInfo): void;
  onMessageDropped?(session: SessionInfo | null, from: MessageFrom | null, message: MessageInfo, info?: { reason?: string }): void;
  onClientConnect?(info: ConnectInfo): void;
  onClientConnack?(info: ConnackInfo): void;
  onClientConnected?(session: SessionInfo): void;
  onClientDisconnected?(session: SessionInfo, info?: { reason?: string }): void;
  onSessionCreated?(session: SessionInfo): void;
  onSessionSubscribed?(session: SessionInfo, subscription: SubscriptionInfo): void;
  onSessionUnsubscribed?(session: SessionInfo, unsubscription: UnsubscriptionInfo): void;
  onSessionTerminated?(session: SessionInfo, info?: { reason?: string }): void;
}

// Connection lifecycle payloads
export interface ConnectInfo {
  node: number;
  remoteAddr: string | null;
  clientId: string;
  username: string | null;
  keepAlive: number;
  protoVer: number;
  cleanSession?: boolean; // MQTT 3.1/3.1.1
  cleanStart?: boolean;   // MQTT 5.0
}

export interface ConnackInfo extends ConnectInfo {
  connAck: string; // e.g., "Success", "BadUsernameOrPassword", "NotAuthorized"
}
```

Implementation notes:
- MessageFrom is a best-effort attribution based on RMQTT origin. When the origin is a client, future versions may populate `clientId` and `username` more precisely. If multi-node is planned, threading the real node id into `SessionInfo`/`MessageFrom` early will avoid downstream assumptions.

## Architecture (how it works)

- Rust core runs RMQTT on a dedicated Tokio runtime thread, separate from Node’s event loop
- Neon bridges expose a minimal, typed API to Node.js
- JS→Rust commands use an internal channel; Rust→JS hooks use a Neon event Channel
- Hook behavior: if a JS hook is registered, it’s authoritative; if not, RMQTT defaults apply
- Safety valves: 5s timeouts for JS decisions (auth/subscribe ACL). Timeout or channel error → deny with WARN
- Resource lifecycle: start/stop/close are graceful; a small shared state waits for server readiness before publishing

## Testing

```bash
npm test
```

What the suite covers:
- Auth allow/deny, timeouts, and missing‑hook behavior
- Subscribe ACL allow/deny with QoS override and session marshalling
- Publish path and client round‑trips using a real MQTT client
- Robust readiness using event‑driven port checks (no arbitrary sleeps)

## Security best practices

- Always enable TLS in production (tls/wss) and use strong certificates
  - Generate or provision certs from a trusted CA; rotate regularly
  - In `createMultiProtocolConfig`, only enable `tls/wss` when both `tlsCert` and `tlsKey` are present
- Never hard‑code secrets (passwords, tokens); pull them from env/secret managers
- Implement `onClientAuthenticate` on the JS side to enforce your auth model
  - Consider rate limiting and account lockouts outside the hook as needed
- Restrict subscription scope with `onClientSubscribeAuthorize` (e.g., `<username>/*`)
- Prefer unique client IDs; reject anonymous if your threat model requires it
- Log only WARN/ERROR and avoid sensitive data in logs
- Keep Node and Rust dependencies up to date; rebuild native module on updates

## Troubleshooting

- ERROR: Attempted to publish while server is not running → Call `start()` before `publish()`
- ERROR: Invalid configuration → Ensure at least one listener; TLS/WSS require both `tlsCert` and `tlsKey`
- WARN: Hook timeout/channel error → Your JS hook may be slow or threw; decisions fall back to deny
- Native build fails during install → Ensure Rust toolchain and platform build tools are available

## Changelog

See `CHANGELOG.md` for release notes.

## License

MIT
