use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use neon::{event::Channel, prelude::*, types::Deferred};
use rmqtt::{
    codec::types::Publish as CodecPublish,
    context::ServerContext,
    hook::Type,
    net::Builder,
    server::MqttServer as RmqttServer,
    session::SessionState,
    types::{From, Id, Publish, QoS},
    utils::timestamp_millis,
    Result as RmqttResult,
};

use crate::rs::hooks::{JavaScriptHookHandler, HOOK_CALLBACKS};

// Shared server state to enable publishing and hook callbacks
#[derive(Clone)]
pub struct SharedServerState {
    context: Arc<Mutex<Option<Arc<ServerContext>>>>,
    is_ready: Arc<Mutex<bool>>,
}

impl SharedServerState {
    pub fn new() -> Self {
        Self {
            context: Arc::new(Mutex::new(None)),
            is_ready: Arc::new(Mutex::new(false)),
        }
    }

    pub fn set_context(&self, ctx: Arc<ServerContext>) {
        if let Ok(mut context) = self.context.lock() { *context = Some(ctx); }
        if let Ok(mut ready) = self.is_ready.lock() { *ready = true; }
    }

    pub fn get_context(&self) -> Option<Arc<ServerContext>> {
        self.context.lock().ok().and_then(|c| c.clone())
    }

    pub fn is_ready(&self) -> bool {
        self.is_ready.lock().map(|r| *r).unwrap_or(false)
    }

    pub async fn wait_for_ready(&self, timeout_ms: u64) -> bool {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);
        loop {
            if self.is_ready() { return true; }
            if start.elapsed() >= timeout { return false; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    pub fn clear(&self) {
        if let Ok(mut context) = self.context.lock() { *context = None; }
        if let Ok(mut ready) = self.is_ready.lock() { *ready = false; }
    }
}

// Messages sent on the server channel
pub enum ServerMessage {
    Start(ServerConfig, Deferred, Box<dyn FnOnce(&Channel, Deferred) + Send>, SharedServerState),
    Stop(Deferred, Box<dyn FnOnce(&Channel, Deferred) + Send>),
    Publish { topic: String, payload: Vec<u8>, qos: u8, retain: bool, deferred: Deferred, callback: Box<dyn FnOnce(&Channel, Deferred) + Send>, shared_state: SharedServerState },
    Close,
}

#[derive(Debug, Clone)]
pub struct ListenerConfig {
    pub name: String,
    pub address: String,
    pub port: u16,
    pub protocol: String,
    pub tls_cert: Option<String>,
    pub tls_key: Option<String>,
    pub allow_anonymous: bool,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub listeners: Vec<ListenerConfig>,
    pub plugins_config_dir: Option<String>,
    pub plugins_config: HashMap<String, String>,
    pub plugins_default_startups: Option<Vec<String>>,
}

pub struct MqttServerWrapper {
    pub tx: mpsc::Sender<ServerMessage>,
    pub shared_state: SharedServerState,
}

impl Finalize for MqttServerWrapper {}

impl MqttServerWrapper {
    pub fn new<'a, C>(cx: &mut C) -> neon::result::NeonResult<Self>
    where
        C: Context<'a>,
    {
        let (tx, rx) = mpsc::channel::<ServerMessage>();
        let channel = cx.channel();
        let shared_state = SharedServerState::new();
        let shared_state_clone = shared_state.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
            let mut server_handle: Option<tokio::task::JoinHandle<RmqttResult<()>>> = None;

            while let Ok(message) = rx.recv() {
                match message {
                    ServerMessage::Start(config, deferred, callback, shared_state_for_start) => {
                        if let Some(handle) = server_handle.take() { handle.abort(); }
                        let server_config = config.clone();
                        server_handle = Some(rt.spawn(async move {
                            Self::start_server(server_config, shared_state_for_start).await
                        }));
                        callback(&channel, deferred);
                    }
                    ServerMessage::Stop(deferred, callback) => {
                        if let Some(handle) = server_handle.take() { handle.abort(); }
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
        if !shared_state.wait_for_ready(5000).await {
            eprintln!("WARN: Publish dropped because server context was not ready within 5s (topic: {})", topic);
            return;
        }
        if let Some(context) = shared_state.get_context() {
            let qos_num = qos;
            let qos = match QoS::try_from(qos_num) {
                Ok(qos) => qos,
                Err(_) => { eprintln!("ERROR: Invalid QoS value {} for topic {}. Dropping publish.", qos_num, topic); return; }
            };

            let codec_publish = CodecPublish { dup: false, retain, qos, topic: topic.clone().into(), packet_id: None, payload: payload.into(), properties: None };
            let mut publish = Publish::from(Box::new(codec_publish)).create_time(timestamp_millis());

            let id = Id::new(1, 0, None, "127.0.0.1:0".parse().ok(), "js-api".into(), None);
            let from = From::from_system(id);

            publish = context.extends.hook_mgr().message_publish(None, from.clone(), &publish).await.unwrap_or(publish);

            let message_storage_available = context.extends.message_mgr().await.enable();
            let message_expiry_interval = Some(Duration::from_secs(3600));

            if let Err(e) = SessionState::forwards(&context, from.clone(), publish.clone(), message_storage_available, message_expiry_interval).await {
                eprintln!("ERROR: Failed to forward message to topic {}: {:?}", topic, e);
            }
        } else {
            eprintln!("WARN: Publish dropped: no server context available (topic: {})", topic);
        }
    }

    async fn start_server(config: ServerConfig, shared_state: SharedServerState) -> RmqttResult<()> {
        let mut context_builder = ServerContext::new();
        if let Some(plugins_dir) = config.plugins_config_dir { context_builder = context_builder.plugins_config_dir(&plugins_dir); }
        for (plugin_name, plugin_config) in config.plugins_config { context_builder = context_builder.plugins_config_map_add(&plugin_name, &plugin_config); }
        let scx = std::sync::Arc::new(context_builder.build().await);

        if let Some(plugins) = config.plugins_default_startups { for plugin_name in plugins { match plugin_name.as_str() { _ => {} } } }

        shared_state.set_context(scx.clone());

        let hook_register = scx.extends.hook_mgr().register();
    hook_register.add(Type::ClientAuthenticate, Box::new(JavaScriptHookHandler::new())).await;
    // Official lifecycle hooks
    hook_register.add(Type::ClientConnect, Box::new(JavaScriptHookHandler::new())).await;
    hook_register.add(Type::ClientConnected, Box::new(JavaScriptHookHandler::new())).await;
    hook_register.add(Type::ClientDisconnected, Box::new(JavaScriptHookHandler::new())).await;
    hook_register.add(Type::ClientConnack, Box::new(JavaScriptHookHandler::new())).await;
    hook_register.add(Type::ClientKeepalive, Box::new(JavaScriptHookHandler::new())).await;
    hook_register.add(Type::SessionCreated, Box::new(JavaScriptHookHandler::new())).await;
    hook_register.add(Type::SessionTerminated, Box::new(JavaScriptHookHandler::new())).await;
    hook_register.add(Type::SessionSubscribed, Box::new(JavaScriptHookHandler::new())).await;
    // Register SessionUnsubscribed when available
    #[allow(unused_must_use)]
    {
        // If this Type exists in this rmqtt version, the following line compiles; otherwise, keep it commented out or feature-gated.
        // hook_register.add(Type::SessionUnsubscribed, Box::new(JavaScriptHookHandler::new())).await;
    }
        hook_register.add(Type::MessagePublishCheckAcl, Box::new(JavaScriptHookHandler::new())).await;
        hook_register.add(Type::MessagePublish, Box::new(JavaScriptHookHandler::new())).await;
        hook_register.add(Type::ClientSubscribeCheckAcl, Box::new(JavaScriptHookHandler::new())).await;
        hook_register.add(Type::ClientSubscribe, Box::new(JavaScriptHookHandler::new())).await;
        hook_register.add(Type::ClientUnsubscribe, Box::new(JavaScriptHookHandler::new())).await;
        // Optional message delivery lifecycle notifications (if supported by this RMQTT version)
        #[allow(unused_must_use)]
        {
            // If these Types are not available in this rmqtt version, remove or gate them.
            // They are present in newer versions to signal delivery/ack/drop events.
            // The `await` calls are kept inside this block to suppress unused_must_use in case of cfg gating.
            let _ = hook_register.add(Type::MessageDelivered, Box::new(JavaScriptHookHandler::new())).await;
            let _ = hook_register.add(Type::MessageAcked, Box::new(JavaScriptHookHandler::new())).await;
            let _ = hook_register.add(Type::MessageDropped, Box::new(JavaScriptHookHandler::new())).await;
        }
        hook_register.start().await;

        let mut server_builder = RmqttServer::new(scx.as_ref().clone());
        for listener_config in config.listeners {
            let mut builder = Builder::new()
                .name(&listener_config.name)
                .laddr((listener_config.address.parse::<std::net::IpAddr>().unwrap_or_else(|_| "0.0.0.0".parse().unwrap()), listener_config.port).into())
                .allow_anonymous(listener_config.allow_anonymous);

            if let Some(cert) = listener_config.tls_cert { builder = builder.tls_cert(Some(&cert)); }
            if let Some(key) = listener_config.tls_key { builder = builder.tls_key(Some(&key)); }

            let bound_builder = builder.bind()?;
            let listener = match listener_config.protocol.as_str() {
                "tcp" => bound_builder.tcp()?,
                "tls" => bound_builder.tls()?,
                "ws" => bound_builder.ws()?,
                "wss" => bound_builder.wss()?,
                _ => {
                    return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("Invalid protocol: {}", listener_config.protocol)).into());
                }
            };
            server_builder = server_builder.listener(listener);
        }

        server_builder.build().run().await
    }

    pub fn close(&self) -> Result<(), mpsc::SendError<ServerMessage>> {
        if let Ok(mut callbacks) = HOOK_CALLBACKS.lock() { callbacks.clear(); }
        self.tx.send(ServerMessage::Close)
    }

    pub fn start_with_config(
        &self,
        config: ServerConfig,
        deferred: Deferred,
        callback: impl FnOnce(&Channel, Deferred) + Send + 'static,
    ) -> Result<(), mpsc::SendError<ServerMessage>> {
        self.tx.send(ServerMessage::Start(config, deferred, Box::new(callback), self.shared_state.clone()))
    }

    pub fn stop_server(&self, deferred: Deferred, callback: impl FnOnce(&Channel, Deferred) + Send + 'static) -> Result<(), mpsc::SendError<ServerMessage>> {
        self.tx.send(ServerMessage::Stop(deferred, Box::new(callback)))
    }

    pub fn publish_message(
        &self,
        topic: String,
        payload: Vec<u8>,
        qos: u8,
        retain: bool,
        deferred: Deferred,
        callback: impl FnOnce(&Channel, Deferred) + Send + 'static,
    ) -> Result<(), mpsc::SendError<ServerMessage>> {
        self.tx.send(ServerMessage::Publish { topic, payload, qos, retain, deferred, callback: Box::new(callback), shared_state: self.shared_state.clone() })
    }
}

pub trait SendResultExt {
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
// moved from src/server.rs
// ... file contents preserved ...