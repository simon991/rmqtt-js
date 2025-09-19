"use strict";

import { HookCallbacks } from "./hooks";
import { ListenerConfig, ServerConfig } from "./config";
import { QoS } from "./types";
import { validateServerConfig } from "../utils/validators";
import { mqttServerClose, mqttServerNew, mqttServerPublish, mqttServerSetHooks, mqttServerStart, mqttServerStop } from "../native/bridge";

export class MqttServer {
  private readonly server: unknown;
  private isRunning = false;
  private hookCallbacks: HookCallbacks = {};

  constructor() {
    this.server = mqttServerNew();
  }

  setHooks(callbacks: HookCallbacks): void {
    this.hookCallbacks = { ...callbacks };
    mqttServerSetHooks.call(this.server, this.hookCallbacks);
  }

  async start(config: ServerConfig): Promise<void> {
    if (this.isRunning) {
      console.warn("WARN: Attempted to start MQTT server, but it is already running.");
      throw new Error("Server is already running");
    }

    validateServerConfig(config);

    const normalizedConfig = {
      listeners: config.listeners.map((listener: ListenerConfig) => ({
        name: listener.name,
        address: listener.address || "0.0.0.0",
        port: listener.port,
        protocol: listener.protocol,
        tlsCert: listener.tlsCert,
        tlsKey: listener.tlsKey,
        allowAnonymous: listener.allowAnonymous !== false,
      })),
      pluginsConfigDir: config.pluginsConfigDir,
      pluginsDefaultStartups: config.pluginsDefaultStartups,
    };

    try {
      await mqttServerStart.call(this.server, normalizedConfig);
    } catch (err) {
      console.error("ERROR: Failed to start MQTT server:", err);
      throw err;
    }
    this.isRunning = true;
  }

  async stop(): Promise<void> {
    if (!this.isRunning) {
      console.warn("WARN: Attempted to stop MQTT server, but it is not running.");
      return;
    }
    try {
      await mqttServerStop.call(this.server);
    } catch (err) {
      console.error("ERROR: Failed to stop MQTT server:", err);
      throw err;
    }
    this.isRunning = false;
  }

  close(): void {
    if (this.isRunning) {
      this.isRunning = false;
    }
    mqttServerClose.call(this.server);
  }

  get running(): boolean {
    return this.isRunning;
  }

  async publish(topic: string, payload: string | Buffer, options: { qos?: QoS; retain?: boolean } = {}): Promise<void> {
    if (!this.isRunning) {
      console.error("ERROR: Attempted to publish while server is not running.");
      throw new Error("Server must be running to publish messages");
    }

    const { qos = 0, retain = false } = options;
    const buffer = Buffer.isBuffer(payload) ? payload : Buffer.from(payload, 'utf8');

    try {
      await mqttServerPublish.call(this.server, topic, buffer, qos, retain);
    } catch (err) {
      console.error("ERROR: Failed to publish message:", err);
      throw err;
    }
  }

  static createBasicConfig(port: number = 1883, address: string = "0.0.0.0"): ServerConfig {
    return {
      listeners: [{
        name: "tcp",
        address,
        port,
        protocol: "tcp",
        allowAnonymous: true,
      }],
    };
  }

  static createMultiProtocolConfig(options: { tcpPort?: number; tlsPort?: number; wsPort?: number; wssPort?: number; address?: string; tlsCert?: string; tlsKey?: string; allowAnonymous?: boolean } = {}): ServerConfig {
    const {
      tcpPort = 1883,
      tlsPort = 8883,
      wsPort = 8080,
      wssPort = 8443,
      address = "0.0.0.0",
      tlsCert,
      tlsKey,
      allowAnonymous = true,
    } = options;

    const listeners: ListenerConfig[] = [
      { name: "tcp", address, port: tcpPort, protocol: "tcp", allowAnonymous },
      { name: "ws", address, port: wsPort, protocol: "ws", allowAnonymous },
    ];

    if (tlsCert && tlsKey) {
      listeners.push(
        { name: "tls", address, port: tlsPort, protocol: "tls", tlsCert, tlsKey, allowAnonymous },
        { name: "wss", address, port: wssPort, protocol: "wss", tlsCert, tlsKey, allowAnonymous },
      );
    }

    return { listeners };
  }
}
