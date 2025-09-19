"use strict";

export interface ListenerConfig {
  name: string;
  address?: string;
  port: number;
  protocol: "tcp" | "tls" | "ws" | "wss";
  tlsCert?: string;
  tlsKey?: string;
  allowAnonymous?: boolean;
}

export interface ServerConfig {
  listeners: ListenerConfig[];
  pluginsConfigDir?: string;
  pluginsDefaultStartups?: string[];
}
