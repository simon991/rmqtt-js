# Examples

This directory contains example implementations demonstrating how to use the MQTT server library.

## simple-server.ts

A complete example showing:
- Basic MQTT server setup
- Configuration options
- Graceful shutdown handling
- Client connection examples for various languages

### Running the Example

```bash
# Using npm script (recommended)
npm run example

# Direct execution (Node.js 20.6.0+)
node examples/simple-server.ts --experimental-strip-types

# Compiled version
npm run build && node dist/examples/simple-server.js
```

### What it demonstrates

- Creating an MQTT server instance
- Starting a TCP server on port 1883
- Proper graceful shutdown with signal handling
- Client connection instructions for:
  - mosquitto command line tools
  - MQTT.js (JavaScript/Node.js)
  - paho-mqtt (Python)

The example server will run until you press Ctrl+C, showing helpful connection information and handling shutdown gracefully.