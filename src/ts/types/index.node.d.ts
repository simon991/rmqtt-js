declare module "*/index.node" {
  export function mqttServerNew(): unknown;
  export function mqttServerClose(this: unknown): void;
  export function mqttServerStart(this: unknown, config: any): Promise<void>;
  export function mqttServerStop(this: unknown): Promise<void>;
}