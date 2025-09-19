# MQTT Client Cheat Sheet

Quick recipes for testing against the example server. Covers mqtt.js (Node.js) and paho-mqtt (Python) over TCP, WebSocket, and TLS/WSS.

> Credentials (example server): username: `alice`, password: `demo`

## mqtt.js (Node.js)

Install:
```bash
npm i mqtt
```

TCP (mqtt://)
```js
const mqtt = require('mqtt');
const client = mqtt.connect('mqtt://localhost:1883', {
  username: 'alice',
  password: 'demo',
  clientId: 'mqttjs-demo',
  clean: true,
  keepalive: 60,
});

client.on('connect', () => {
  console.log('connected');
  client.subscribe('alice/#', { qos: 1 });
  client.publish('alice/test', 'hello from mqtt.js', { qos: 1 });
});

client.on('message', (topic, payload) => {
  console.log(`msg ${topic} = ${payload.toString()}`);
});
```

WebSocket (ws://)
```js
const mqtt = require('mqtt');
// If your server exposes WS on port 8080
const client = mqtt.connect('ws://localhost:8080', {
  username: 'alice',
  password: 'demo',
});
```

TLS (mqtts://)
```js
const fs = require('fs');
const mqtt = require('mqtt');
const client = mqtt.connect('mqtts://localhost:8883', {
  username: 'alice',
  password: 'demo',
  // Uncomment and provide CA if using self-signed certs
  // ca: fs.readFileSync('./certs/ca.pem'),
  // rejectUnauthorized: true,
});
```

WSS (wss://)
```js
const mqtt = require('mqtt');
const client = mqtt.connect('wss://localhost:8443', {
  username: 'alice',
  password: 'demo',
  // ca/cert handling depends on your deployment/environment
});
```

## Python (paho-mqtt)

Install:
```bash
python3 -m pip install paho-mqtt
```

TCP (mqtt://)
```python
import paho.mqtt.client as mqtt

client = mqtt.Client(client_id='paho-demo', clean_session=True)
client.username_pw_set('alice', 'demo')

def on_connect(client, userdata, flags, rc):
    print('connected rc=', rc)
    client.subscribe('alice/#', qos=1)
    client.publish('alice/test', 'hello from paho-mqtt', qos=1)

client.on_connect = on_connect
client.on_message = lambda c, u, m: print(f"msg {m.topic} = {m.payload.decode()}")
client.connect('localhost', 1883, 60)
client.loop_forever()
```

WebSocket (ws://)
```python
import paho.mqtt.client as mqtt

# WebSocket requires enabling WS listener on the server (e.g., port 8080)
client = mqtt.Client(transport='websockets')
client.username_pw_set('alice', 'demo')
client.connect('localhost', 8080, 60)
client.loop_start()
client.subscribe('alice/#', qos=1)
client.publish('alice/test', 'hi over ws', qos=1)
```

TLS (mqtts://)
```python
import ssl
import paho.mqtt.client as mqtt

client = mqtt.Client()
client.username_pw_set('alice', 'demo')
client.tls_set(ca_certs='./certs/ca.pem')  # Use your CA bundle or disable for testing
client.tls_insecure_set(False)
client.connect('localhost', 8883, 60)
client.loop_start()
client.subscribe('alice/#', qos=1)
client.publish('alice/test', 'secure hello', qos=1)
```

WSS (wss://)
```python
import ssl
import paho.mqtt.client as mqtt

client = mqtt.Client(transport='websockets')
client.username_pw_set('alice', 'demo')
# Certificate verification depends on environment; provide CA if self-signed
client.tls_set(ca_certs='./certs/ca.pem')
client.connect('localhost', 8443, 60)
client.loop_start()
client.subscribe('alice/#', qos=1)
client.publish('alice/test', 'secure ws hello', qos=1)
```