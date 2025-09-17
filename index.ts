"use strict";

// Type-safe native module imports
interface NativeModule {
  mqttServerNew(): unknown;
  mqttServerClose(this: unknown): void;
  mqttServerStart(this: unknown, config: any): Promise<void>;
  mqttServerStop(this: unknown): Promise<void>;
}

// Import the native module
const nativeModule = require("./index.node") as NativeModule;
const { mqttServerNew, mqttServerClose, mqttServerStart, mqttServerStop } = nativeModule;

/**
 * Configuration for a single MQTT listener
 */
export interface ListenerConfig {
  /** Unique name for this listener */
  name: string;
  /** IP address to bind to (default: "0.0.0.0") */
  address?: string;
  /** Port number to listen on */
  port: number;
  /** Protocol type */
  protocol: "tcp" | "tls" | "ws" | "wss";
  /** Path to TLS certificate file (required for tls/wss) */
  tlsCert?: string;
  /** Path to TLS private key file (required for tls/wss) */
  tlsKey?: string;
  /** Whether to allow anonymous connections (default: true) */
  allowAnonymous?: boolean;
}

/**
 * MQTT server configuration
 */
export interface ServerConfig {
  /** Array of listener configurations */
  listeners: ListenerConfig[];
  /** Optional directory containing plugin configuration files */
  pluginsConfigDir?: string;
}

/**
 * Options for creating a multi-protocol server configuration
 */
export interface MultiProtocolOptions {
  /** TCP port (default: 1883) */
  tcpPort?: number;
  /** TLS port (default: 8883) */
  tlsPort?: number;
  /** WebSocket port (default: 8080) */
  wsPort?: number;
  /** WebSocket Secure port (default: 8443) */
  wssPort?: number;
  /** Bind address (default: "0.0.0.0") */
  address?: string;
  /** Path to TLS certificate file */
  tlsCert?: string;
  /** Path to TLS private key file */
  tlsKey?: string;
  /** Allow anonymous connections (default: true) */
  allowAnonymous?: boolean;
}

/**
 * High-performance MQTT server wrapper for Node.js
 * Built on top of the RMQTT Rust library using Neon bindings
 */
export class MqttServer {
  private readonly server: unknown;
  private isRunning: boolean = false;

  constructor() {
    this.server = mqttServerNew();
  }

  /**
   * Start the MQTT server with the given configuration
   * @param config - Server configuration
   * @throws Error if server is already running or configuration is invalid
   */
  async start(config: ServerConfig): Promise<void> {
    if (this.isRunning) {
      throw new Error("Server is already running");
    }

    // Validate configuration
    this._validateConfig(config);

    // Set default values and normalize configuration
    const normalizedConfig = {
      listeners: config.listeners.map(listener => ({
        name: listener.name,
        address: listener.address || "0.0.0.0",
        port: listener.port,
        protocol: listener.protocol,
        tlsCert: listener.tlsCert,
        tlsKey: listener.tlsKey,
        allowAnonymous: listener.allowAnonymous !== false
      })),
      pluginsConfigDir: config.pluginsConfigDir,
    };

    await mqttServerStart.call(this.server, normalizedConfig);
    this.isRunning = true;
  }

  /**
   * Stop the MQTT server
   */
  async stop(): Promise<void> {
    if (!this.isRunning) {
      return;
    }

    await mqttServerStop.call(this.server);
    this.isRunning = false;
  }

  /**
   * Close the server and clean up resources
   * Call this to allow the process to exit immediately instead of waiting for GC
   */
  close(): void {
    if (this.isRunning) {
      // Note: This will force-close without graceful shutdown
      this.isRunning = false;
    }
    mqttServerClose.call(this.server);
  }

  /**
   * Check if the server is currently running
   */
  get running(): boolean {
    return this.isRunning;
  }

  /**
   * Validate server configuration
   * @private
   */
  private _validateConfig(config: ServerConfig): void {
    if (!config || typeof config !== 'object') {
      throw new Error("Configuration must be an object");
    }

    if (!Array.isArray(config.listeners) || config.listeners.length === 0) {
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
        throw new Error("Each listener protocol must be one of: tcp, tls, ws, wss");
      }

      if ((listener.protocol === 'tls' || listener.protocol === 'wss')) {
        if (!listener.tlsCert || !listener.tlsKey) {
          throw new Error("TLS/WSS listeners require both tlsCert and tlsKey");
        }
      }
    }
  }

  /**
   * Create a basic MQTT server configuration
   * @param port - Port to listen on (default: 1883)
   * @param address - Address to bind to (default: "0.0.0.0")
   * @returns Basic configuration object
   */
  static createBasicConfig(port: number = 1883, address: string = "0.0.0.0"): ServerConfig {
    return {
      listeners: [{
        name: "tcp",
        address,
        port,
        protocol: "tcp",
        allowAnonymous: true
      }]
    };
  }

  /**
   * Create a multi-protocol MQTT server configuration
   * @param options - Configuration options
   * @returns Multi-protocol configuration object
   */
  static createMultiProtocolConfig(options: MultiProtocolOptions = {}): ServerConfig {
    const {
      tcpPort = 1883,
      tlsPort = 8883,
      wsPort = 8080,
      wssPort = 8443,
      address = "0.0.0.0",
      tlsCert,
      tlsKey,
      allowAnonymous = true
    } = options;

    const listeners: ListenerConfig[] = [
      {
        name: "tcp",
        address,
        port: tcpPort,
        protocol: "tcp",
        allowAnonymous
      },
      {
        name: "ws",
        address,
        port: wsPort,
        protocol: "ws",
        allowAnonymous
      }
    ];

    if (tlsCert && tlsKey) {
      listeners.push(
        {
          name: "tls",
          address,
          port: tlsPort,
          protocol: "tls",
          tlsCert,
          tlsKey,
          allowAnonymous
        },
        {
          name: "wss",
          address,
          port: wssPort,
          protocol: "wss",
          tlsCert,
          tlsKey,
          allowAnonymous
        }
      );
    }

    return { listeners };
  }
}

// Export types for consumers
// Note: Types are already exported above as interfaces

// Default export for CommonJS compatibility
export default MqttServer;