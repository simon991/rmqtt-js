"use strict";

// Barrel exports for the public API (now colocated in src)

// Core server class
export { MqttServer } from "./ts/api/MqttServer";

// Config types
export type { ServerConfig, ListenerConfig } from "./ts/api/config";

// Hook-related types
export type {
  HookCallbacks,
  AuthenticationRequest,
  AuthenticationResult,
  SubscribeAuthorizeResult,
  MessagePublishHook,
  ClientSubscribeHook,
  ClientUnsubscribeHook,
  MessageDeliveredHook,
} from "./ts/api/hooks";

// Public data types
export { QoS } from "./ts/api/types";
export type {
  SessionInfo,
  MessageFrom,
  MessageInfo,
  SubscriptionInfo,
  UnsubscriptionInfo,
  MqttMessage,
  SubscribeOptions,
  PublishOptions,
  MultiProtocolOptions,
} from "./ts/api/types";

// Default export for CommonJS consumers
import { MqttServer as _MqttServerDefault } from "./ts/api/MqttServer";
export default _MqttServerDefault;
