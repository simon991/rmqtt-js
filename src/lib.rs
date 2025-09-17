use std::sync::mpsc;
use std::thread;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use neon::{prelude::*, types::Deferred, types::buffer::TypedArray, event::Channel};
use rmqtt::{
    context::ServerContext, 
    net::Builder, 
    server::MqttServer as RmqttServer, 
    Result as RmqttResult,
    hook::{Handler, HookResult, Parameter, ReturnType, Type},
    types::{From, Publish, QoS, Id},
    session::SessionState,
    codec::types::Publish as CodecPublish,
    utils::timestamp_millis
};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use serde_json::json;

// Global storage for JavaScript hook callbacks
static HOOK_CALLBACKS: Lazy<Mutex<HookCallbackStorage>> = Lazy::new(|| {
    Mutex::new(HookCallbackStorage::new())
});

struct HookCallbackStorage {
    channel: Option<Channel>,
    on_message_publish: Option<neon::handle::Root<JsFunction>>,
    on_client_subscribe: Option<neon::handle::Root<JsFunction>>,
    on_client_unsubscribe: Option<neon::handle::Root<JsFunction>>,
}

impl HookCallbackStorage {
    fn new() -> Self {
        Self {
            channel: None,
            on_message_publish: None,
            on_client_subscribe: None,
            on_client_unsubscribe: None,
        }
    }

    fn clear(&mut self) {
        self.channel = None;
        self.on_message_publish = None;
        self.on_client_subscribe = None;
        self.on_client_unsubscribe = None;
    }
}

type DeferredCallback = Box<dyn FnOnce(&Channel, Deferred) + Send>;
type ServerCallback = Box<dyn FnOnce(&Channel, Deferred) + Send>;

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
    // Promise to resolve and callback to be executed
    Callback(Deferred, ServerCallback),
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
        println!("call_js_hook called with event_type: {}", event_type);
        
        if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
            println!("Successfully acquired callbacks lock");
            
            if let Some(channel) = &callbacks.channel {
                println!("Channel is available");
                
                // Check if we have the appropriate callback
                let has_callback = match event_type {
                    "message_publish" => callbacks.on_message_publish.is_some(),
                    "client_subscribe" => callbacks.on_client_subscribe.is_some(),
                    "client_unsubscribe" => callbacks.on_client_unsubscribe.is_some(),
                    _ => false,
                };

                println!("Has callback for {}: {}", event_type, has_callback);

                if has_callback {
                    let event_type = event_type.to_string();
                    let data_json = data.to_string();
                    println!("Sending event to JavaScript: {} with data: {}", event_type, data_json);
                    
                    let send_result = channel.try_send(move |mut cx| {
                        println!("Inside JavaScript context callback");
                        
                        // Access the global callbacks again inside the JavaScript context
                        if let Ok(callbacks) = HOOK_CALLBACKS.lock() {
                            let callback_root = match event_type.as_str() {
                                "message_publish" => &callbacks.on_message_publish,
                                "client_subscribe" => &callbacks.on_client_subscribe,
                                "client_unsubscribe" => &callbacks.on_client_unsubscribe,
                                _ => return Ok(()),
                            };

                            if let Some(callback_root) = callback_root {
                                println!("Calling JavaScript function for event: {}", event_type);
                                
                                // Convert the Root back to a JavaScript function
                                let callback = callback_root.to_inner(&mut cx);
                                
                                // Parse the JSON data and create proper parameters for each hook type
                                match event_type.as_str() {
                                    "message_publish" => {
                                        // Parse JSON data for message publish hook
                                        if let Ok(data_value) = serde_json::from_str::<serde_json::Value>(&data_json) {
                                            // Create session parameter (null for system messages)
                                            let session = cx.null();
                                            
                                            // Create from parameter (simplified for now)
                                            let from = cx.empty_object();
                                            let from_type = cx.string("System");
                                            from.set(&mut cx, "type", from_type)?;
                                            
                                            // Create message parameter
                                            let message = cx.empty_object();
                                            if let Some(topic) = data_value.get("topic").and_then(|v| v.as_str()) {
                                                let topic_val = cx.string(topic);
                                                message.set(&mut cx, "topic", topic_val)?;
                                            }
                                            if let Some(payload) = data_value.get("payload").and_then(|v| v.as_str()) {
                                                let mut buffer = cx.buffer(payload.len())?;
                                                buffer.as_mut_slice(&mut cx).copy_from_slice(payload.as_bytes());
                                                message.set(&mut cx, "payload", buffer)?;
                                            }
                                            if let Some(qos) = data_value.get("qos").and_then(|v| v.as_u64()) {
                                                let qos_val = cx.number(qos as f64);
                                                message.set(&mut cx, "qos", qos_val)?;
                                            }
                                            if let Some(retain) = data_value.get("retain").and_then(|v| v.as_bool()) {
                                                let retain_val = cx.boolean(retain);
                                                message.set(&mut cx, "retain", retain_val)?;
                                            }
                                            if let Some(dup) = data_value.get("dup").and_then(|v| v.as_bool()) {
                                                let dup_val = cx.boolean(dup);
                                                message.set(&mut cx, "dup", dup_val)?;
                                            }
                                            if let Some(create_time) = data_value.get("createTime").and_then(|v| v.as_u64()) {
                                                let create_time_val = cx.number(create_time as f64);
                                                message.set(&mut cx, "createTime", create_time_val)?;
                                            }
                                            
                                            // Call with three parameters: session, from, message
                                            let _result = callback.call_with(&cx)
                                                .arg(session)
                                                .arg(from)
                                                .arg(message)
                                                .apply::<JsUndefined, _>(&mut cx);
                                        }
                                    },
                                    "client_subscribe" => {
                                        // Parse JSON data for client subscribe hook
                                        if let Ok(data_value) = serde_json::from_str::<serde_json::Value>(&data_json) {
                                            // Create session parameter (null for now - we don't have session info)
                                            let session = cx.null();
                                            
                                            // Create subscription parameter
                                            let subscription = cx.empty_object();
                                            if let Some(topic_filter) = data_value.get("topicFilter").and_then(|v| v.as_str()) {
                                                let topic_filter_val = cx.string(topic_filter);
                                                subscription.set(&mut cx, "topicFilter", topic_filter_val)?;
                                            }
                                            if let Some(qos) = data_value.get("qos").and_then(|v| v.as_u64()) {
                                                let qos_val = cx.number(qos as f64);
                                                subscription.set(&mut cx, "qos", qos_val)?;
                                            }
                                            
                                            // Call with two parameters: session, subscription
                                            let _result = callback.call_with(&cx)
                                                .arg(session)
                                                .arg(subscription)
                                                .apply::<JsUndefined, _>(&mut cx);
                                        }
                                    },
                                    "client_unsubscribe" => {
                                        // Parse JSON data for client unsubscribe hook
                                        if let Ok(data_value) = serde_json::from_str::<serde_json::Value>(&data_json) {
                                            // Create session parameter (null for now)
                                            let session = cx.null();
                                            
                                            // Create unsubscription parameter
                                            let unsubscription = cx.empty_object();
                                            if let Some(topic_filter) = data_value.get("topicFilter").and_then(|v| v.as_str()) {
                                                let topic_filter_val = cx.string(topic_filter);
                                                unsubscription.set(&mut cx, "topicFilter", topic_filter_val)?;
                                            }
                                            
                                            // Call with two parameters: session, unsubscription
                                            let _result = callback.call_with(&cx)
                                                .arg(session)
                                                .arg(unsubscription)
                                                .apply::<JsUndefined, _>(&mut cx);
                                        }
                                    },
                                    _ => {
                                        // Fallback to the old single-parameter approach
                                        let data_str = cx.string(data_json);
                                        let _result = callback.call_with(&cx)
                                            .arg(data_str)
                                            .apply::<JsUndefined, _>(&mut cx);
                                    }
                                }
                                    
                                println!("JavaScript function called successfully");
                            }
                        }
                        Ok(())
                    });
                    
                    match send_result {
                        Ok(_join_handle) => println!("Successfully sent event to JavaScript"),
                        Err(e) => println!("Failed to send event to JavaScript: {:?}", e),
                    }
                } else {
                    println!("No callback registered for event type: {}", event_type);
                }
            } else {
                println!("No channel available");
            }
        } else {
            println!("Failed to acquire callbacks lock");
        }
    }
}

#[async_trait]
impl Handler for JavaScriptHookHandler {
    async fn hook(&self, param: &Parameter, acc: Option<HookResult>) -> ReturnType {
        match param {
            Parameter::MessagePublish(_session, _from, publish) => {
                println!("Hook: Message published to topic: {}", publish.topic);
                
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
                println!("Hook: Client subscribed to: {}", subscribe.topic_filter);
                
                // Call JavaScript callback if registered
                let data = serde_json::json!({
                    "topicFilter": subscribe.topic_filter.to_string(),
                    "qos": subscribe.opts.qos() as u8
                });
                self.call_js_hook("client_subscribe", data);
            }
            Parameter::ClientUnsubscribe(_session, unsubscribe) => {
                println!("Hook: Client unsubscribed from: {}", unsubscribe.topic_filter);
                
                // Call JavaScript callback if registered
                let data = serde_json::json!({
                    "topicFilter": unsubscribe.topic_filter.to_string()
                });
                self.call_js_hook("client_unsubscribe", data);
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
                    ServerMessage::Callback(deferred, f) => {
                        f(&channel, deferred);
                    }
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
                    ServerMessage::Publish { topic, payload, qos, retain, deferred, callback, shared_state } => {
                        let topic_clone = topic.clone();
                        let payload_clone = payload.clone();
                        
                        rt.spawn(async move {
                            Self::handle_publish(shared_state, topic_clone, payload_clone, qos, retain).await;
                        });
                        
                        callback(&channel, deferred);
                    }
                    ServerMessage::Close => break,
                }
            }
        });

        Ok(Self { tx, shared_state })
    }

    async fn handle_publish(shared_state: SharedServerState, topic: String, payload: Vec<u8>, qos: u8, retain: bool) {
        // Wait for the server context to be ready with a 5-second timeout
        if !shared_state.wait_for_ready(5000).await {
            println!("Timeout waiting for server context to be ready for publishing");
            return;
        }

        if let Some(context) = shared_state.get_context() {
            // Create a proper RMQTT Publish message
            let qos = match QoS::try_from(qos) {
                Ok(qos) => qos,
                Err(_) => {
                    println!("Invalid QoS level: {}", qos);
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

            // Convert to high-level Publish with metadata
            let mut publish = Publish::from(Box::new(codec_publish))
                .create_time(timestamp_millis());

            // Create a synthetic "from" representing the system/API using proper RMQTT API
            let id = Id::new(1, 0, None, "127.0.0.1:0".parse().ok(), "js-api".into(), None);
            let from = From::from_system(id);

            println!("DEBUG: About to call hook_mgr().message_publish with topic: {}, from: {:?}", publish.topic, from);

            // First call the hook manager to trigger message_publish hooks - this is what we were missing!
            publish = context.extends.hook_mgr().message_publish(None, from.clone(), &publish).await.unwrap_or(publish);

            println!("DEBUG: Hook manager returned, now calling SessionState::forwards");

            // Then use SessionState::forwards to deliver the message
            let message_storage_available = context.extends.message_mgr().await.enable();
            let message_expiry_interval = Some(Duration::from_secs(3600)); // 1 hour default

            match SessionState::forwards(
                &context,
                from.clone(),
                publish.clone(),
                message_storage_available,
                message_expiry_interval,
            ).await {
                Ok(()) => {
                    println!("Successfully published message to topic: {}", topic);
                }
                Err(e) => {
                    println!("Failed to publish message to topic '{}': {}", topic, e);
                }
            }
        } else {
            println!("Server context not available for publishing after waiting");
        }
    }

    async fn start_server(config: ServerConfig, shared_state: SharedServerState) -> RmqttResult<()> {
        let mut context_builder = ServerContext::new();
        
        if let Some(plugins_dir) = config.plugins_config_dir {
            context_builder = context_builder.plugins_config_dir(&plugins_dir);
        }

        for (plugin_name, plugin_config) in config.plugins_config {
            context_builder = context_builder.plugins_config_map_add(&plugin_name, &plugin_config);
        }

        let scx = Arc::new(context_builder.build().await);
        
        // Store the context in shared state for publishing
        shared_state.set_context(scx.clone());
        
        // Register our JavaScript hook handler
        let hook_register = scx.extends.hook_mgr().register();
        hook_register.add(Type::MessagePublish, Box::new(JavaScriptHookHandler::new())).await;
        hook_register.add(Type::ClientSubscribe, Box::new(JavaScriptHookHandler::new())).await;
        hook_register.add(Type::ClientUnsubscribe, Box::new(JavaScriptHookHandler::new())).await;
        
        // IMPORTANT: Start the hook register to enable the handlers
        hook_register.start().await;
        
        let mut server_builder = RmqttServer::new(scx.as_ref().clone());

        for listener_config in config.listeners {
            let mut builder = Builder::new()
                .name(&listener_config.name)
                .laddr((listener_config.address.parse::<std::net::IpAddr>()
                    .unwrap_or_else(|_| "0.0.0.0".parse().unwrap()), listener_config.port).into())
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
                        format!("Invalid protocol: {}", listener_config.protocol)
                    ).into());
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

    fn send(
        &self,
        deferred: Deferred,
        callback: impl FnOnce(&Channel, Deferred) + Send + 'static,
    ) -> Result<(), mpsc::SendError<ServerMessage>> {
        self.tx
            .send(ServerMessage::Callback(deferred, Box::new(callback)))
    }

    fn start_with_config(
        &self,
        config: ServerConfig,
        deferred: Deferred,
        callback: impl FnOnce(&Channel, Deferred) + Send + 'static,
    ) -> Result<(), mpsc::SendError<ServerMessage>> {
        self.tx
            .send(ServerMessage::Start(config, deferred, Box::new(callback), self.shared_state.clone()))
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
        self.tx
            .send(ServerMessage::Publish { 
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

        server.start_with_config(config, deferred, |channel, deferred| {
            deferred.settle_with(channel, |mut cx| {
                Ok(cx.undefined())
            });
        })
        .into_rejection(&mut cx)?;

        Ok(promise)
    }

    fn js_stop(mut cx: FunctionContext) -> JsResult<JsPromise> {
        let server = cx.this::<JsBox<MqttServerWrapper>>()?;
        let (deferred, promise) = cx.promise();

        server.stop_server(deferred, |channel, deferred| {
            deferred.settle_with(channel, |mut cx| {
                Ok(cx.undefined())
            });
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

        server.publish_message(topic, payload, qos, retain, deferred, |channel, deferred| {
            deferred.settle_with(channel, |mut cx| {
                Ok(cx.undefined())
            });
        })
        .into_rejection(&mut cx)?;

        Ok(promise)
    }

    fn parse_config(cx: &mut FunctionContext, config_obj: Handle<JsObject>) -> NeonResult<ServerConfig> {
        let listeners_array = config_obj.get::<JsArray, _, _>(cx, "listeners")?;
        let mut listeners = Vec::new();

        for i in 0..listeners_array.len(cx) {
            let listener_obj = listeners_array.get::<JsObject, _, _>(cx, i)?;
            
            let name = listener_obj.get::<JsString, _, _>(cx, "name")?.value(cx);
            let address = listener_obj.get::<JsString, _, _>(cx, "address")?.value(cx);
            let port = listener_obj.get::<JsNumber, _, _>(cx, "port")?.value(cx) as u16;
            let protocol = listener_obj.get::<JsString, _, _>(cx, "protocol")?.value(cx);
            
            let tls_cert = listener_obj.get_opt::<JsString, _, _>(cx, "tlsCert")?
                .map(|s| s.value(cx));
            let tls_key = listener_obj.get_opt::<JsString, _, _>(cx, "tlsKey")?
                .map(|s| s.value(cx));
            let allow_anonymous = listener_obj.get_opt::<JsBoolean, _, _>(cx, "allowAnonymous")?
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

        let plugins_config_dir = config_obj.get_opt::<JsString, _, _>(cx, "pluginsConfigDir")?
            .map(|s| s.value(cx));

        let plugins_config = HashMap::new(); // TODO: Parse plugins config from JS object

        Ok(ServerConfig {
            listeners,
            plugins_config_dir,
            plugins_config,
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
            if let Ok(Some(on_message_publish)) = hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onMessagePublish") {
                callbacks.on_message_publish = Some(on_message_publish.root(&mut cx));
            }
            
            if let Ok(Some(on_client_subscribe)) = hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onClientSubscribe") {
                callbacks.on_client_subscribe = Some(on_client_subscribe.root(&mut cx));
            }
            
            if let Ok(Some(on_client_unsubscribe)) = hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onClientUnsubscribe") {
                callbacks.on_client_unsubscribe = Some(on_client_unsubscribe.root(&mut cx));
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
            match err.0 {
                ServerMessage::Callback(deferred, _) |
                ServerMessage::Start(_, deferred, _, _) |
                ServerMessage::Stop(deferred, _) |
                ServerMessage::Publish { deferred, .. } => {
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
