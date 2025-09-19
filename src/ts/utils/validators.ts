"use strict";

import { ServerConfig } from "../api/config";

export function validateServerConfig(config: ServerConfig): void {
  if (!config || typeof config !== 'object') {
    console.error("ERROR: Invalid configuration: expected an object.");
    throw new Error("Configuration must be an object");
  }

  if (!Array.isArray(config.listeners) || config.listeners.length === 0) {
    console.error("ERROR: Invalid configuration: at least one listener is required.");
    throw new Error("Configuration must include at least one listener");
  }

  for (const listener of config.listeners) {
    if (!listener.name || typeof listener.name !== 'string') {
      throw new Error("Each listener must have a name");
    }
    if (!listener.port || typeof listener.port !== 'number' || listener.port < 1 || listener.port > 65535) {
      throw new Error("Each listener must have a valid port number");
    }
    if (!listener.protocol || !['tcp', 'tls', 'ws', 'wss'].includes(listener.protocol)) {
      console.error("ERROR: Invalid listener protocol:", listener.protocol);
      throw new Error("Each listener protocol must be one of: tcp, tls, ws, wss");
    }
    if ((listener.protocol === 'tls' || listener.protocol === 'wss')) {
      if (!listener.tlsCert || !listener.tlsKey) {
        console.error("ERROR: TLS/WSS listeners require both tlsCert and tlsKey");
        throw new Error("TLS/WSS listeners require both tlsCert and tlsKey");
      }
    }
  }
}
