use std::time::Duration;
use std::collections::{HashMap};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

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

// Cache to carry publish mutation decisions from ACL stage to publish stage (single JS call per message)
static PUBLISH_DECISIONS: Lazy<Mutex<HashMap<String, PublishDecision>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// Using official RMQTT lifecycle hooks; no duplicate tracking needed

#[inline]
fn make_publish_key(p: &rmqtt::types::Publish) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    p.topic.to_string().hash(&mut hasher);
    p.payload.hash(&mut hasher);
    (p.qos as u8).hash(&mut hasher);
    p.retain.hash(&mut hasher);
    let h = hasher.finish();
    format!("{}:{}:{}:{}", p.topic, h, p.qos as u8, p.retain as u8)
}

pub(crate) struct HookCallbackStorage {
    pub channel: Option<Channel>,
    pub on_message_publish: Option<neon::handle::Root<JsFunction>>,
    pub on_client_subscribe: Option<neon::handle::Root<JsFunction>>,
    pub on_client_unsubscribe: Option<neon::handle::Root<JsFunction>>,
    pub on_client_authenticate: Option<neon::handle::Root<JsFunction>>,
    pub on_client_subscribe_authorize: Option<neon::handle::Root<JsFunction>>,
    pub on_client_publish_authorize: Option<neon::handle::Root<JsFunction>>,
    // Lifecycle hooks aligned with RMQTT
    pub on_client_connect: Option<neon::handle::Root<JsFunction>>,
    pub on_client_connack: Option<neon::handle::Root<JsFunction>>,
    pub on_client_connected: Option<neon::handle::Root<JsFunction>>,
    pub on_client_disconnected: Option<neon::handle::Root<JsFunction>>,
    pub on_session_created: Option<neon::handle::Root<JsFunction>>,
    pub on_session_subscribed: Option<neon::handle::Root<JsFunction>>,
    pub on_session_unsubscribed: Option<neon::handle::Root<JsFunction>>,
    pub on_session_terminated: Option<neon::handle::Root<JsFunction>>,
    // Message delivery lifecycle
    pub on_message_delivered: Option<neon::handle::Root<JsFunction>>,
    pub on_message_acked: Option<neon::handle::Root<JsFunction>>,
    pub on_message_dropped: Option<neon::handle::Root<JsFunction>>,
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
            on_client_publish_authorize: None,
            on_client_connect: None,
            on_client_connack: None,
            on_client_connected: None,
            on_client_disconnected: None,
            on_session_created: None,
            on_session_subscribed: None,
            on_session_unsubscribed: None,
            on_session_terminated: None,
            on_message_delivered: None,
            on_message_acked: None,
            on_message_dropped: None,
        }
    }

    pub fn clear(&mut self) {
        self.channel = None;
        self.on_message_publish = None;
        self.on_client_subscribe = None;
        self.on_client_unsubscribe = None;
        self.on_client_authenticate = None;
        self.on_client_subscribe_authorize = None;
        self.on_client_publish_authorize = None;
        self.on_client_connect = None;
        self.on_client_connack = None;
        self.on_client_connected = None;
        self.on_client_disconnected = None;
        self.on_session_created = None;
        self.on_session_subscribed = None;
        self.on_session_unsubscribed = None;
        self.on_session_terminated = None;
        self.on_message_delivered = None;
        self.on_message_acked = None;
        self.on_message_dropped = None;
    }
}

// Authentication result structure for JavaScript callbacks
#[derive(Debug, Clone)]
pub struct AuthenticationResult {
    pub allow: bool,
    pub superuser: bool,
}

// Subscription authorization result structure from JavaScript
#[derive(Debug, Clone)]
pub struct SubscribeDecision {
    pub allow: bool,
    pub qos: Option<u8>,
}

// Publish authorization result structure from JavaScript
#[derive(Debug, Clone)]
pub struct PublishDecision {
    pub allow: bool,
    pub topic: Option<String>,
    pub payload: Option<Vec<u8>>,
    pub qos: Option<u8>,
}

// Removed unused hook data structures to keep module lean

// JavaScript hook handler that calls JavaScript callbacks
pub struct JavaScriptHookHandler {}

impl JavaScriptHookHandler {
    pub fn new() -> Self {
        Self {}
    }

    fn emit_message_event(
        &self,
        kind: &str,
        session_info: Option<(String, Option<String>, Option<String>)>,
        from_dbg: Option<String>,
        publish: &rmqtt::types::Publish,
        reason: Option<String>,
    ) {
        if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
            let cb_root_opt = match kind {
                "delivered" => callbacks.on_message_delivered.as_ref(),
                "acked" => callbacks.on_message_acked.as_ref(),
                "dropped" => callbacks.on_message_dropped.as_ref(),
                _ => None,
            };
            if cb_root_opt.is_none() { return; }
            let channel = callbacks.channel.clone();
            if channel.is_none() { return; }

            // Clone payload fields
            let topic = publish.topic.to_string();
            let payload = publish.payload.to_vec();
            let qos = publish.qos as u8;
            let retain = publish.retain;
            let dup = publish.dup;
            let create_time = publish.create_time.unwrap_or(0);

            // Clone minimal session representation
            let (client_id, username, remote_addr) = session_info.unwrap_or((String::new(), None, None));

            let kind_string = kind.to_string();
            let reason_clone = reason.clone();
            let from_dbg_clone = from_dbg.clone();

            if let Some(channel) = channel {
                let _ = channel.try_send(move |mut cx| {
                    if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                        let cb_root_opt = match kind_string.as_str() {
                            "delivered" => callbacks.on_message_delivered.as_ref(),
                            "acked" => callbacks.on_message_acked.as_ref(),
                            "dropped" => callbacks.on_message_dropped.as_ref(),
                            _ => None,
                        };
                        if let Some(cb_root) = cb_root_opt {
                            let cb = cb_root.to_inner(&mut cx);
                            // session
                            let s_val: Handle<JsValue> = if client_id.is_empty() {
                                cx.null().upcast()
                            } else {
                                let s = cx.empty_object();
                                let node_js = cx.number(1f64);
                                let client_js = cx.string(client_id.clone());
                                let user_js: Handle<JsValue> = match username.as_ref() { Some(u)=>cx.string(u).upcast(), None=>cx.null().upcast() };
                                let remote_js: Handle<JsValue> = match remote_addr.as_ref() { Some(r)=>cx.string(r).upcast(), None=>cx.null().upcast() };
                                s.set(&mut cx, "node", node_js).ok();
                                s.set(&mut cx, "clientId", client_js).ok();
                                s.set(&mut cx, "username", user_js).ok();
                                s.set(&mut cx, "remoteAddr", remote_js).ok();
                                s.upcast()
                            };
                            // from
                            let from_obj = cx.empty_object();
                            // best-effort mapping using Debug string of From
                            let t = if let Some(ref d) = from_dbg_clone {
                                if d.contains("Client") { "client" } else if d.contains("Bridge") { "bridge" } else if d.contains("Admin") { "admin" } else if d.contains("LastWill") { "lastwill" } else if d.contains("System") { "system" } else { "custom" }
                            } else { "system" };
                            let t_js = cx.string(t);
                            let node_js2 = cx.number(1f64);
                            let client_empty = cx.string("");
                            let username_null: Handle<JsValue> = cx.null().upcast();
                            let remote_null: Handle<JsValue> = cx.null().upcast();
                            from_obj.set(&mut cx, "type", t_js).ok();
                            from_obj.set(&mut cx, "node", node_js2).ok();
                            from_obj.set(&mut cx, "clientId", client_empty).ok();
                            from_obj.set(&mut cx, "username", username_null).ok();
                            from_obj.set(&mut cx, "remoteAddr", remote_null).ok();

                            // message
                            let msg = cx.empty_object();
                            let topic_js = cx.string(topic);
                            let mut buf = cx.buffer(payload.len())?;
                            buf.as_mut_slice(&mut cx).copy_from_slice(&payload);
                            let qos_js = cx.number(qos as f64);
                            let retain_js = cx.boolean(retain);
                            let dup_js = cx.boolean(dup);
                            let ct_js = cx.number(create_time as f64);
                            msg.set(&mut cx, "topic", topic_js).ok();
                            msg.set(&mut cx, "payload", buf).ok();
                            msg.set(&mut cx, "qos", qos_js).ok();
                            msg.set(&mut cx, "retain", retain_js).ok();
                            msg.set(&mut cx, "dup", dup_js).ok();
                            msg.set(&mut cx, "createTime", ct_js).ok();

                            if kind_string.as_str() == "dropped" {
                                let info = cx.empty_object();
                                if let Some(r) = reason_clone.as_ref() {
                                    let r_js = cx.string(r.as_str());
                                    info.set(&mut cx, "reason", r_js).ok();
                                }
                                let _ = cb.call_with(&cx).arg(s_val).arg(from_obj).arg(msg).arg(info).apply::<JsUndefined, _>(&mut cx);
                            } else {
                                let _ = cb.call_with(&cx).arg(s_val).arg(from_obj).arg(msg).apply::<JsUndefined, _>(&mut cx);
                            }
                        }
                    }
                    Ok(())
                });
            }
        }
    }

    fn emit_client_connect(&self, info: &rmqtt::types::ConnectInfo) {
        if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
            if callbacks.on_client_connect.is_none() { return; }
            let channel = callbacks.channel.clone();
            if channel.is_none() { return; }
            let id = info.id();
            let client_id = id.client_id.to_string();
            let username = info.username().map(|u| u.to_string());
            let remote_addr = id.remote_addr.map(|a| a.to_string());
            let keep_alive = info.keep_alive();
            let proto_ver = info.proto_ver();
            let clean_start = info.clean_start();
            if let Some(channel) = channel {
                let _ = channel.try_send(move |mut cx| {
                    if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                        if let Some(cb_root) = &callbacks.on_client_connect {
                            let cb = cb_root.to_inner(&mut cx);
                            let obj = cx.empty_object();
                            let node_js = cx.number(1f64);
                            let client_js = cx.string(client_id);
                            let user_js: Handle<JsValue> = match username { Some(u)=>cx.string(u).upcast(), None=>cx.null().upcast()};
                            let remote_js: Handle<JsValue> = match remote_addr { Some(r)=>cx.string(r).upcast(), None=>cx.null().upcast()};
                            let keep_alive_js = cx.number(keep_alive as f64);
                            let proto_js = cx.number(proto_ver as f64);
                            let clean_start_js = cx.boolean(clean_start);
                            obj.set(&mut cx, "node", node_js).ok();
                            obj.set(&mut cx, "clientId", client_js).ok();
                            obj.set(&mut cx, "username", user_js).ok();
                            obj.set(&mut cx, "remoteAddr", remote_js).ok();
                            obj.set(&mut cx, "keepAlive", keep_alive_js).ok();
                            obj.set(&mut cx, "protoVer", proto_js).ok();
                            obj.set(&mut cx, "cleanStart", clean_start_js).ok();
                            let _ = cb.call_with(&cx).arg(obj).apply::<JsUndefined, _>(&mut cx);
                        }
                    }
                    Ok(())
                });
            }
        }
    }

    fn emit_client_connack(&self, info: &rmqtt::types::ConnectInfo, reason: &rmqtt::types::ConnectAckReason) {
        if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
            if callbacks.on_client_connack.is_none() { return; }
            let channel = callbacks.channel.clone();
            if channel.is_none() { return; }
            let id = info.id();
            let client_id = id.client_id.to_string();
            let username = info.username().map(|u| u.to_string());
            let remote_addr = id.remote_addr.map(|a| a.to_string());
            let keep_alive = info.keep_alive();
            let proto_ver = info.proto_ver();
            let clean_start = info.clean_start();
            let connack_str = format!("{:?}", reason);
            if let Some(channel) = channel {
                let _ = channel.try_send(move |mut cx| {
                    if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                        if let Some(cb_root) = &callbacks.on_client_connack {
                            let cb = cb_root.to_inner(&mut cx);
                            let obj = cx.empty_object();
                            let node_js = cx.number(1f64);
                            let client_js = cx.string(client_id);
                            let user_js: Handle<JsValue> = match username { Some(u)=>cx.string(u).upcast(), None=>cx.null().upcast()};
                            let remote_js: Handle<JsValue> = match remote_addr { Some(r)=>cx.string(r).upcast(), None=>cx.null().upcast()};
                            let keep_alive_js = cx.number(keep_alive as f64);
                            let proto_js = cx.number(proto_ver as f64);
                            let clean_start_js = cx.boolean(clean_start);
                            let connack_js = cx.string(connack_str);
                            obj.set(&mut cx, "node", node_js).ok();
                            obj.set(&mut cx, "clientId", client_js).ok();
                            obj.set(&mut cx, "username", user_js).ok();
                            obj.set(&mut cx, "remoteAddr", remote_js).ok();
                            obj.set(&mut cx, "keepAlive", keep_alive_js).ok();
                            obj.set(&mut cx, "protoVer", proto_js).ok();
                            obj.set(&mut cx, "cleanStart", clean_start_js).ok();
                            obj.set(&mut cx, "connAck", connack_js).ok();
                            let _ = cb.call_with(&cx).arg(obj).apply::<JsUndefined, _>(&mut cx);
                        }
                    }
                    Ok(())
                });
            }
        }
    }


    fn emit_session_subscribed(&self, session: &rmqtt::session::Session, subscribe: &rmqtt::types::Subscribe) {
        if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
            if callbacks.on_session_subscribed.is_none() { return; }
            let channel = callbacks.channel.clone();
            if channel.is_none() { return; }
            let sess_id = session.id();
            let client_id = sess_id.client_id.to_string();
            let username = session.username().map(|u| u.to_string());
            let remote_addr = sess_id.remote_addr.map(|a| a.to_string());
            let topic_filter = subscribe.topic_filter.to_string();
            let qos = subscribe.opts.qos() as u8;
            if let Some(channel) = channel {
                let _ = channel.try_send(move |mut cx| {
                    if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                        if let Some(cb_root) = &callbacks.on_session_subscribed {
                            let cb = cb_root.to_inner(&mut cx);
                            let s = cx.empty_object();
                            let node_js = cx.number(1f64);
                            let client_js = cx.string(client_id);
                            let user_js: Handle<JsValue> = match username { Some(u)=>cx.string(u).upcast(), None=>cx.null().upcast()};
                            let remote_js: Handle<JsValue> = match remote_addr { Some(r)=>cx.string(r).upcast(), None=>cx.null().upcast()};
                            s.set(&mut cx, "node", node_js).ok();
                            s.set(&mut cx, "clientId", client_js).ok();
                            s.set(&mut cx, "username", user_js).ok();
                            s.set(&mut cx, "remoteAddr", remote_js).ok();

                            let sub = cx.empty_object();
                            let topic_js = cx.string(topic_filter);
                            let qos_js = cx.number(qos as f64);
                            sub.set(&mut cx, "topicFilter", topic_js).ok();
                            sub.set(&mut cx, "qos", qos_js).ok();

                            let _ = cb.call_with(&cx).arg(s).arg(sub).apply::<JsUndefined, _>(&mut cx);
                        }
                    }
                    Ok(())
                });
            }
        }
    }

    fn emit_session_unsubscribed(&self, session: &rmqtt::session::Session, unsubscribe: &rmqtt::types::Unsubscribe) {
        if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
            if callbacks.on_session_unsubscribed.is_none() { return; }
            let channel = callbacks.channel.clone();
            if channel.is_none() { return; }
            let sess_id = session.id();
            let client_id = sess_id.client_id.to_string();
            let username = session.username().map(|u| u.to_string());
            let remote_addr = sess_id.remote_addr.map(|a| a.to_string());
            let topic_filter = unsubscribe.topic_filter.to_string();
            if let Some(channel) = channel {
                let _ = channel.try_send(move |mut cx| {
                    if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                        if let Some(cb_root) = &callbacks.on_session_unsubscribed {
                            let cb = cb_root.to_inner(&mut cx);
                            let s = cx.empty_object();
                            let node_js = cx.number(1f64);
                            let client_js = cx.string(client_id);
                            let user_js: Handle<JsValue> = match username { Some(u)=>cx.string(u).upcast(), None=>cx.null().upcast()};
                            let remote_js: Handle<JsValue> = match remote_addr { Some(r)=>cx.string(r).upcast(), None=>cx.null().upcast()};
                            s.set(&mut cx, "node", node_js).ok();
                            s.set(&mut cx, "clientId", client_js).ok();
                            s.set(&mut cx, "username", user_js).ok();
                            s.set(&mut cx, "remoteAddr", remote_js).ok();

                            let unsub = cx.empty_object();
                            let topic_js = cx.string(topic_filter);
                            unsub.set(&mut cx, "topicFilter", topic_js).ok();

                            let _ = cb.call_with(&cx).arg(s).arg(unsub).apply::<JsUndefined, _>(&mut cx);
                        }
                    }
                    Ok(())
                });
            }
        }
    }

    fn emit_lifecycle(&self, event: &str, session_opt: Option<&rmqtt::session::Session>, reason: Option<String>) {
        if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
            let (cb_opt, expect_session) = match event {
                "client_connect" => (&callbacks.on_client_connect, false),
                "client_connack" => (&callbacks.on_client_connack, false),
                "client_connected" => (&callbacks.on_client_connected, true),
                "client_disconnected" => (&callbacks.on_client_disconnected, true),
                "session_created" => (&callbacks.on_session_created, true),
                "session_subscribed" => (&callbacks.on_session_subscribed, true),
                "session_unsubscribed" => (&callbacks.on_session_unsubscribed, true),
                "session_terminated" => (&callbacks.on_session_terminated, true),
                _ => { return; }
            };
            if cb_opt.is_none() { return; }
            let channel = callbacks.channel.clone();
            if channel.is_none() { return; }
            let reason_clone = reason.clone();
            // Clone minimal representation of session fields
            let (client_id, username, remote_addr) = session_opt.map(|s| {
                let id = s.id();
                (id.client_id.to_string(), s.username().map(|u| u.to_string()), id.remote_addr.map(|a| a.to_string()))
            }).unwrap_or((String::new(), None, None));

            if let Some(channel) = channel {
                let event_kind = event.to_string();
                let _ = channel.try_send(move |mut cx| {
                    if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                        let cb_root = match event_kind.as_str() {
                            "client_connect" => callbacks.on_client_connect.as_ref(),
                            "client_connack" => callbacks.on_client_connack.as_ref(),
                            "client_connected" => callbacks.on_client_connected.as_ref(),
                            "client_disconnected" => callbacks.on_client_disconnected.as_ref(),
                            "session_created" => callbacks.on_session_created.as_ref(),
                            "session_subscribed" => callbacks.on_session_subscribed.as_ref(),
                            "session_unsubscribed" => callbacks.on_session_unsubscribed.as_ref(),
                            "session_terminated" => callbacks.on_session_terminated.as_ref(),
                            _ => None,
                        };
                        if let Some(cb_root) = cb_root {
                            let cb = cb_root.to_inner(&mut cx);
                            if expect_session {
                                let s = cx.empty_object();
                                let node_num = cx.number(1f64);
                                let client_id_js = cx.string(client_id);
                                let user_js: Handle<JsValue> = match username { Some(u) => cx.string(u).upcast(), None => cx.null().upcast() };
                                let remote_js: Handle<JsValue> = match remote_addr { Some(r) => cx.string(r).upcast(), None => cx.null().upcast() };
                                s.set(&mut cx, "node", node_num).ok();
                                s.set(&mut cx, "clientId", client_id_js).ok();
                                s.set(&mut cx, "username", user_js).ok();
                                s.set(&mut cx, "remoteAddr", remote_js).ok();
                                if event_kind.as_str() == "client_disconnected" || event_kind.as_str() == "session_terminated" {
                                    let info = cx.empty_object();
                                    if let Some(r) = reason_clone.as_ref() {
                                        let reason_js = cx.string(r);
                                        info.set(&mut cx, "reason", reason_js).ok();
                                    }
                                    let _ = cb.call_with(&cx).arg(s).arg(info).apply::<JsUndefined, _>(&mut cx);
                                } else {
                                    let _ = cb.call_with(&cx).arg(s).apply::<JsUndefined, _>(&mut cx);
                                }
                            } else {
                                // Non-session info events (client_connect/connack)
                                let info = cx.empty_object();
                                let remote_js: Handle<JsValue> = match remote_addr { Some(r) => cx.string(r).upcast(), None => cx.null().upcast() };
                                info.set(&mut cx, "remoteAddr", remote_js).ok();
                                let client_id_js = cx.string(client_id);
                                info.set(&mut cx, "clientId", client_id_js).ok();
                                let user_js: Handle<JsValue> = match username { Some(u) => cx.string(u).upcast(), None => cx.null().upcast() };
                                info.set(&mut cx, "username", user_js).ok();
                                if let Some(r) = reason_clone.as_ref() {
                                    let reason_js = cx.string(r);
                                    info.set(&mut cx, "connAck", reason_js).ok();
                                }
                                let _ = cb.call_with(&cx).arg(info).apply::<JsUndefined, _>(&mut cx);
                            }
                        }
                    }
                    Ok(())
                });
            }
        }
    }

    // Removed legacy connected emission from ConnectInfo; now using official lifecycle hooks

    fn emit_on_message_publish(&self, publish: &rmqtt::types::Publish) {
        if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
            if callbacks.on_message_publish.is_none() { return; }
            let channel = callbacks.channel.clone();
            if channel.is_none() { return; }
            // Clone data for 'static closure
            let topic = publish.topic.to_string();
            let payload = publish.payload.to_vec();
            let qos = publish.qos as u8;
            let retain = publish.retain;
            let dup = publish.dup;
            let create_time = publish.create_time.unwrap_or(0);
            if let Some(channel) = channel {
                let _ = channel.try_send(move |mut cx| {
                    if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                        if let Some(cb_root) = &callbacks.on_message_publish {
                            let cb = cb_root.to_inner(&mut cx);
                            let session = cx.null();
                            let from = cx.empty_object();
                            // keep type casing consistent with TS: "system"
                            let js_from_type = cx.string("system");
                            from.set(&mut cx, "type", js_from_type)?;
                            let message = cx.empty_object();
                            let js_topic = cx.string(topic);
                            let mut js_payload = cx.buffer(payload.len())?;
                            js_payload.as_mut_slice(&mut cx).copy_from_slice(&payload);
                            let js_qos = cx.number(qos as f64);
                            let js_retain = cx.boolean(retain);
                            let js_dup = cx.boolean(dup);
                            let js_ct = cx.number(create_time as f64);
                            message.set(&mut cx, "topic", js_topic)?;
                            message.set(&mut cx, "payload", js_payload)?;
                            message.set(&mut cx, "qos", js_qos)?;
                            message.set(&mut cx, "retain", js_retain)?;
                            message.set(&mut cx, "dup", js_dup)?;
                            message.set(&mut cx, "createTime", js_ct)?;
                            let _ = cb.call_with(&cx).arg(session).arg(from).arg(message).apply::<JsUndefined, _>(&mut cx);
                        }
                    }
                    Ok(())
                });
            }
        }
    }

    fn emit_on_client_subscribe(&self, subscribe: &rmqtt::types::Subscribe) {
        if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
            if callbacks.on_client_subscribe.is_none() { return; }
            let channel = callbacks.channel.clone();
            if channel.is_none() { return; }
            let topic_filter = subscribe.topic_filter.to_string();
            let qos = subscribe.opts.qos() as u8;
            if let Some(channel) = channel {
                let _ = channel.try_send(move |mut cx| {
                    if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                        if let Some(cb_root) = &callbacks.on_client_subscribe {
                            let cb = cb_root.to_inner(&mut cx);
                            let session = cx.null();
                            let subscription = cx.empty_object();
                            let js_topic = cx.string(topic_filter);
                            let js_qos = cx.number(qos as f64);
                            subscription.set(&mut cx, "topicFilter", js_topic)?;
                            subscription.set(&mut cx, "qos", js_qos)?;
                            let _ = cb.call_with(&cx).arg(session).arg(subscription).apply::<JsUndefined, _>(&mut cx);
                        }
                    }
                    Ok(())
                });
            }
        }
    }

    fn emit_on_client_unsubscribe(&self, unsubscribe: &rmqtt::types::Unsubscribe) {
        if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
            if callbacks.on_client_unsubscribe.is_none() { return; }
            let channel = callbacks.channel.clone();
            if channel.is_none() { return; }
            let topic_filter = unsubscribe.topic_filter.to_string();
            if let Some(channel) = channel {
                let _ = channel.try_send(move |mut cx| {
                    if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                        if let Some(cb_root) = &callbacks.on_client_unsubscribe {
                            let cb = cb_root.to_inner(&mut cx);
                            let session = cx.null();
                            let unsubscription = cx.empty_object();
                            let js_topic = cx.string(topic_filter);
                            unsubscription.set(&mut cx, "topicFilter", js_topic)?;
                            let _ = cb.call_with(&cx).arg(session).arg(unsubscription).apply::<JsUndefined, _>(&mut cx);
                        }
                    }
                    Ok(())
                });
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

            // Clone required fields out of connect_info so the closure is 'static
            let client_id = connect_info.id().client_id.to_string();
            let username_opt: Option<String> = connect_info.username().map(|u| u.to_string());
            let password_opt: Option<String> = connect_info.password().map(|p| String::from_utf8_lossy(p).to_string());
            let protocol_version = connect_info.proto_ver();
            let remote_addr = connect_info
                .id()
                .remote_addr
                .map(|a| a.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let keep_alive = connect_info.keep_alive();
            let clean_session = connect_info.clean_start();

            let channel = {
                if let Ok(callbacks) = HOOK_CALLBACKS.lock() { callbacks.channel.clone() } else { None }
            };

            if let Some(channel) = channel {
                let send_result = channel.try_send(move |mut cx| {
                    if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                        if let Some(callback_root) = &callbacks.on_client_authenticate {
                            let callback = callback_root.to_inner(&mut cx);
                            // Build auth request directly from connect_info
                            let auth_request = cx.empty_object();
                            // Precompute JS values to avoid multiple mutable borrows
                            let js_client_id = cx.string(client_id);
                            let js_username: Handle<JsValue> = match username_opt {
                                Some(u) => cx.string(u).upcast(),
                                None => cx.null().upcast(),
                            };
                            let js_password: Handle<JsValue> = match password_opt {
                                Some(p) => cx.string(p).upcast(),
                                None => cx.null().upcast(),
                            };
                            let js_proto = cx.number(protocol_version as f64);
                            let js_remote = cx.string(remote_addr);
                            let js_keep_alive = cx.number(keep_alive as f64);
                            let js_clean = cx.boolean(clean_session);

                            auth_request.set(&mut cx, "clientId", js_client_id)?;
                            auth_request.set(&mut cx, "username", js_username)?;
                            auth_request.set(&mut cx, "password", js_password)?;
                            auth_request.set(&mut cx, "protocolVersion", js_proto)?;
                            auth_request.set(&mut cx, "remoteAddr", js_remote)?;
                            auth_request.set(&mut cx, "keepAlive", js_keep_alive)?;
                            auth_request.set(&mut cx, "cleanSession", js_clean)?;

                            // Call user callback, support promise
                            let result_val = match callback.call_with(&cx).arg(auth_request).apply::<JsValue, _>(&mut cx) {
                                Ok(v) => v,
                                Err(_) => { let _ = tx.send(AuthenticationResult { allow: false, superuser: false }); return Ok(()); }
                            };
                            if result_val.is_a::<JsObject, _>(&mut cx) {
                                let obj = result_val.downcast::<JsObject, _>(&mut cx).unwrap();
                                if let Ok(Some(then_fn)) = obj.get_opt::<JsFunction, _, _>(&mut cx, "then") {
                                    let tx_arc: Arc<Mutex<Option<tokio::sync::oneshot::Sender<AuthenticationResult>>>> = Arc::new(Mutex::new(Some(tx)));
                                    let resolve_arc = tx_arc.clone();
                                    let resolve = JsFunction::new(&mut cx, move |mut cx| {
                                        let arg0 = cx.argument::<JsValue>(0)?;
                                        if let Ok(obj) = arg0.downcast::<JsObject, _>(&mut cx) {
                                            let allow = obj.get::<JsBoolean, _, _>(&mut cx, "allow").ok().map(|b| b.value(&mut cx)).unwrap_or(false);
                                            let superuser = obj.get_opt::<JsBoolean, _, _>(&mut cx, "superuser").ok().flatten().map(|b| b.value(&mut cx)).unwrap_or(false);
                                            if let Some(sender) = resolve_arc.lock().unwrap().take() { let _ = sender.send(AuthenticationResult { allow, superuser }); }
                                        } else {
                                            if let Some(sender) = resolve_arc.lock().unwrap().take() { let _ = sender.send(AuthenticationResult { allow: false, superuser: false }); }
                                        }
                                        Ok(cx.undefined())
                                    })?;
                                    let reject_arc = tx_arc.clone();
                                    let reject = JsFunction::new(&mut cx, move |mut cx| {
                                        if let Some(sender) = reject_arc.lock().unwrap().take() { let _ = sender.send(AuthenticationResult { allow: false, superuser: false }); }
                                        Ok(cx.undefined())
                                    })?;
                                    let _ = then_fn.call_with(&cx).this(obj).arg(resolve).arg(reject).apply::<JsValue, _>(&mut cx);
                                } else {
                                    // Synchronous object
                                    let allow = obj.get::<JsBoolean, _, _>(&mut cx, "allow").ok().map(|b| b.value(&mut cx)).unwrap_or(false);
                                    let superuser = obj.get_opt::<JsBoolean, _, _>(&mut cx, "superuser").ok().flatten().map(|b| b.value(&mut cx)).unwrap_or(false);
                                    let _ = tx.send(AuthenticationResult { allow, superuser });
                                }
                            } else {
                                let _ = tx.send(AuthenticationResult { allow: false, superuser: false });
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

            // Clone data needed by JS closure to satisfy 'static
            let sess_id_client = session.id().client_id.to_string();
            let sess_remote: Option<String> = session.id().remote_addr.map(|a| a.to_string());
            let sess_username: Option<String> = session.username().map(|u| u.to_string());
            let sub_topic = subscribe.topic_filter.to_string();
            let sub_qos = subscribe.opts.qos() as u8;

            let channel = { if let Ok(callbacks) = HOOK_CALLBACKS.lock() { callbacks.channel.clone() } else { None } };

            if let Some(channel) = channel {
                let send_result = channel.try_send(move |mut cx| {
                    if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                        if let Some(callback_root) = &callbacks.on_client_subscribe_authorize {
                            let callback = callback_root.to_inner(&mut cx);
                            // Build session object directly
                            let s_obj = cx.empty_object();
                            let js_remote: Handle<JsValue> = match sess_remote {
                                Some(r) => cx.string(r).upcast(),
                                None => cx.null().upcast(),
                            };
                            let js_client = cx.string(sess_id_client);
                            let js_username: Handle<JsValue> = match sess_username {
                                Some(u) => cx.string(u).upcast(),
                                None => cx.null().upcast(),
                            };
                            s_obj.set(&mut cx, "remoteAddr", js_remote)?;
                            s_obj.set(&mut cx, "clientId", js_client)?;
                            s_obj.set(&mut cx, "username", js_username)?;

                            // Build subscription object
                            let sub_obj = cx.empty_object();
                            let js_topic = cx.string(sub_topic);
                            let js_qos = cx.number(sub_qos as f64);
                            sub_obj.set(&mut cx, "topicFilter", js_topic)?;
                            sub_obj.set(&mut cx, "qos", js_qos)?;

                            // Call user callback with Promise support
                            let result_val = match callback.call_with(&cx).arg(s_obj.upcast::<JsValue>()).arg(sub_obj).apply::<JsValue, _>(&mut cx) {
                                Ok(v) => v,
                                Err(_) => { let _ = tx.send(SubscribeDecision { allow: false, qos: None }); return Ok(()); }
                            };
                            if result_val.is_a::<JsObject, _>(&mut cx) {
                                let obj = result_val.downcast::<JsObject, _>(&mut cx).unwrap();
                                if let Ok(Some(then_fn)) = obj.get_opt::<JsFunction, _, _>(&mut cx, "then") {
                                    let tx_arc: Arc<Mutex<Option<tokio::sync::oneshot::Sender<SubscribeDecision>>>> = Arc::new(Mutex::new(Some(tx)));
                                    let resolve_arc = tx_arc.clone();
                                    let resolve = JsFunction::new(&mut cx, move |mut cx| {
                                        let arg0 = cx.argument::<JsValue>(0)?;
                                        let mut allow = false; let mut qos: Option<u8> = None;
                                        if let Ok(obj) = arg0.downcast::<JsObject, _>(&mut cx) {
                                            allow = obj.get::<JsBoolean, _, _>(&mut cx, "allow").ok().map(|b| b.value(&mut cx)).unwrap_or(false);
                                            qos = obj.get_opt::<JsNumber, _, _>(&mut cx, "qos").ok().flatten().map(|n| n.value(&mut cx) as u8);
                                        }
                                        if let Some(sender) = resolve_arc.lock().unwrap().take() { let _ = sender.send(SubscribeDecision { allow, qos }); }
                                        Ok(cx.undefined())
                                    })?;
                                    let reject_arc = tx_arc.clone();
                                    let reject = JsFunction::new(&mut cx, move |mut cx| {
                                        if let Some(sender) = reject_arc.lock().unwrap().take() { let _ = sender.send(SubscribeDecision { allow: false, qos: None }); }
                                        Ok(cx.undefined())
                                    })?;
                                    let _ = then_fn.call_with(&cx).this(obj).arg(resolve).arg(reject).apply::<JsValue, _>(&mut cx);
                                } else {
                                    // Synchronous object
                                    let allow = obj.get::<JsBoolean, _, _>(&mut cx, "allow").ok().map(|b| b.value(&mut cx)).unwrap_or(false);
                                    let qos = obj.get_opt::<JsNumber, _, _>(&mut cx, "qos").ok().flatten().map(|n| n.value(&mut cx) as u8);
                                    let _ = tx.send(SubscribeDecision { allow, qos });
                                }
                            } else {
                                let _ = tx.send(SubscribeDecision { allow: false, qos: None });
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
                                let qos = match RmQos::try_from(qos_num) { Ok(q) => q, Err(_) => { eprintln!("WARN: Invalid QoS {} in subscribe hook; using original", qos_num); subscribe.opts.qos() } };
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

    async fn get_publish_decision(
        &self,
        session: Option<&rmqtt::session::Session>,
        publish: &rmqtt::types::Publish,
    ) -> Option<PublishDecision> {
        // Check availability
        let (has_callback, channel_available) = {
            if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                (
                    callbacks.on_client_publish_authorize.is_some(),
                    callbacks.channel.is_some(),
                )
            } else {
                (false, false)
            }
        };

        if !(has_callback && channel_available) {
            return None;
        }

        let (tx, rx) = oneshot::channel::<PublishDecision>();

        // Capture session fields directly to avoid JSON round-trips
        let (remote_addr_opt, client_id_opt, username_opt) = if let Some(s) = session {
            let id = s.id();
            (
                id.remote_addr.map(|a| a.to_string()),
                Some(id.client_id.to_string()),
                s.username().map(|u| u.to_string()),
            )
        } else {
            (None, None, None)
        };

        // Clone fields needed for JS closure
        let topic = publish.topic.to_string();
        let payload_bytes: Vec<u8> = publish.payload.to_vec();
        let qos_val: u8 = publish.qos as u8;
        let retain = publish.retain;

    let channel = { if let Ok(callbacks) = HOOK_CALLBACKS.lock() { callbacks.channel.clone() } else { None } };

        if let Some(channel) = channel {
            let send_result = channel.try_send(move |mut cx| {
                if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                    if let Some(callback_root) = &callbacks.on_client_publish_authorize {
                        let callback = callback_root.to_inner(&mut cx);

                        // Build session object (null when no session)
                        let session_val: Handle<JsValue> = if client_id_opt.is_none() {
                            cx.null().upcast()
                        } else {
                            let s = cx.empty_object();
                            if let Some(remote) = remote_addr_opt.as_ref() {
                                let js = cx.string(remote.as_str());
                                s.set(&mut cx, "remoteAddr", js)?;
                            } else {
                                let js = cx.null();
                                s.set(&mut cx, "remoteAddr", js)?;
                            }
                            if let Some(cid) = client_id_opt.as_ref() {
                                let js = cx.string(cid.as_str());
                                s.set(&mut cx, "clientId", js)?;
                            }
                            if let Some(user) = username_opt.as_ref() {
                                let js = cx.string(user.as_str());
                                s.set(&mut cx, "username", js)?;
                            } else {
                                let js = cx.null();
                                s.set(&mut cx, "username", js)?;
                            }
                            s.upcast()
                        };

                        // Build packet object
                        let packet = cx.empty_object();
                        let topic_js = cx.string(&topic);
                        packet.set(&mut cx, "topic", topic_js)?;
                        let mut buf = cx.buffer(payload_bytes.len())?;
                        buf.as_mut_slice(&mut cx).copy_from_slice(&payload_bytes);
                        packet.set(&mut cx, "payload", buf)?;
                        let qos_js = cx.number(qos_val as f64);
                        packet.set(&mut cx, "qos", qos_js)?;
                        let retain_js = cx.boolean(retain);
                        packet.set(&mut cx, "retain", retain_js)?;

                        // Call user callback
                        let result_val = match callback
                            .call_with(&cx)
                            .arg(session_val)
                            .arg(packet)
                            .apply::<JsValue, _>(&mut cx) {
                                Ok(v) => v,
                                Err(_) => {
                                    let _ = tx.send(PublishDecision { allow: false, topic: None, payload: None, qos: None });
                                    return Ok(());
                                }
                            };

                        // If Promise, attach then/catch; else treat as object
                        if result_val.is_a::<JsObject, _>(&mut cx) {
                            // Check for thenable (basic promise support)
                            let obj = result_val.downcast::<JsObject, _>(&mut cx).unwrap();
                            if let Ok(Some(then_fn)) = obj.get_opt::<JsFunction, _, _>(&mut cx, "then") {
                                // Promise-like: wire resolve/reject
                                let tx_arc: Arc<Mutex<Option<tokio::sync::oneshot::Sender<PublishDecision>>>> = Arc::new(Mutex::new(Some(tx)));

                                let resolve_arc = tx_arc.clone();
                                let resolve = JsFunction::new(&mut cx, move |mut cx| {
                                    let arg0 = cx.argument::<JsValue>(0)?;
                                    if let Ok(obj) = arg0.downcast::<JsObject, _>(&mut cx) {
                                        if let Some(sender) = resolve_arc.lock().unwrap().take() {
                                            // parse and send
                                            let allow = obj.get::<JsBoolean, _, _>(&mut cx, "allow").ok().map(|b| b.value(&mut cx)).unwrap_or(false);
                                            let topic_opt = obj.get_opt::<JsString, _, _>(&mut cx, "topic").ok().flatten().map(|s| s.value(&mut cx));
                                            let qos_opt = obj.get_opt::<JsNumber, _, _>(&mut cx, "qos").ok().flatten().map(|n| n.value(&mut cx) as u8);
                                            let payload_opt = if let Ok(Some(jsb)) = obj.get_opt::<JsBuffer, _, _>(&mut cx, "payload") { Some(jsb.as_slice(&mut cx).to_vec()) } else { None };
                                            let _ = sender.send(PublishDecision { allow, topic: topic_opt, payload: payload_opt, qos: qos_opt });
                                        }
                                    } else {
                                        if let Some(sender) = resolve_arc.lock().unwrap().take() { let _ = sender.send(PublishDecision { allow: false, topic: None, payload: None, qos: None }); }
                                    }
                                    Ok(cx.undefined())
                                })?;

                                let reject_arc = tx_arc.clone();
                                let reject = JsFunction::new(&mut cx, move |mut cx| {
                                    if let Some(sender) = reject_arc.lock().unwrap().take() {
                                        let _ = sender.send(PublishDecision { allow: false, topic: None, payload: None, qos: None });
                                    }
                                    Ok(cx.undefined())
                                })?;

                                if let Err(_) = then_fn.call_with(&cx).this(obj).arg(resolve).arg(reject).apply::<JsValue, _>(&mut cx) {
                                    if let Some(sender) = tx_arc.lock().unwrap().take() {
                                        let _ = sender.send(PublishDecision { allow: false, topic: None, payload: None, qos: None });
                                    }
                                }
                            } else {
                                // Synchronous object
                                let allow = obj.get::<JsBoolean, _, _>(&mut cx, "allow").ok().map(|b| b.value(&mut cx)).unwrap_or(false);
                                let topic_opt = obj.get_opt::<JsString, _, _>(&mut cx, "topic").ok().flatten().map(|s| s.value(&mut cx));
                                let qos_opt = obj.get_opt::<JsNumber, _, _>(&mut cx, "qos").ok().flatten().map(|n| n.value(&mut cx) as u8);
                                let payload_opt = if let Ok(Some(jsb)) = obj.get_opt::<JsBuffer, _, _>(&mut cx, "payload") { Some(jsb.as_slice(&mut cx).to_vec()) } else { None };
                                let _ = tx.send(PublishDecision { allow, topic: topic_opt, payload: payload_opt, qos: qos_opt });
                            }
                        } else {
                            // Unexpected type: deny fast
                            let _ = tx.send(PublishDecision { allow: false, topic: None, payload: None, qos: None });
                        }
                    }
                }
                Ok(())
            });

            match send_result {
                Ok(_) => match tokio::time::timeout(Duration::from_millis(5000), rx).await {
                    Ok(Ok(decision)) => Some(decision),
                    Ok(Err(_)) => {
                        eprintln!("WARN: Publish ACL callback channel error; denying publish");
                        Some(PublishDecision { allow: false, topic: None, payload: None, qos: None })
                    }
                    Err(_) => {
                        eprintln!("WARN: Publish ACL callback timed out; denying publish");
                        Some(PublishDecision { allow: false, topic: None, payload: None, qos: None })
                    }
                },
                Err(_) => {
                    eprintln!("WARN: Failed to dispatch publish ACL request to JavaScript; denying publish");
                    Some(PublishDecision { allow: false, topic: None, payload: None, qos: None })
                }
            }
        } else {
            None
        }
    }
}

#[async_trait]
impl Handler for JavaScriptHookHandler {
    async fn hook(&self, param: &Parameter, acc: Option<HookResult>) -> ReturnType {
        match param {
            // Fire connected after we see authenticate parameter (post-evaluation we can't distinguish allow/deny here; auth hook returns allow via result)
            // We'll emit connected on success path inside call_js_auth_hook.
            Parameter::MessagePublishCheckAcl(session, publish) => {
                // Default: if no hook registered, allow, except deny $SYS publishes by default
                let topic_str = publish.topic.to_string();
                let hook_available = {
                    if let Ok(callbacks) = HOOK_CALLBACKS.lock() { callbacks.on_client_publish_authorize.is_some() } else { false }
                };
                if !hook_available {
                    if topic_str.starts_with("$SYS") {
                        return (false, Some(HookResult::PublishAclResult(rmqtt::types::PublishAclResult::Rejected(false))));
                    }
                    return (true, acc);
                }

                if let Some(mut decision) = self.get_publish_decision(Some(session), publish).await {
                    if decision.allow {
                        // Cache mutation fields (validated) to apply in MessagePublish stage
                        let key = make_publish_key(publish);
                        let mut should_cache = false;
                        // Validate qos range
                        if let Some(q) = decision.qos {
                            if q > 2 { eprintln!("WARN: Invalid QoS {} in publish hook; ignoring", q); decision.qos = None; }
                        }
                        // Validate topic mutation (no wildcards, non-empty)
                        if let Some(t) = decision.topic.as_ref() {
                            if t.is_empty() || t.contains('+') || t.contains('#') { eprintln!("WARN: Invalid topic mutation '{}'; ignoring", t); decision.topic = None; }
                        }
                        if decision.topic.is_some() || decision.payload.is_some() || decision.qos.is_some() {
                            should_cache = true;
                        }
                        if should_cache {
                            if let Ok(mut map) = PUBLISH_DECISIONS.lock() { map.insert(key, decision); }
                        }
                        return (false, Some(HookResult::PublishAclResult(rmqtt::types::PublishAclResult::Allow)));
                    } else {
                        return (false, Some(HookResult::PublishAclResult(rmqtt::types::PublishAclResult::Rejected(false))))
                    }
                }
            }
            Parameter::MessagePublish(session_opt, _from, publish) => {
                self.emit_on_message_publish(publish);
                // Apply cached mutation, if any
                let key = make_publish_key(publish);
                if let Ok(mut map) = PUBLISH_DECISIONS.lock() {
                    if let Some(decision) = map.remove(&key) {
                        let mut new_publish = (*publish).clone();
                        if let Some(t) = decision.topic { new_publish.topic = t.into(); }
                        if let Some(q) = decision.qos { if let Ok(qos) = RmQos::try_from(q) { new_publish.qos = qos; } else { eprintln!("WARN: Invalid QoS {} after validation; ignoring", q); } }
                        if let Some(p) = decision.payload { new_publish.payload = p.into(); }
                        return (false, Some(HookResult::Publish(new_publish)));
                    }
                }
                // If we got here, cache miss; try calling JS now to fetch mutation decision
                if let Some(mut decision) = self.get_publish_decision(session_opt.as_deref(), publish).await {
                    if decision.allow {
                        // Validate mutation
                        if let Some(q) = decision.qos { if q > 2 { eprintln!("WARN: Invalid QoS {} in publish hook (late); ignoring", q); decision.qos = None; } }
                        if let Some(t) = decision.topic.as_ref() { if t.is_empty() || t.contains('+') || t.contains('#') { eprintln!("WARN: Invalid topic mutation '{}'; ignoring", t); decision.topic = None; } }
                        let mut new_publish = (*publish).clone();
                        if let Some(t) = decision.topic { new_publish.topic = t.into(); }
                        if let Some(q) = decision.qos { if let Ok(qos) = RmQos::try_from(q) { new_publish.qos = qos; } }
                        if let Some(p) = decision.payload { new_publish.payload = p.into(); }
                        return (false, Some(HookResult::Publish(new_publish)));
                    }
                }
                // fallthrough: also emit delivered/acked/dropped handled by dedicated hook params when they occur; nothing to do here
            }
            // Optional RMQTT events: deliver/ack/drop notifications
            #[allow(unreachable_patterns)]
            Parameter::MessageDelivered(session, from, publish) => {
                let id = session.id();
                let session_info = (
                    id.client_id.to_string(),
                    session.username().map(|u| u.to_string()),
                    id.remote_addr.map(|a| a.to_string()),
                );
                let from_dbg = Some(format!("{:?}", from));
                self.emit_message_event("delivered", Some(session_info), from_dbg, publish, None);
            }
            #[allow(unreachable_patterns)]
            Parameter::MessageAcked(session, from, publish) => {
                let id = session.id();
                let session_info = (
                    id.client_id.to_string(),
                    session.username().map(|u| u.to_string()),
                    id.remote_addr.map(|a| a.to_string()),
                );
                let from_dbg = Some(format!("{:?}", from));
                self.emit_message_event("acked", Some(session_info), from_dbg, publish, None);
            }
            #[allow(unreachable_patterns)]
            Parameter::MessageDropped(session_opt, from_opt, publish, reason) => {
                let session_info = session_opt.as_ref().map(|id| (
                    id.client_id.to_string(),
                    None,
                    id.remote_addr.map(|a| a.to_string()),
                ));
                let from_dbg = Some(format!("{:?}", from_opt));
                self.emit_message_event("dropped", session_info, from_dbg, publish, Some(format!("{:?}", reason)));
            }
            Parameter::ClientSubscribe(_session_ref, subscribe) => {
                self.emit_on_client_subscribe(subscribe);
            }
            Parameter::SessionSubscribed(session_ref, subscribe) => {
                self.emit_session_subscribed(session_ref, subscribe);
            }
            Parameter::ClientSubscribeCheckAcl(session, subscribe) => {
                return self.call_js_subscribe_acl_hook(session, subscribe).await;
            }
            Parameter::ClientUnsubscribe(_session, unsubscribe) => {
                self.emit_on_client_unsubscribe(unsubscribe);
            }
            Parameter::SessionUnsubscribed(session_ref, unsubscribe) => {
                self.emit_session_unsubscribed(session_ref, unsubscribe);
            }
            Parameter::ClientConnected(session) => {
                self.emit_lifecycle("client_connected", Some(session), None);
            }
            Parameter::ClientDisconnected(session, reason) => {
                self.emit_lifecycle("client_disconnected", Some(session), Some(format!("{:?}", reason)));
            }
            Parameter::ClientConnack(connect_info, reason) => {
                self.emit_client_connack(connect_info, reason);
            }
            Parameter::ClientConnect(connect_info) => {
                self.emit_client_connect(connect_info);
            }
            Parameter::ClientKeepalive(session, is_ping) => {
                // We don't surface keepalive to JS hooks by default; if desired later, we can add an onClientKeepalive callback.
                // For now, treat abnormal keepalive states as client_error if needed. Here we just ignore to avoid noise.
                let _ = is_ping; // suppress unused
                let _ = session; // suppress unused
            }
            Parameter::SessionCreated(session) => {
                self.emit_lifecycle("session_created", Some(session), None);
            }
            Parameter::SessionTerminated(session, reason) => {
                self.emit_lifecycle("session_terminated", Some(session), Some(format!("{:?}", reason)));
            }
            Parameter::ClientAuthenticate(connect_info) => {
                return self.call_js_auth_hook(connect_info).await;
            }
            _ => {}
        }
        (true, acc)
    }
}