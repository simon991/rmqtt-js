# Examples

This directory contains runnable examples demonstrating how to use the rmqtt-server library with real hooks, authentication, and publish/subscribe flows.

## simple-server.ts

A comprehensive example that showcases:

- Basic TCP server on port 1883
- JavaScript authentication via `onClientAuthenticate` (demo password = "demo")
- Subscribe ACLs via `onClientSubscribeAuthorize` (allow `<username>/*` and `server/public/*`, QoS ≤ 1)
- Real‑time logs for publish/subscribe/unsubscribe hooks
- Lifecycle notifications: `onClientConnect`, `onClientConnack`, `onClientConnected`, `onClientDisconnected`, `onSessionCreated`, `onSessionSubscribed`, `onSessionUnsubscribed`, `onSessionTerminated`
- Message delivery notifications: `onMessageDelivered`, `onMessageAcked`, `onMessageDropped`
- Server‑side publishing (status, heartbeat, sensor samples)
- Graceful shutdown on SIGINT/SIGTERM

### Run it

```bash
# Recommended
npm run example

# Direct (Node.js 20.6.0+)
node examples/simple-server.ts --experimental-strip-types

# Compiled
npm run build && node dist/examples/simple-server.js
```

### Try it with mosquitto

```bash
# Subscribe (allowed)
mosquitto_sub -h localhost -p 1883 -u alice -P "demo" -t "alice/#"
mosquitto_sub -h localhost -p 1883 -u alice -P "demo" -t "server/public/#"

# Subscribe (denied)
mosquitto_sub -h localhost -p 1883 -u alice -P "demo" -t "server/secret/#"

# Publish
mosquitto_pub -h localhost -p 1883 -u alice -P "demo" -t "alice/test" -m "Hello!"
```

The example runs until you press Ctrl+C and prints helpful logs describing auth decisions, ACL outcomes, and message activity.

See also: [CHEATSHEET.md](./CHEATSHEET.md) for mqtt.js and Python (paho-mqtt) quick recipes.