# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and this project adheres to Semantic Versioning.

## [0.1.2] - 2025-09-22

Initial release of the Neon-powered MQTT broker for Node.js.

- High-performance RMQTT core with Tokio runtime
- TypeScript API with hooks and pub/sub
- Multi-protocol listeners (TCP/TLS/WS/WSS)
- Auth and Subscribe ACL decisions via JS hooks
- Delivery and lifecycle hooks
- Source-install builds the native addon at install time
