"use strict";

// Core public types/enums used by consumers

export enum QoS {
  AtMostOnce = 0,
  AtLeastOnce = 1,
  ExactlyOnce = 2
}

export interface SessionInfo {
  node: number;
  remoteAddr: string | null;
  clientId: string;
  username: string | null;
}

export interface MessageFrom {
  type: "client" | "system" | "bridge" | "admin" | "lastwill" | "custom";
  node: number;
  remoteAddr: string | null;
  clientId: string;
  username: string | null;
}

export interface MessageInfo {
  dup: boolean;
  qos: QoS;
  retain: boolean;
  topic: string;
  payload: Buffer;
  createTime: number;
}

export interface SubscriptionInfo {
  topicFilter: string;
  qos: QoS;
}

export interface UnsubscriptionInfo {
  topicFilter: string;
}

export interface MqttMessage {
  topic: string;
  payload: Buffer;
  qos: QoS;
  retain: boolean;
}

export interface SubscribeOptions { qos?: QoS }

export interface PublishOptions { qos?: QoS; retain?: boolean }

export interface MultiProtocolOptions {
  tcpPort?: number;
  tlsPort?: number;
  wsPort?: number;
  wssPort?: number;
  address?: string;
  tlsCert?: string;
  tlsKey?: string;
  allowAnonymous?: boolean;
}

// Event payloads aligned with RMQTT webhook documentation
export interface ConnectInfo {
  node: number;
  remoteAddr: string | null;
  clientId: string;
  username: string | null;
  keepAlive: number;
  protoVer: number;
  cleanSession?: boolean; // MQTT 3.1/3.1.1
  cleanStart?: boolean;   // MQTT 5.0
}

export interface ConnackInfo extends ConnectInfo {
  connAck: string; // Broker outcome string, e.g., "Success", "BadUsernameOrPassword", "NotAuthorized"
}
