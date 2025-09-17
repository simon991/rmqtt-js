/**
 * Type definitions for the native MQTT server module
 * This module is compiled from Rust using Neon bindings
 */

declare module "./index.node" {
  /**
   * Native MQTT server configuration object
   */
  export interface NativeServerConfig {
    listeners: Array<{
      name: string;
      address: string;
      port: number;
      protocol: "tcp" | "tls" | "ws" | "wss";
      tlsCert?: string;
      tlsKey?: string;
      allowAnonymous: boolean;
    }>;
    pluginsConfigDir?: string;
  }

  /**
   * Creates a new MQTT server instance
   * @returns Opaque native server handle
   */
  export function mqttServerNew(): unknown;

  /**
   * Closes the MQTT server and cleans up resources
   * @param server Native server handle
   */
  export function mqttServerClose(this: unknown): void;

  /**
   * Starts the MQTT server with the given configuration
   * @param server Native server handle
   * @param config Server configuration
   * @returns Promise that resolves when server starts
   */
  export function mqttServerStart(this: unknown, config: NativeServerConfig): Promise<void>;

  /**
   * Stops the MQTT server
   * @param server Native server handle
   * @returns Promise that resolves when server stops
   */
  export function mqttServerStop(this: unknown): Promise<void>;
}