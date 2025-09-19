use std::time::Duration;

use async_trait::async_trait;
use neon::{event::Channel, prelude::*, types::buffer::TypedArray};
use once_cell::sync::Lazy;
use rmqtt::{
    hook::{Handler, HookResult, Parameter, ReturnType},
    types::QoS as RmQos,
};
use tokio::sync::oneshot;

// Global storage for JavaScript hook callbacks
pub(crate) static HOOK_CALLBACKS: Lazy<std::sync::Mutex<HookCallbackStorage>> =
    Lazy::new(|| std::sync::Mutex::new(HookCallbackStorage::new()));

pub(crate) struct HookCallbackStorage {
    pub channel: Option<Channel>,
    pub on_message_publish: Option<neon::handle::Root<JsFunction>>,
    pub on_client_subscribe: Option<neon::handle::Root<JsFunction>>,
    pub on_client_unsubscribe: Option<neon::handle::Root<JsFunction>>,
    pub on_client_authenticate: Option<neon::handle::Root<JsFunction>>,
    pub on_client_subscribe_authorize: Option<neon::handle::Root<JsFunction>>,
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

    pub fn clear(&mut self) {
        self.channel = None;
        self.on_message_publish = None;
        self.on_client_subscribe = None;
        self.on_client_unsubscribe = None;
        self.on_client_authenticate = None;
        self.on_client_subscribe_authorize = None;
    }
}

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

// Removed unused hook data structures to keep module lean

// JavaScript hook handler that calls JavaScript callbacks
pub struct JavaScriptHookHandler {}

impl JavaScriptHookHandler {
    pub fn new() -> Self {
        Self {}
    }

    fn call_js_hook(&self, event_type: &str, data: serde_json::Value) {
        if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
            if let Some(channel) = &callbacks.channel {
                let has_callback = match event_type {
                    "message_publish" => callbacks.on_message_publish.is_some(),
                    "client_subscribe" => callbacks.on_client_subscribe.is_some(),
                    "client_unsubscribe" => callbacks.on_client_unsubscribe.is_some(),
                    _ => false,
                };

                if has_callback {
                    let event_type = event_type.to_string();
                    let data_json = data.to_string();

                    let _ = channel.try_send(move |mut cx| {
                        if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                            let callback_root = match event_type.as_str() {
                                "message_publish" => &callbacks.on_message_publish,
                                "client_subscribe" => &callbacks.on_client_subscribe,
                                "client_unsubscribe" => &callbacks.on_client_unsubscribe,
                                _ => return Ok(()),
                            };

                            if let Some(callback_root) = callback_root {
                                let callback = callback_root.to_inner(&mut cx);

                                match event_type.as_str() {
                                    "message_publish" => {
                                        if let Ok(data_value) = serde_json::from_str::<serde_json::Value>(&data_json) {
                                            let session = cx.null();
                                            let from = cx.empty_object();
                                            let from_type = cx.string("System");
                                            from.set(&mut cx, "type", from_type)?;

                                            let message = cx.empty_object();
                                            if let Some(topic) = data_value.get("topic").and_then(|v| v.as_str()) {
                                                let js = cx.string(topic);
                                                message.set(&mut cx, "topic", js)?;
                                            }
                                            if let Some(payload) = data_value.get("payload").and_then(|v| v.as_str()) {
                                                let mut buffer = cx.buffer(payload.len())?;
                                                buffer.as_mut_slice(&mut cx).copy_from_slice(payload.as_bytes());
                                                message.set(&mut cx, "payload", buffer)?;
                                            }
                                            if let Some(qos) = data_value.get("qos").and_then(|v| v.as_u64()) {
                                                let js = cx.number(qos as f64);
                                                message.set(&mut cx, "qos", js)?;
                                            }
                                            if let Some(retain) = data_value.get("retain").and_then(|v| v.as_bool()) {
                                                let js = cx.boolean(retain);
                                                message.set(&mut cx, "retain", js)?;
                                            }
                                            if let Some(dup) = data_value.get("dup").and_then(|v| v.as_bool()) {
                                                let js = cx.boolean(dup);
                                                message.set(&mut cx, "dup", js)?;
                                            }
                                            if let Some(create_time) = data_value.get("createTime").and_then(|v| v.as_u64()) {
                                                let js = cx.number(create_time as f64);
                                                message.set(&mut cx, "createTime", js)?;
                                            }

                                            let _ = callback
                                                .call_with(&cx)
                                                .arg(session)
                                                .arg(from)
                                                .arg(message)
                                                .apply::<JsUndefined, _>(&mut cx);
                                        }
                                    }
                                    "client_subscribe" => {
                                        if let Ok(data_value) = serde_json::from_str::<serde_json::Value>(&data_json) {
                                            let session = cx.null();
                                            let subscription = cx.empty_object();
                                            if let Some(topic_filter) = data_value.get("topicFilter").and_then(|v| v.as_str()) {
                                                let js = cx.string(topic_filter);
                                                subscription.set(&mut cx, "topicFilter", js)?;
                                            }
                                            if let Some(qos) = data_value.get("qos").and_then(|v| v.as_u64()) {
                                                let js = cx.number(qos as f64);
                                                subscription.set(&mut cx, "qos", js)?;
                                            }
                                            let _ = callback
                                                .call_with(&cx)
                                                .arg(session)
                                                .arg(subscription)
                                                .apply::<JsUndefined, _>(&mut cx);
                                        }
                                    }
                                    "client_unsubscribe" => {
                                        if let Ok(data_value) = serde_json::from_str::<serde_json::Value>(&data_json) {
                                            let session = cx.null();
                                            let unsubscription = cx.empty_object();
                                            if let Some(topic_filter) = data_value.get("topicFilter").and_then(|v| v.as_str()) {
                                                let js = cx.string(topic_filter);
                                                unsubscription.set(&mut cx, "topicFilter", js)?;
                                            }
                                            let _ = callback
                                                .call_with(&cx)
                                                .arg(session)
                                                .arg(unsubscription)
                                                .apply::<JsUndefined, _>(&mut cx);
                                        }
                                    }
                                    _ => {
                                        let data_str = cx.string(data_json);
                                        let _ = callback.call_with(&cx).arg(data_str).apply::<JsUndefined, _>(&mut cx);
                                    }
                                }
                            }
                        }
                        Ok(())
                    });
                }
            }
        }
    }

    async fn call_js_auth_hook(&self, connect_info: &rmqtt::types::ConnectInfo) -> ReturnType {
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
            let (tx, rx) = oneshot::channel::<AuthenticationResult>();

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

            let channel = {
                if let Ok(callbacks) = HOOK_CALLBACKS.lock() { callbacks.channel.clone() } else { None }
            };

            if let Some(channel) = channel {
                let send_result = channel.try_send(move |mut cx| {
                    if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                        if let Some(callback_root) = &callbacks.on_client_authenticate {
                            let callback = callback_root.to_inner(&mut cx);
                            if let Ok(_data_value) = serde_json::from_str::<serde_json::Value>(&data_json) {
                                let auth_request = cx.empty_object();
                                // directly set fields from parsed JSON for performance
                                let v: serde_json::Value = serde_json::from_str(&data_json).unwrap_or_default();
                                if let Some(s) = v.get("clientId").and_then(|s| s.as_str()) {
                                    let js = cx.string(s);
                                    auth_request.set(&mut cx, "clientId", js)?;
                                }
                                if let Some(s) = v.get("username").and_then(|s| s.as_str()) {
                                    let js = cx.string(s);
                                    auth_request.set(&mut cx, "username", js)?;
                                } else {
                                    let js = cx.null();
                                    auth_request.set(&mut cx, "username", js)?;
                                }
                                if let Some(s) = v.get("password").and_then(|s| s.as_str()) {
                                    let js = cx.string(s);
                                    auth_request.set(&mut cx, "password", js)?;
                                } else {
                                    let js = cx.null();
                                    auth_request.set(&mut cx, "password", js)?;
                                }
                                if let Some(n) = v.get("protocolVersion").and_then(|n| n.as_u64()) {
                                    let js = cx.number(n as f64);
                                    auth_request.set(&mut cx, "protocolVersion", js)?;
                                }
                                if let Some(s) = v.get("remoteAddr").and_then(|s| s.as_str()) {
                                    let js = cx.string(s);
                                    auth_request.set(&mut cx, "remoteAddr", js)?;
                                }
                                if let Some(n) = v.get("keepAlive").and_then(|n| n.as_u64()) {
                                    let js = cx.number(n as f64);
                                    auth_request.set(&mut cx, "keepAlive", js)?;
                                }
                                if let Some(b) = v.get("cleanSession").and_then(|b| b.as_bool()) {
                                    let js = cx.boolean(b);
                                    auth_request.set(&mut cx, "cleanSession", js)?;
                                }

                                let result = callback.call_with(&cx).arg(auth_request).apply::<JsObject, _>(&mut cx)?;

                                let allow = result.get::<JsBoolean, _, _>(&mut cx, "allow")?.value(&mut cx);
                                let superuser = result.get_opt::<JsBoolean, _, _>(&mut cx, "superuser")?.map(|b| b.value(&mut cx)).unwrap_or(false);
                                let reason = result.get_opt::<JsString, _, _>(&mut cx, "reason")?.map(|s| s.value(&mut cx));

                                let _ = tx.send(AuthenticationResult { allow, superuser, reason });
                            }
                        }
                    }
                    Ok(())
                });

                match send_result {
                    Ok(_) => match tokio::time::timeout(Duration::from_millis(5000), rx).await {
                        Ok(Ok(auth_result)) => {
                            if auth_result.allow {
                                return (
                                    false,
                                    Some(HookResult::AuthResult(rmqtt::types::AuthResult::Allow(auth_result.superuser, None))),
                                );
                            } else {
                                return (
                                    false,
                                    Some(HookResult::AuthResult(rmqtt::types::AuthResult::BadUsernameOrPassword)),
                                );
                            }
                        }
                        Ok(Err(_)) => {
                            eprintln!("WARN: Authentication callback channel error; denying authorization (clientId: {})", connect_info.id().client_id);
                            return (false, Some(HookResult::AuthResult(rmqtt::types::AuthResult::NotAuthorized)));
                        }
                        Err(_) => {
                            eprintln!("WARN: Authentication callback timed out; denying authorization (clientId: {})", connect_info.id().client_id);
                            return (false, Some(HookResult::AuthResult(rmqtt::types::AuthResult::NotAuthorized)));
                        }
                    },
                    Err(_) => {
                        eprintln!("WARN: Failed to dispatch authentication request to JavaScript; denying authorization (clientId: {})", connect_info.id().client_id);
                        return (false, Some(HookResult::AuthResult(rmqtt::types::AuthResult::NotAuthorized)));
                    }
                }
            }
        }
        (true, None)
    }

    async fn call_js_subscribe_acl_hook(
        &self,
        session: &rmqtt::session::Session,
        subscribe: &rmqtt::types::Subscribe,
    ) -> ReturnType {
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
            use rmqtt::types::SubscribeAclResult;

            let (tx, rx) = oneshot::channel::<SubscribeDecision>();

            let data = serde_json::json!({
                "topicFilter": subscribe.topic_filter.to_string(),
                "qos": subscribe.opts.qos() as u8
            });
            let data_json = data.to_string();

            let sess_info = {
                let id = session.id();
                serde_json::json!({
                    "remoteAddr": id.remote_addr.map(|a| a.to_string()),
                    "clientId": id.client_id,
                    "username": session.username().map(|u| u.to_string())
                })
            };
            let sess_json = sess_info.to_string();

            let channel = { if let Ok(callbacks) = HOOK_CALLBACKS.lock() { callbacks.channel.clone() } else { None } };

            if let Some(channel) = channel {
                let send_result = channel.try_send(move |mut cx| {
                    if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                        if let Some(callback_root) = &callbacks.on_client_subscribe_authorize {
                            let callback = callback_root.to_inner(&mut cx);
                            if let Ok(data_value) = serde_json::from_str::<serde_json::Value>(&data_json) {
                                let session: Handle<JsValue> = if let Ok(sess_val) = serde_json::from_str::<serde_json::Value>(&sess_json) {
                                    let s = cx.empty_object();
                                    if let Some(remote) = sess_val.get("remoteAddr").and_then(|v| v.as_str()) {
                                        let js = cx.string(remote);
                                        s.set(&mut cx, "remoteAddr", js)?;
                                    } else {
                                        let js = cx.null();
                                        s.set(&mut cx, "remoteAddr", js)?;
                                    }
                                    if let Some(cid) = sess_val.get("clientId").and_then(|v| v.as_str()) {
                                        let js = cx.string(cid);
                                        s.set(&mut cx, "clientId", js)?;
                                    }
                                    if let Some(user) = sess_val.get("username").and_then(|v| v.as_str()) {
                                        let js = cx.string(user);
                                        s.set(&mut cx, "username", js)?;
                                    } else {
                                        let js = cx.null();
                                        s.set(&mut cx, "username", js)?;
                                    }
                                    s.upcast()
                                } else { cx.null().upcast() };

                                let subscription = cx.empty_object();
                                if let Some(topic_filter) = data_value.get("topicFilter").and_then(|v| v.as_str()) {
                                    let js = cx.string(topic_filter);
                                    subscription.set(&mut cx, "topicFilter", js)?;
                                }
                                if let Some(qos) = data_value.get("qos").and_then(|v| v.as_u64()) {
                                    let js = cx.number(qos as f64);
                                    subscription.set(&mut cx, "qos", js)?;
                                }

                                let result = callback.call_with(&cx).arg(session).arg(subscription).apply::<JsObject, _>(&mut cx)?;
                                let allow = result.get::<JsBoolean, _, _>(&mut cx, "allow")?.value(&mut cx);
                                let qos = result.get_opt::<JsNumber, _, _>(&mut cx, "qos")?.map(|n| n.value(&mut cx) as u8);
                                let reason = result.get_opt::<JsString, _, _>(&mut cx, "reason")?.map(|s| s.value(&mut cx));

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
                                let qos = match RmQos::try_from(qos_num) { Ok(q) => q, Err(_) => subscribe.opts.qos() };
                                return (false, Some(HookResult::SubscribeAclResult(SubscribeAclResult::new_success(qos, None))));
                            } else {
                                return (false, Some(HookResult::SubscribeAclResult(SubscribeAclResult::new_failure(SubscribeAckReason::NotAuthorized))));
                            }
                        }
                        Ok(Err(_)) => {
                            eprintln!("WARN: Subscribe ACL callback channel error; denying subscription");
                            return (false, Some(HookResult::SubscribeAclResult(SubscribeAclResult::new_failure(rmqtt::codec::v5::SubscribeAckReason::NotAuthorized))));
                        }
                        Err(_) => {
                            eprintln!("WARN: Subscribe ACL callback timed out; denying subscription");
                            return (false, Some(HookResult::SubscribeAclResult(SubscribeAclResult::new_failure(rmqtt::codec::v5::SubscribeAckReason::NotAuthorized))));
                        }
                    },
                    Err(_) => {
                        eprintln!("WARN: Failed to dispatch subscribe ACL request to JavaScript; denying subscription");
                        return (false, Some(HookResult::SubscribeAclResult(rmqtt::types::SubscribeAclResult::new_failure(rmqtt::codec::v5::SubscribeAckReason::NotAuthorized))));
                    }
                }
            }
        }

        (true, None)
    }
}

#[async_trait]
impl Handler for JavaScriptHookHandler {
    async fn hook(&self, param: &Parameter, acc: Option<HookResult>) -> ReturnType {
        match param {
            Parameter::MessagePublish(_session, _from, publish) => {
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
                let data = serde_json::json!({
                    "topicFilter": unsubscribe.topic_filter.to_string()
                });
                self.call_js_hook("client_unsubscribe", data);
            }
            Parameter::ClientAuthenticate(connect_info) => {
                return self.call_js_auth_hook(connect_info).await;
            }
            _ => {}
        }
        (true, acc)
    }
}
