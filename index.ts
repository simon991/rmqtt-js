"use strict";

/**
 * MQTT Quality of Service levels
 */
export enum QoS {
  /** At most once delivery */
  AtMostOnce = 0,
  /** At least once delivery */
  AtLeastOnce = 1,
  /** Exactly once delivery */
  ExactlyOnce = 2
}

/**
 * Information about an MQTT session/client
 */
export interface SessionInfo {
  /** Node ID where the client is connected */
  node: number;
  /** Client's remote IP address and port */
  remoteAddr: string;
  /** Client identifier */
  clientId: string;
  /** Username (may be null for anonymous connections) */
  username: string | null;
}

/**
 * Information about the source of a message
 */
export interface MessageFrom {
  /** Type of message source */
  type: "client" | "system" | "bridge" | "admin" | "lastwill" | "custom";
  /** Node ID of the source */
  node: number;
  /** Remote address of the source */
  remoteAddr: string | null;
  /** Client ID of the source */
  clientId: string;
  /** Username of the source */
  username: string | null;
}

/**
 * Information about a published message
 */
export interface MessageInfo {
  /** Whether this is a duplicate message */
  dup: boolean;
  /** Quality of Service level */
  qos: QoS;
  /** Whether this message should be retained */
  retain: boolean;
  /** Topic name */
  topic: string;
  /** Message payload */
  payload: Buffer;
  /** When the message was created (milliseconds since epoch) */
  createTime: number;
}

/**
 * Information about a subscription
 */
export interface SubscriptionInfo {
  /** Topic filter being subscribed to */
  topicFilter: string;
  /** Quality of Service level for the subscription */
  qos: QoS;
}

/**
 * Information about an unsubscription
 */
export interface UnsubscriptionInfo {
  /** Topic filter being unsubscribed from */
  topicFilter: string;
}

/**
 * Hook callback functions for MQTT events
 */
export interface HookCallbacks {
  /** Called when a message is published */
  onMessagePublish?: (session: SessionInfo | null, from: MessageFrom, message: MessageInfo) => void;
  /** Called when a client subscribes to a topic */
  onClientSubscribe?: (session: SessionInfo | null, subscription: SubscriptionInfo) => void;
  /** Called when a client unsubscribes from a topic */
  onClientUnsubscribe?: (session: SessionInfo | null, unsubscription: UnsubscriptionInfo) => void;
}

/**
 * Information about a subscription request
 */
export interface SubscribeInfo {
  /** Topic filter being subscribed to */
  topicFilter: string;
  /** Quality of Service level for the subscription */
  qos: QoS;
  /** Whether to receive retained messages */
  retainHandling: number;
  /** Whether the subscription should not be forwarded */
  noLocal: boolean;
  /** Whether to preserve the retain flag when forwarding */
  retainAsPublished: boolean;
}

/**
 * Information about an unsubscribe request
 */
export interface UnsubscribeInfo {
  /** Topic filter being unsubscribed from */
  topicFilter: string;
}

/**
 * Hook callback for message publish events
 */
export type MessagePublishHook = (session: SessionInfo | null, from: MessageFrom, message: MessageInfo) => void;

/**
 * Hook callback for client subscribe events
 */
export type ClientSubscribeHook = (session: SessionInfo, subscribe: SubscribeInfo) => void;

/**
 * Hook callback for client unsubscribe events
 */
export type ClientUnsubscribeHook = (session: SessionInfo, unsubscribe: UnsubscribeInfo) => void;

/**
 * Hook callback for message delivered events
 */
export type MessageDeliveredHook = (session: SessionInfo, from: MessageFrom, message: MessageInfo) => void;

// Type-safe native module imports
interface NativeModule {
  mqttServerNew(): unknown;
  mqttServerClose(this: unknown): void;
  mqttServerStart(this: unknown, config: any): Promise<void>;
  mqttServerStop(this: unknown): Promise<void>;
  mqttServerPublish(this: unknown, topic: string, payload: Buffer, qos: number, retain: boolean): Promise<void>;
  mqttServerSetHooks(this: unknown, callbacks: HookCallbacks): void;
}

// Import the native module
const nativeModule = require("./index.node") as NativeModule;
const { mqttServerNew, mqttServerClose, mqttServerStart, mqttServerStop, mqttServerPublish, mqttServerSetHooks } = nativeModule;

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
 * MQTT message received from a topic subscription
 */
export interface MqttMessage {
  /** The topic the message was published to */
  topic: string;
  /** The message payload as a Buffer */
  payload: Buffer;
  /** Quality of Service level */
  qos: QoS;
  /** Whether the message was retained */
  retain: boolean;
}

/**
 * Callback function for handling received MQTT messages
 */
export type MessageCallback = (message: MqttMessage) => void;

/**
 * Options for subscribing to a topic
 */
export interface SubscribeOptions {
  /** Quality of Service level for the subscription (default: QoS.AtMostOnce) */
  qos?: QoS;
}

/**
 * Options for publishing a message
 */
export interface PublishOptions {
  /** Quality of Service level (default: QoS.AtMostOnce) */
  qos?: QoS;
  /** Whether to retain the message (default: false) */
  retain?: boolean;
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
  private hookCallbacks: HookCallbacks = {};

  constructor() {
    this.server = mqttServerNew();
  }

  /**
   * Register hook callbacks for MQTT events
   * @param callbacks - Object containing callback functions for different events
   */
  setHooks(callbacks: HookCallbacks): void {
    this.hookCallbacks = { ...callbacks };
    mqttServerSetHooks.call(this.server, this.hookCallbacks);
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
   * Publish a message to a topic
   * @param topic - The topic to publish to
   * @param payload - The message payload (string or Buffer)
   * @param options - Publish options (QoS, retain)
   */
  async publish(topic: string, payload: string | Buffer, options: { qos?: QoS; retain?: boolean } = {}): Promise<void> {
    if (!this.isRunning) {
      throw new Error("Server must be running to publish messages");
    }

    const { qos = 0, retain = false } = options;
    const buffer = Buffer.isBuffer(payload) ? payload : Buffer.from(payload, 'utf8');
    
    await mqttServerPublish.call(this.server, topic, buffer, qos, retain);
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