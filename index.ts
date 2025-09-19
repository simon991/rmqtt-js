"use strict";

// Barrel exports for the public API

// Core server class
export { MqttServer } from "./src/ts/api/MqttServer";

// Config types
export type { ServerConfig, ListenerConfig } from "./src/ts/api/config";

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
} from "./src/ts/api/hooks";

// Public data types
export {
  QoS,
} from "./src/ts/api/types";

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
} from "./src/ts/api/types";

// Default export for CommonJS consumers
import { MqttServer as _MqttServerDefault } from "./src/ts/api/MqttServer";
export default _MqttServerDefault;