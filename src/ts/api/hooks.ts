"use strict";

import { MessageFrom, MessageInfo, MqttMessage, QoS, SessionInfo, SubscriptionInfo, UnsubscriptionInfo } from "./types";

export interface AuthenticationRequest {
  clientId: string;
  username: string | null;
  password: string | null;
  protocolVersion: number;
  remoteAddr: string;
  keepAlive: number;
  cleanSession: boolean;
}

export interface AuthenticationResult {
  allow: boolean;
  superuser?: boolean;
  reason?: string;
}

export interface SubscribeAuthorizeResult {
  allow: boolean;
  qos?: number; // QoS value 0/1/2; consumers can import QoS if they want enum
  reason?: string;
}

export interface PublishAuthorizeResult {
  allow: boolean;
  // Optional mutation fields
  topic?: string;
  payload?: Buffer;
  // Keep as number to align with existing patterns; recommend using QoS enum
  qos?: number; // 0 | 1 | 2
  reason?: string;
}

export interface HookCallbacks {
  onClientAuthenticate?: (authRequest: AuthenticationRequest) => AuthenticationResult | Promise<AuthenticationResult>;
  onClientSubscribeAuthorize?: (session: SessionInfo | null, subscription: SubscriptionInfo) => SubscribeAuthorizeResult | Promise<SubscribeAuthorizeResult>;
  onClientPublishAuthorize?: (session: SessionInfo | null, packet: MqttMessage) => PublishAuthorizeResult | Promise<PublishAuthorizeResult>;
  onMessagePublish?: (session: SessionInfo | null, from: MessageFrom, message: MessageInfo) => void;
  onClientSubscribe?: (session: SessionInfo | null, subscription: SubscriptionInfo) => void;
  onClientUnsubscribe?: (session: SessionInfo | null, unsubscription: UnsubscriptionInfo) => void;
}

export type MessagePublishHook = (session: SessionInfo | null, from: MessageFrom, message: MessageInfo) => void;
export type ClientSubscribeHook = (session: SessionInfo, subscribe: SubscriptionInfo) => void;
export type ClientUnsubscribeHook = (session: SessionInfo, unsubscribe: UnsubscriptionInfo) => void;
export type MessageDeliveredHook = (session: SessionInfo, from: MessageFrom, message: MessageInfo) => void;
