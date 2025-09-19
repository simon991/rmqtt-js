use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use async_trait::async_trait;
use neon::{event::Channel, prelude::*, types::buffer::TypedArray, types::Deferred};
use once_cell::sync::Lazy;
use rmqtt::{
    codec::types::Publish as CodecPublish,
    context::ServerContext,
    hook::{Handler, HookResult, Parameter, ReturnType, Type},
    net::Builder,
    server::MqttServer as RmqttServer,
    session::SessionState,
    types::{From, Id, Publish, QoS},
    utils::timestamp_millis,
    Result as RmqttResult,
};
use tokio::sync::oneshot;

// Global storage for JavaScript hook callbacks
static HOOK_CALLBACKS: Lazy<Mutex<HookCallbackStorage>> =
    Lazy::new(|| Mutex::new(HookCallbackStorage::new()));

struct HookCallbackStorage {
    channel: Option<Channel>,
    on_message_publish: Option<neon::handle::Root<JsFunction>>,
    on_client_subscribe: Option<neon::handle::Root<JsFunction>>,
    on_client_unsubscribe: Option<neon::handle::Root<JsFunction>>,
    on_client_authenticate: Option<neon::handle::Root<JsFunction>>,
    on_client_subscribe_authorize: Option<neon::handle::Root<JsFunction>>,
}

impl HookCallbackStorage {
    fn new() -> Self {
        Self {
            channel: None,
            on_message_publish: None,
            on_client_subscribe: None,
            on_client_unsubscribe: None,
            on_client_authenticate: None,
            on_client_subscribe_authorize: None,
        }
    }

    fn clear(&mut self) {
        self.channel = None;
        self.on_message_publish = None;
        self.on_client_subscribe = None;
        self.on_client_unsubscribe = None;
        self.on_client_authenticate = None;
        self.on_client_subscribe_authorize = None;
    }
}

type ServerCallback = Box<dyn FnOnce(&Channel, Deferred) + Send>;

// Authentication result structure for JavaScript callbacks
#[derive(Debug, Clone)]
pub struct AuthenticationResult {
    pub allow: bool,
    pub superuser: bool,
    pub reason: Option<String>,
}

// Subscription authorization result structure from JavaScript
#[derive(Debug, Clone)]
pub struct SubscribeDecision {
    pub allow: bool,
    pub qos: Option<u8>,
    pub reason: Option<String>,
}

// Shared server state to enable publishing and hook callbacks
#[derive(Clone)]
struct SharedServerState {
    context: Arc<Mutex<Option<Arc<ServerContext>>>>,
    is_ready: Arc<Mutex<bool>>,
}

impl SharedServerState {
    fn new() -> Self {
        Self {
            context: Arc::new(Mutex::new(None)),
            is_ready: Arc::new(Mutex::new(false)),
        }
    }

    fn set_context(&self, ctx: Arc<ServerContext>) {
        if let Ok(mut context) = self.context.lock() {
            *context = Some(ctx);
        }
        // Mark as ready
        if let Ok(mut ready) = self.is_ready.lock() {
            *ready = true;
        }
    }

    fn get_context(&self) -> Option<Arc<ServerContext>> {
        if let Ok(context) = self.context.lock() {
            context.clone()
        } else {
            None
        }
    }

    fn is_ready(&self) -> bool {
        if let Ok(ready) = self.is_ready.lock() {
            *ready
        } else {
            false
        }
    }

    async fn wait_for_ready(&self, timeout_ms: u64) -> bool {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);

        loop {
            if self.is_ready() {
                return true;
            }

            if start.elapsed() >= timeout {
                return false;
            }

            // Sleep for a short while before checking again
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    fn clear(&self) {
        if let Ok(mut context) = self.context.lock() {
            *context = None;
        }
        if let Ok(mut ready) = self.is_ready.lock() {
            *ready = false;
        }
    }
}

// Messages sent on the server channel
enum ServerMessage {
    // Start the server with given configuration
    Start(ServerConfig, Deferred, ServerCallback, SharedServerState),
    // Stop the server
    Stop(Deferred, ServerCallback),
    // Publish a message
    Publish {
        topic: String,
        payload: Vec<u8>,
        qos: u8,
        retain: bool,
        deferred: Deferred,
        callback: ServerCallback,
        shared_state: SharedServerState,
    },
    // Signal to close the server thread
    Close,
}

// Hook data structures for TypeScript integration
#[derive(Debug, Clone)]
pub struct HookEventData {
    pub event_type: String,
    pub session: Option<SessionData>,
    pub from: Option<MessageFromData>,
    pub publish: Option<MessagePublishData>,
    pub subscribe: Option<SubscribeData>,
    pub unsubscribe: Option<UnsubscribeData>,
}

#[derive(Debug, Clone)]
pub struct SessionData {
    pub node: u32,
    pub remote_addr: String,
    pub client_id: String,
    pub username: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MessageFromData {
    pub from_type: String,
    pub node: u64,
    pub remote_addr: Option<String>,
    pub client_id: String,
    pub username: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MessagePublishData {
    pub topic: String,
    pub qos: u8,
    pub retain: bool,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SubscribeData {
    pub topic_filter: String,
    pub qos: u8,
}

#[derive(Debug, Clone)]
pub struct UnsubscribeData {
    pub topic_filter: String,
}

// JavaScript hook handler that calls JavaScript callbacks
pub struct JavaScriptHookHandler {
    // Stores references to JavaScript callback functions
}

impl JavaScriptHookHandler {
    pub fn new() -> Self {
        Self {}
    }

    fn call_js_hook(&self, event_type: &str, data: serde_json::Value) {
        if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
            if let Some(channel) = &callbacks.channel {
                // Check if we have the appropriate callback
                let has_callback = match event_type {
                    "message_publish" => callbacks.on_message_publish.is_some(),
                    "client_subscribe" => callbacks.on_client_subscribe.is_some(),
                    "client_unsubscribe" => callbacks.on_client_unsubscribe.is_some(),
                    _ => false,
                };

                if has_callback {
                    let event_type = event_type.to_string();
                    let data_json = data.to_string();

                    let send_result = channel.try_send(move |mut cx| {
                        // Access the global callbacks again inside the JavaScript context
                        if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                            let callback_root = match event_type.as_str() {
                                "message_publish" => &callbacks.on_message_publish,
                                "client_subscribe" => &callbacks.on_client_subscribe,
                                "client_unsubscribe" => &callbacks.on_client_unsubscribe,
                                _ => return Ok(()),
                            };

                            if let Some(callback_root) = callback_root {
                                // Convert the Root back to a JavaScript function
                                let callback = callback_root.to_inner(&mut cx);

                                // Parse the JSON data and create proper parameters for each hook type
                                match event_type.as_str() {
                                    "message_publish" => {
                                        // Parse JSON data for message publish hook
                                        if let Ok(data_value) =
                                            serde_json::from_str::<serde_json::Value>(&data_json)
                                        {
                                            // Create session parameter (not available here yet, use null)
                                            let session = cx.null();

                                            // Create from parameter (simplified for now)
                                            let from = cx.empty_object();
                                            let from_type = cx.string("System");
                                            from.set(&mut cx, "type", from_type)?;

                                            // Create message parameter
                                            let message = cx.empty_object();
                                            if let Some(topic) =
                                                data_value.get("topic").and_then(|v| v.as_str())
                                            {
                                                let topic_val = cx.string(topic);
                                                message.set(&mut cx, "topic", topic_val)?;
                                            }
                                            if let Some(payload) =
                                                data_value.get("payload").and_then(|v| v.as_str())
                                            {
                                                let mut buffer = cx.buffer(payload.len())?;
                                                buffer
                                                    .as_mut_slice(&mut cx)
                                                    .copy_from_slice(payload.as_bytes());
                                                message.set(&mut cx, "payload", buffer)?;
                                            }
                                            if let Some(qos) =
                                                data_value.get("qos").and_then(|v| v.as_u64())
                                            {
                                                let qos_val = cx.number(qos as f64);
                                                message.set(&mut cx, "qos", qos_val)?;
                                            }
                                            if let Some(retain) =
                                                data_value.get("retain").and_then(|v| v.as_bool())
                                            {
                                                let retain_val = cx.boolean(retain);
                                                message.set(&mut cx, "retain", retain_val)?;
                                            }
                                            if let Some(dup) =
                                                data_value.get("dup").and_then(|v| v.as_bool())
                                            {
                                                let dup_val = cx.boolean(dup);
                                                message.set(&mut cx, "dup", dup_val)?;
                                            }
                                            if let Some(create_time) = data_value
                                                .get("createTime")
                                                .and_then(|v| v.as_u64())
                                            {
                                                let create_time_val = cx.number(create_time as f64);
                                                message.set(
                                                    &mut cx,
                                                    "createTime",
                                                    create_time_val,
                                                )?;
                                            }

                                            // Call with three parameters: session, from, message
                                            let _result = callback
                                                .call_with(&cx)
                                                .arg(session)
                                                .arg(from)
                                                .arg(message)
                                                .apply::<JsUndefined, _>(&mut cx);
                                        }
                                    }
                                    "client_subscribe" => {
                                        // Parse JSON data for client subscribe hook
                                        if let Ok(data_value) =
                                            serde_json::from_str::<serde_json::Value>(&data_json)
                                        {
                                            // Create session parameter (not available here yet, use null)
                                            let session = cx.null();

                                            // Create subscription parameter
                                            let subscription = cx.empty_object();
                                            if let Some(topic_filter) = data_value
                                                .get("topicFilter")
                                                .and_then(|v| v.as_str())
                                            {
                                                let topic_filter_val = cx.string(topic_filter);
                                                subscription.set(
                                                    &mut cx,
                                                    "topicFilter",
                                                    topic_filter_val,
                                                )?;
                                            }
                                            if let Some(qos) =
                                                data_value.get("qos").and_then(|v| v.as_u64())
                                            {
                                                let qos_val = cx.number(qos as f64);
                                                subscription.set(&mut cx, "qos", qos_val)?;
                                            }

                                            // Call with two parameters: session, subscription
                                            let _result = callback
                                                .call_with(&cx)
                                                .arg(session)
                                                .arg(subscription)
                                                .apply::<JsUndefined, _>(&mut cx);
                                        }
                                    }
                                    "client_unsubscribe" => {
                                        // Parse JSON data for client unsubscribe hook
                                        if let Ok(data_value) =
                                            serde_json::from_str::<serde_json::Value>(&data_json)
                                        {
                                            // Create session parameter (not available here yet, use null)
                                            let session = cx.null();

                                            // Create unsubscription parameter
                                            let unsubscription = cx.empty_object();
                                            if let Some(topic_filter) = data_value
                                                .get("topicFilter")
                                                .and_then(|v| v.as_str())
                                            {
                                                let topic_filter_val = cx.string(topic_filter);
                                                unsubscription.set(
                                                    &mut cx,
                                                    "topicFilter",
                                                    topic_filter_val,
                                                )?;
                                            }

                                            // Call with two parameters: session, unsubscription
                                            let _result = callback
                                                .call_with(&cx)
                                                .arg(session)
                                                .arg(unsubscription)
                                                .apply::<JsUndefined, _>(&mut cx);
                                        }
                                    }
                                    _ => {
                                        // Fallback to the old single-parameter approach
                                        let data_str = cx.string(data_json);
                                        let _result = callback
                                            .call_with(&cx)
                                            .arg(data_str)
                                            .apply::<JsUndefined, _>(&mut cx);
                                    }
                                }
                            }
                        }
                        Ok(())
                    });
                    let _ = send_result;
                } else {
                    // no-op
                }
            } else {
                // no channel
            }
        } else {
            // lock failure
        }
    }

    async fn call_js_auth_hook(&self, connect_info: &rmqtt::types::ConnectInfo) -> ReturnType {
        // Check if we have an authentication callback registered
        let (has_callback, channel_available) = {
            if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                (
                    callbacks.on_client_authenticate.is_some(),
                    callbacks.channel.is_some(),
                )
            } else {
                (false, false)
            }
        };

        if has_callback && channel_available {
            // Create a oneshot channel to receive the result
            let (tx, rx) = oneshot::channel::<AuthenticationResult>();

            // Prepare the authentication data
            let data = serde_json::json!({
                "clientId": connect_info.id().client_id.to_string(),
                "username": connect_info.username().map(|u| u.to_string()),
                "password": connect_info.password().map(|p| String::from_utf8_lossy(p).to_string()),
                "protocolVersion": connect_info.proto_ver(),
                "remoteAddr": connect_info.id().remote_addr.map(|addr| addr.to_string()).unwrap_or_else(|| "unknown".to_string()),
                "keepAlive": connect_info.keep_alive(),
                "cleanSession": connect_info.clean_start()
            });

            let data_json = data.to_string();

            // Get the channel (ensuring the lock is dropped before await)
            let channel = {
                if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                    callbacks.channel.clone()
                } else {
                    None
                }
            };

            if let Some(channel) = channel {
                // Send the authentication request to JavaScript with the oneshot sender
                let send_result = channel.try_send(move |mut cx| {
                    if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                        if let Some(callback_root) = &callbacks.on_client_authenticate {
                            let callback = callback_root.to_inner(&mut cx);

                            // Parse the JSON data for the authentication request
                            if let Ok(data_value) =
                                serde_json::from_str::<serde_json::Value>(&data_json)
                            {
                                let auth_request = cx.empty_object();

                                // Set all the authentication request fields
                                if let Some(client_id) =
                                    data_value.get("clientId").and_then(|v| v.as_str())
                                {
                                    let client_id_val = cx.string(client_id);
                                    auth_request.set(&mut cx, "clientId", client_id_val)?;
                                }
                                if let Some(username) =
                                    data_value.get("username").and_then(|v| v.as_str())
                                {
                                    let username_val = cx.string(username);
                                    auth_request.set(&mut cx, "username", username_val)?;
                                } else {
                                    let username_val = cx.null();
                                    auth_request.set(&mut cx, "username", username_val)?;
                                }
                                if let Some(password) =
                                    data_value.get("password").and_then(|v| v.as_str())
                                {
                                    let password_val = cx.string(password);
                                    auth_request.set(&mut cx, "password", password_val)?;
                                } else {
                                    let password_val = cx.null();
                                    auth_request.set(&mut cx, "password", password_val)?;
                                }
                                if let Some(protocol_version) =
                                    data_value.get("protocolVersion").and_then(|v| v.as_u64())
                                {
                                    let protocol_version_val = cx.number(protocol_version as f64);
                                    auth_request.set(
                                        &mut cx,
                                        "protocolVersion",
                                        protocol_version_val,
                                    )?;
                                }
                                if let Some(remote_addr) =
                                    data_value.get("remoteAddr").and_then(|v| v.as_str())
                                {
                                    let remote_addr_val = cx.string(remote_addr);
                                    auth_request.set(&mut cx, "remoteAddr", remote_addr_val)?;
                                }
                                if let Some(keep_alive) =
                                    data_value.get("keepAlive").and_then(|v| v.as_u64())
                                {
                                    let keep_alive_val = cx.number(keep_alive as f64);
                                    auth_request.set(&mut cx, "keepAlive", keep_alive_val)?;
                                }
                                if let Some(clean_session) =
                                    data_value.get("cleanSession").and_then(|v| v.as_bool())
                                {
                                    let clean_session_val = cx.boolean(clean_session);
                                    auth_request.set(&mut cx, "cleanSession", clean_session_val)?;
                                }

                                // Call the JavaScript authentication function
                                let result = callback
                                    .call_with(&cx)
                                    .arg(auth_request)
                                    .apply::<JsObject, _>(&mut cx)?;

                                // Extract the authentication result
                                let allow = result
                                    .get::<JsBoolean, _, _>(&mut cx, "allow")?
                                    .value(&mut cx);
                                let superuser = result
                                    .get_opt::<JsBoolean, _, _>(&mut cx, "superuser")?
                                    .map(|b| b.value(&mut cx))
                                    .unwrap_or(false);
                                let reason = result
                                    .get_opt::<JsString, _, _>(&mut cx, "reason")?
                                    .map(|s| s.value(&mut cx));

                                let auth_result = AuthenticationResult {
                                    allow,
                                    superuser,
                                    reason,
                                };

                                // Send the result back through the oneshot channel
                                let _ = tx.send(auth_result);
                            }
                        }
                    }
                    Ok(())
                });

                match send_result {
                    Ok(_) => {
                        // Wait for the JavaScript callback to complete with a timeout
                        match tokio::time::timeout(Duration::from_millis(5000), rx).await {
                            Ok(Ok(auth_result)) => {
                                if auth_result.allow {
                                    // Return success with proper authentication result
                                    return (
                                        false,
                                        Some(HookResult::AuthResult(
                                            rmqtt::types::AuthResult::Allow(
                                                auth_result.superuser,
                                                None,
                                            ),
                                        )),
                                    );
                                } else {
                                    // Return authentication failure
                                    return (
                                        false,
                                        Some(HookResult::AuthResult(
                                            rmqtt::types::AuthResult::BadUsernameOrPassword,
                                        )),
                                    );
                                }
                            }
                            Ok(Err(_)) => {
                                eprintln!(
                                    "WARN: Authentication callback channel error; denying authorization (clientId: {})",
                                    connect_info.id().client_id
                                );
                                return (
                                    false,
                                    Some(HookResult::AuthResult(
                                        rmqtt::types::AuthResult::NotAuthorized,
                                    )),
                                );
                            }
                            Err(_) => {
                                eprintln!(
                                    "WARN: Authentication callback timed out; denying authorization (clientId: {})",
                                    connect_info.id().client_id
                                );
                                return (
                                    false,
                                    Some(HookResult::AuthResult(
                                        rmqtt::types::AuthResult::NotAuthorized,
                                    )),
                                );
                            }
                        }
                    }
                    Err(_e) => {
                        eprintln!(
                            "WARN: Failed to dispatch authentication request to JavaScript; denying authorization (clientId: {})",
                            connect_info.id().client_id
                        );
                        return (
                            false,
                            Some(HookResult::AuthResult(
                                rmqtt::types::AuthResult::NotAuthorized,
                            )),
                        );
                    }
                }
            }
        }
        // No authentication callback registered - let RMQTT handle authentication normally
        return (true, None);
    }

    async fn call_js_subscribe_acl_hook(
        &self,
        session: &rmqtt::session::Session,
        subscribe: &rmqtt::types::Subscribe,
    ) -> ReturnType {
        // Check if we have an authorization callback registered for subscribe ACL
        let (has_callback, channel_available) = {
            if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                (
                    callbacks.on_client_subscribe_authorize.is_some(),
                    callbacks.channel.is_some(),
                )
            } else {
                (false, false)
            }
        };

        if has_callback && channel_available {
            use rmqtt::codec::v5::SubscribeAckReason;
            use rmqtt::types::QoS as RmQos;
            use rmqtt::types::SubscribeAclResult;

            let (tx, rx) = oneshot::channel::<SubscribeDecision>();

            // Prepare subscription info for JS
            let data = serde_json::json!({
                "topicFilter": subscribe.topic_filter.to_string(),
                "qos": subscribe.opts.qos() as u8
            });
            let data_json = data.to_string();

            // Capture session info for JS (remoteAddr, clientId, username)
            let sess_info = {
                let id = session.id();
                serde_json::json!({
                    "remoteAddr": id.remote_addr.map(|a| a.to_string()),
                    "clientId": id.client_id,
                    "username": session.username().map(|u| u.to_string())
                })
            };
            let sess_json = sess_info.to_string();

            let channel = {
                if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                    callbacks.channel.clone()
                } else {
                    None
                }
            };

            if let Some(channel) = channel {
                let send_result = channel.try_send(move |mut cx| {
                    if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                        if let Some(callback_root) = &callbacks.on_client_subscribe_authorize {
                            let callback = callback_root.to_inner(&mut cx);

                            if let Ok(data_value) =
                                serde_json::from_str::<serde_json::Value>(&data_json)
                            {
                                // Build session value (object or null)
                                let session: Handle<JsValue> = if let Ok(sess_val) = serde_json::from_str::<serde_json::Value>(&sess_json) {
                                    let s = cx.empty_object();
                                    // remoteAddr
                                    if let Some(remote) = sess_val.get("remoteAddr").and_then(|v| v.as_str()) {
                                        let v = cx.string(remote);
                                        s.set(&mut cx, "remoteAddr", v)?;
                                    } else {
                                        let v = cx.null();
                                        s.set(&mut cx, "remoteAddr", v)?;
                                    }
                                    // clientId
                                    if let Some(cid) = sess_val.get("clientId").and_then(|v| v.as_str()) {
                                        let v = cx.string(cid);
                                        s.set(&mut cx, "clientId", v)?;
                                    }
                                    // username
                                    if let Some(user) = sess_val.get("username").and_then(|v| v.as_str()) {
                                        let v = cx.string(user);
                                        s.set(&mut cx, "username", v)?;
                                    } else {
                                        let v = cx.null();
                                        s.set(&mut cx, "username", v)?;
                                    }
                                    s.upcast()
                                } else { cx.null().upcast() };
                                let subscription = cx.empty_object();
                                if let Some(topic_filter) = data_value
                                    .get("topicFilter")
                                    .and_then(|v| v.as_str())
                                {
                                    let v = cx.string(topic_filter);
                                    subscription.set(&mut cx, "topicFilter", v)?;
                                }
                                if let Some(qos) = data_value.get("qos").and_then(|v| v.as_u64()) {
                                    let v = cx.number(qos as f64);
                                    subscription.set(&mut cx, "qos", v)?;
                                }

                                // Call the JS function expecting { allow: boolean, qos?: number, reason?: string }
                                let result = callback
                                    .call_with(&cx)
                                    .arg(session)
                                    .arg(subscription)
                                    .apply::<JsObject, _>(&mut cx)?;

                                let allow = result
                                    .get::<JsBoolean, _, _>(&mut cx, "allow")?
                                    .value(&mut cx);
                                let qos = result
                                    .get_opt::<JsNumber, _, _>(&mut cx, "qos")?
                                    .map(|n| n.value(&mut cx) as u8);
                                let reason = result
                                    .get_opt::<JsString, _, _>(&mut cx, "reason")?
                                    .map(|s| s.value(&mut cx));

                                let _ = tx.send(SubscribeDecision { allow, qos, reason });
                            }
                        }
                    }
                    Ok(())
                });

                match send_result {
                    Ok(_) => match tokio::time::timeout(Duration::from_millis(5000), rx).await {
                        Ok(Ok(decision)) => {
                            if decision.allow {
                                let qos_num = decision.qos.unwrap_or(subscribe.opts.qos() as u8);
                                let qos = match RmQos::try_from(qos_num) {
                                    Ok(q) => q,
                                    Err(_) => subscribe.opts.qos(),
                                };
                                return (
                                    false,
                                    Some(HookResult::SubscribeAclResult(SubscribeAclResult::new_success(qos, None))),
                                );
                            } else {
                                return (
                                    false,
                                    Some(HookResult::SubscribeAclResult(SubscribeAclResult::new_failure(
                                        SubscribeAckReason::NotAuthorized,
                                    ))),
                                );
                            }
                        }
                        Ok(Err(_)) => {
                            eprintln!(
                                "WARN: Subscribe ACL callback channel error; denying subscription"
                            );
                            return (
                                false,
                                Some(HookResult::SubscribeAclResult(SubscribeAclResult::new_failure(
                                    SubscribeAckReason::NotAuthorized,
                                ))),
                            );
                        }
                        Err(_) => {
                            eprintln!(
                                "WARN: Subscribe ACL callback timed out; denying subscription"
                            );
                            return (
                                false,
                                Some(HookResult::SubscribeAclResult(SubscribeAclResult::new_failure(
                                    SubscribeAckReason::NotAuthorized,
                                ))),
                            );
                        }
                    },
                    Err(_) => {
                        eprintln!(
                            "WARN: Failed to dispatch subscribe ACL request to JavaScript; denying subscription"
                        );
                        return (
                            false,
                            Some(HookResult::SubscribeAclResult(SubscribeAclResult::new_failure(
                                SubscribeAckReason::NotAuthorized,
                            ))),
                        );
                    }
                }
            }
        }

        // No JS hook registered; proceed and let RMQTT/other ACL decide
        (true, None)
    }
}

#[async_trait]
impl Handler for JavaScriptHookHandler {
    async fn hook(&self, param: &Parameter, acc: Option<HookResult>) -> ReturnType {
        match param {
            Parameter::MessagePublish(_session, _from, publish) => {
                // Call JavaScript callback if registered
                let data = serde_json::json!({
                    "topic": publish.topic.to_string(),
                    "payload": String::from_utf8_lossy(&publish.payload),
                    "qos": publish.qos as u8,
                    "retain": publish.retain,
                    "dup": publish.dup,
                    "createTime": publish.create_time.unwrap_or(0)
                });
                self.call_js_hook("message_publish", data);
            }
            Parameter::ClientSubscribe(_session, subscribe) => {
                // Call JavaScript callback if registered
                let data = serde_json::json!({
                    "topicFilter": subscribe.topic_filter.to_string(),
                    "qos": subscribe.opts.qos() as u8
                });
                self.call_js_hook("client_subscribe", data);
            }
            Parameter::ClientSubscribeCheckAcl(session, subscribe) => {
                return self.call_js_subscribe_acl_hook(session, subscribe).await;
            }
            Parameter::ClientUnsubscribe(_session, unsubscribe) => {
                // Call JavaScript callback if registered
                let data = serde_json::json!({
                    "topicFilter": unsubscribe.topic_filter.to_string()
                });
                self.call_js_hook("client_unsubscribe", data);
            }
            Parameter::ClientAuthenticate(connect_info) => {
                // For authentication, we need to synchronously call JavaScript and wait for result
                return self.call_js_auth_hook(connect_info).await;
            }
            _ => {
                // For now, ignore other hook types
            }
        }
        (true, acc) // Continue hook chain
    }
}

// Wraps an RMQTT server instance with async communication
struct MqttServerWrapper {
    tx: mpsc::Sender<ServerMessage>,
    shared_state: SharedServerState,
}

#[derive(Debug, Clone)]
struct ListenerConfig {
    name: String,
    address: String,
    port: u16,
    protocol: String, // "tcp", "tls", "ws", "wss"
    tls_cert: Option<String>,
    tls_key: Option<String>,
    allow_anonymous: bool,
}

#[derive(Debug, Clone)]
struct ServerConfig {
    listeners: Vec<ListenerConfig>,
    plugins_config_dir: Option<String>,
    plugins_config: HashMap<String, String>,
    plugins_default_startups: Option<Vec<String>>,
}

impl Finalize for MqttServerWrapper {}

impl MqttServerWrapper {
    fn new<'a, C>(cx: &mut C) -> neon::result::NeonResult<Self>
    where
        C: Context<'a>,
    {
        let (tx, rx) = mpsc::channel::<ServerMessage>();
        let channel = cx.channel();
        let shared_state = SharedServerState::new();
        let shared_state_clone = shared_state.clone();

        // Spawn a thread for managing the MQTT server
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
            let mut server_handle: Option<tokio::task::JoinHandle<RmqttResult<()>>> = None;

            while let Ok(message) = rx.recv() {
                match message {
                    ServerMessage::Start(config, deferred, callback, shared_state_for_start) => {
                        // Stop existing server if running
                        if let Some(handle) = server_handle.take() {
                            handle.abort();
                        }

                        // Start new server
                        let server_config = config.clone();
                        server_handle = Some(rt.spawn(async move {
                            Self::start_server(server_config, shared_state_for_start).await
                        }));

                        callback(&channel, deferred);
                    }
                    ServerMessage::Stop(deferred, callback) => {
                        if let Some(handle) = server_handle.take() {
                            handle.abort();
                        }
                        // Clear the context when stopping
                        shared_state_clone.clear();
                        callback(&channel, deferred);
                    }
                    ServerMessage::Publish {
                        topic,
                        payload,
                        qos,
                        retain,
                        deferred,
                        callback,
                        shared_state,
                    } => {
                        let topic_clone = topic.clone();
                        let payload_clone = payload.clone();

                        rt.spawn(async move {
                            Self::handle_publish(
                                shared_state,
                                topic_clone,
                                payload_clone,
                                qos,
                                retain,
                            )
                            .await;
                        });

                        callback(&channel, deferred);
                    }
                    ServerMessage::Close => break,
                }
            }
        });

        Ok(Self { tx, shared_state })
    }

    async fn handle_publish(
        shared_state: SharedServerState,
        topic: String,
        payload: Vec<u8>,
        qos: u8,
        retain: bool,
    ) {
        // Wait for the server context to be ready with a 5-second timeout
        if !shared_state.wait_for_ready(5000).await {
            eprintln!(
                "WARN: Publish dropped because server context was not ready within 5s (topic: {})",
                topic
            );
            return;
        }

        if let Some(context) = shared_state.get_context() {
            // Create a proper RMQTT Publish message
            let qos_num = qos;
            let qos = match QoS::try_from(qos_num) {
                Ok(qos) => qos,
                Err(_) => {
                    eprintln!(
                        "ERROR: Invalid QoS value {} for topic {}. Dropping publish.",
                        qos_num, topic
                    );
                    return;
                }
            };

            // Create a low-level CodecPublish first
            let codec_publish = CodecPublish {
                dup: false,
                retain,
                qos,
                topic: topic.clone().into(),
                packet_id: None,
                payload: payload.into(),
                properties: None, // Simplify by not setting properties
            };

            // Convert to high-level Publish with metadata and set create_time
            let mut publish =
                Publish::from(Box::new(codec_publish)).create_time(timestamp_millis());

            // Create a synthetic "from" representing the system/API using proper RMQTT API
            let id = Id::new(
                1,
                0,
                None,
                "127.0.0.1:0".parse().ok(),
                "js-api".into(),
                None,
            );
            let from = From::from_system(id);

            // First call the hook manager to trigger message_publish hooks - this is what we were missing!
            publish = context
                .extends
                .hook_mgr()
                .message_publish(None, from.clone(), &publish)
                .await
                .unwrap_or(publish);

            // Then use SessionState::forwards to deliver the message
            let message_storage_available = context.extends.message_mgr().await.enable();
            let message_expiry_interval = Some(Duration::from_secs(3600)); // 1 hour default

            match SessionState::forwards(
                &context,
                from.clone(),
                publish.clone(),
                message_storage_available,
                message_expiry_interval,
            )
            .await
            {
                Ok(()) => {}
                Err(e) => {
                    eprintln!(
                        "ERROR: Failed to forward message to topic {}: {:?}",
                        topic, e
                    );
                }
            }
        } else {
            // context not available
            eprintln!(
                "WARN: Publish dropped: no server context available (topic: {})",
                topic
            );
        }
    }

    async fn start_server(
        config: ServerConfig,
        shared_state: SharedServerState,
    ) -> RmqttResult<()> {
        let mut context_builder = ServerContext::new();

        if let Some(plugins_dir) = config.plugins_config_dir {
            context_builder = context_builder.plugins_config_dir(&plugins_dir);
        }

        for (plugin_name, plugin_config) in config.plugins_config {
            context_builder = context_builder.plugins_config_map_add(&plugin_name, &plugin_config);
        }

        let scx = Arc::new(context_builder.build().await);

        // Register plugins if specified in configuration
        if let Some(plugins) = config.plugins_default_startups {
            for plugin_name in plugins {
                match plugin_name.as_str() {
                    _ => {}
                }
            }
        }

        // Store the context in shared state for publishing
        shared_state.set_context(scx.clone());

        // Register our JavaScript hook handler
        let hook_register = scx.extends.hook_mgr().register();
        hook_register
            .add(
                Type::ClientAuthenticate,
                Box::new(JavaScriptHookHandler::new()),
            )
            .await;
        hook_register
            .add(Type::MessagePublish, Box::new(JavaScriptHookHandler::new()))
            .await;
        hook_register
            .add(
                Type::ClientSubscribeCheckAcl,
                Box::new(JavaScriptHookHandler::new()),
            )
            .await;
        hook_register
            .add(
                Type::ClientSubscribe,
                Box::new(JavaScriptHookHandler::new()),
            )
            .await;
        hook_register
            .add(
                Type::ClientUnsubscribe,
                Box::new(JavaScriptHookHandler::new()),
            )
            .await;

        // IMPORTANT: Start the hook register to enable the handlers
        hook_register.start().await;

        let mut server_builder = RmqttServer::new(scx.as_ref().clone());

        for listener_config in config.listeners {
            let mut builder = Builder::new()
                .name(&listener_config.name)
                .laddr(
                    (
                        listener_config
                            .address
                            .parse::<std::net::IpAddr>()
                            .unwrap_or_else(|_| "0.0.0.0".parse().unwrap()),
                        listener_config.port,
                    )
                        .into(),
                )
                .allow_anonymous(listener_config.allow_anonymous);

            if let Some(cert) = listener_config.tls_cert {
                builder = builder.tls_cert(Some(&cert));
            }
            if let Some(key) = listener_config.tls_key {
                builder = builder.tls_key(Some(&key));
            }

            let bound_builder = builder.bind()?;

            let listener = match listener_config.protocol.as_str() {
                "tcp" => bound_builder.tcp()?,
                "tls" => bound_builder.tls()?,
                "ws" => bound_builder.ws()?,
                "wss" => bound_builder.wss()?,
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("Invalid protocol: {}", listener_config.protocol),
                    )
                    .into());
                }
            };

            server_builder = server_builder.listener(listener);
        }

        server_builder.build().run().await
    }

    fn close(&self) -> Result<(), mpsc::SendError<ServerMessage>> {
        // Clear hook callbacks to release Channel references
        if let Ok(mut callbacks) = HOOK_CALLBACKS.lock() {
            callbacks.clear();
        }

        self.tx.send(ServerMessage::Close)
    }

    // removed unused send helper

    fn start_with_config(
        &self,
        config: ServerConfig,
        deferred: Deferred,
        callback: impl FnOnce(&Channel, Deferred) + Send + 'static,
    ) -> Result<(), mpsc::SendError<ServerMessage>> {
        self.tx.send(ServerMessage::Start(
            config,
            deferred,
            Box::new(callback),
            self.shared_state.clone(),
        ))
    }

    fn stop_server(
        &self,
        deferred: Deferred,
        callback: impl FnOnce(&Channel, Deferred) + Send + 'static,
    ) -> Result<(), mpsc::SendError<ServerMessage>> {
        self.tx
            .send(ServerMessage::Stop(deferred, Box::new(callback)))
    }

    fn publish_message(
        &self,
        topic: String,
        payload: Vec<u8>,
        qos: u8,
        retain: bool,
        deferred: Deferred,
        callback: impl FnOnce(&Channel, Deferred) + Send + 'static,
    ) -> Result<(), mpsc::SendError<ServerMessage>> {
        self.tx.send(ServerMessage::Publish {
            topic,
            payload,
            qos,
            retain,
            deferred,
            callback: Box::new(callback),
            shared_state: self.shared_state.clone(),
        })
    }
}

// JavaScript-exposed methods
impl MqttServerWrapper {
    fn js_new(mut cx: FunctionContext) -> JsResult<JsBox<MqttServerWrapper>> {
        let server = MqttServerWrapper::new(&mut cx)
            .or_else(|err| cx.throw_error(format!("Failed to create MQTT server: {}", err)))?;

        Ok(cx.boxed(server))
    }

    fn js_close(mut cx: FunctionContext) -> JsResult<JsUndefined> {
        cx.this::<JsBox<MqttServerWrapper>>()?
            .close()
            .or_else(|err| cx.throw_error(err.to_string()))?;

        Ok(cx.undefined())
    }

    fn js_start(mut cx: FunctionContext) -> JsResult<JsPromise> {
        let config_obj = cx.argument::<JsObject>(0)?;
        let config = Self::parse_config(&mut cx, config_obj)?;

        let server = cx.this::<JsBox<MqttServerWrapper>>()?;
        let (deferred, promise) = cx.promise();

        server
            .start_with_config(config, deferred, |channel, deferred| {
                deferred.settle_with(channel, |mut cx| Ok(cx.undefined()));
            })
            .into_rejection(&mut cx)?;

        Ok(promise)
    }

    fn js_stop(mut cx: FunctionContext) -> JsResult<JsPromise> {
        let server = cx.this::<JsBox<MqttServerWrapper>>()?;
        let (deferred, promise) = cx.promise();

        server
            .stop_server(deferred, |channel, deferred| {
                deferred.settle_with(channel, |mut cx| Ok(cx.undefined()));
            })
            .into_rejection(&mut cx)?;

        Ok(promise)
    }

    fn js_publish(mut cx: FunctionContext) -> JsResult<JsPromise> {
        let topic = cx.argument::<JsString>(0)?.value(&mut cx);
        let payload_arg = cx.argument::<JsBuffer>(1)?;
        let payload = payload_arg.as_slice(&mut cx).to_vec();
        let qos = cx.argument::<JsNumber>(2)?.value(&mut cx) as u8;
        let retain = cx.argument::<JsBoolean>(3)?.value(&mut cx);

        let server = cx.this::<JsBox<MqttServerWrapper>>()?;
        let (deferred, promise) = cx.promise();

        server
            .publish_message(
                topic,
                payload,
                qos,
                retain,
                deferred,
                |channel, deferred| {
                    deferred.settle_with(channel, |mut cx| Ok(cx.undefined()));
                },
            )
            .into_rejection(&mut cx)?;

        Ok(promise)
    }

    fn parse_config(
        cx: &mut FunctionContext,
        config_obj: Handle<JsObject>,
    ) -> NeonResult<ServerConfig> {
        let listeners_array = config_obj.get::<JsArray, _, _>(cx, "listeners")?;
        let mut listeners = Vec::new();

        for i in 0..listeners_array.len(cx) {
            let listener_obj = listeners_array.get::<JsObject, _, _>(cx, i)?;

            let name = listener_obj.get::<JsString, _, _>(cx, "name")?.value(cx);
            let address = listener_obj.get::<JsString, _, _>(cx, "address")?.value(cx);
            let port = listener_obj.get::<JsNumber, _, _>(cx, "port")?.value(cx) as u16;
            let protocol = listener_obj
                .get::<JsString, _, _>(cx, "protocol")?
                .value(cx);

            let tls_cert = listener_obj
                .get_opt::<JsString, _, _>(cx, "tlsCert")?
                .map(|s| s.value(cx));
            let tls_key = listener_obj
                .get_opt::<JsString, _, _>(cx, "tlsKey")?
                .map(|s| s.value(cx));
            let allow_anonymous = listener_obj
                .get_opt::<JsBoolean, _, _>(cx, "allowAnonymous")?
                .map(|b| b.value(cx))
                .unwrap_or(true);

            listeners.push(ListenerConfig {
                name,
                address,
                port,
                protocol,
                tls_cert,
                tls_key,
                allow_anonymous,
            });
        }

        let plugins_config_dir = config_obj
            .get_opt::<JsString, _, _>(cx, "pluginsConfigDir")?
            .map(|s| s.value(cx));

        let plugins_default_startups = config_obj
            .get_opt::<JsArray, _, _>(cx, "pluginsDefaultStartups")?
            .map(|arr| {
                let length = arr.len(cx);
                let mut startups = Vec::new();
                for i in 0..length {
                    if let Ok(plugin_name) = arr.get::<JsString, _, _>(cx, i) {
                        startups.push(plugin_name.value(cx));
                    }
                }
                startups
            });

        let plugins_config = HashMap::new(); // TODO: Parse plugins config from JS object

        Ok(ServerConfig {
            listeners,
            plugins_config_dir,
            plugins_config,
            plugins_default_startups,
        })
    }

    fn js_set_hooks(mut cx: FunctionContext) -> JsResult<JsUndefined> {
        let hooks_obj = cx.argument::<JsObject>(0)?;

        // Extract callback functions from the hooks object
        let channel = cx.channel();

        if let Ok(mut callbacks) = HOOK_CALLBACKS.lock() {
            // Store the channel for sending events to JavaScript
            callbacks.channel = Some(channel);

            // Try to extract callback functions - use get_opt to avoid errors for missing properties
            if let Ok(Some(on_message_publish)) =
                hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onMessagePublish")
            {
                callbacks.on_message_publish = Some(on_message_publish.root(&mut cx));
            }

            if let Ok(Some(on_client_subscribe)) =
                hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onClientSubscribe")
            {
                callbacks.on_client_subscribe = Some(on_client_subscribe.root(&mut cx));
            }

            if let Ok(Some(on_client_unsubscribe)) =
                hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onClientUnsubscribe")
            {
                callbacks.on_client_unsubscribe = Some(on_client_unsubscribe.root(&mut cx));
            }

            if let Ok(Some(on_client_authenticate)) =
                hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onClientAuthenticate")
            {
                callbacks.on_client_authenticate = Some(on_client_authenticate.root(&mut cx));
            }

            if let Ok(Some(on_client_subscribe_authorize)) =
                hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onClientSubscribeAuthorize")
            {
                callbacks.on_client_subscribe_authorize =
                    Some(on_client_subscribe_authorize.root(&mut cx));
            }
        }

        Ok(cx.undefined())
    }
}

trait SendResultExt {
    fn into_rejection<'a, C: Context<'a>>(self, cx: &mut C) -> NeonResult<()>;
}

impl SendResultExt for Result<(), mpsc::SendError<ServerMessage>> {
    fn into_rejection<'a, C: Context<'a>>(self, cx: &mut C) -> NeonResult<()> {
        self.or_else(|err| {
            let msg = err.to_string();
            eprintln!("ERROR: Internal server message send failed: {}", msg);
            match err.0 {
                ServerMessage::Start(_, deferred, _, _)
                | ServerMessage::Stop(deferred, _)
                | ServerMessage::Publish { deferred, .. } => {
                    let err = cx.error(msg)?;
                    deferred.reject(cx, err);
                    Ok(())
                }
                ServerMessage::Close => cx.throw_error("Expected server message with deferred"),
            }
        })
    }
}

#[neon::main]
fn main(mut cx: ModuleContext) -> NeonResult<()> {
    cx.export_function("mqttServerNew", MqttServerWrapper::js_new)?;
    cx.export_function("mqttServerClose", MqttServerWrapper::js_close)?;
    cx.export_function("mqttServerStart", MqttServerWrapper::js_start)?;
    cx.export_function("mqttServerStop", MqttServerWrapper::js_stop)?;
    cx.export_function("mqttServerPublish", MqttServerWrapper::js_publish)?;
    cx.export_function("mqttServerSetHooks", MqttServerWrapper::js_set_hooks)?;

    Ok(())
}
