use std::sync::mpsc;
use std::thread;
use std::collections::HashMap;

use neon::{prelude::*, types::Deferred};
use rmqtt::{context::ServerContext, net::Builder, server::MqttServer as RmqttServer, Result as RmqttResult};

type ServerCallback = Box<dyn FnOnce(&Channel, Deferred) + Send>;

// Wraps an RMQTT server instance with async communication
struct MqttServerWrapper {
    tx: mpsc::Sender<ServerMessage>,
}

// Messages sent on the server channel
enum ServerMessage {
    // Promise to resolve and callback to be executed
    Callback(Deferred, ServerCallback),
    // Start the server with given configuration
    Start(ServerConfig, Deferred, ServerCallback),
    // Stop the server
    Stop(Deferred, ServerCallback),
    // Indicates that the thread should be stopped
    Close,
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

        // Spawn a thread for managing the MQTT server
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
            let mut server_handle: Option<tokio::task::JoinHandle<RmqttResult<()>>> = None;

            while let Ok(message) = rx.recv() {
                match message {
                    ServerMessage::Callback(deferred, f) => {
                        f(&channel, deferred);
                    }
                    ServerMessage::Start(config, deferred, callback) => {
                        // Stop existing server if running
                        if let Some(handle) = server_handle.take() {
                            handle.abort();
                        }

                        // Start new server
                        let server_config = config.clone();
                        server_handle = Some(rt.spawn(async move {
                            Self::start_server(server_config).await
                        }));

                        callback(&channel, deferred);
                    }
                    ServerMessage::Stop(deferred, callback) => {
                        if let Some(handle) = server_handle.take() {
                            handle.abort();
                        }
                        callback(&channel, deferred);
                    }
                    ServerMessage::Close => break,
                }
            }
        });

        Ok(Self { tx })
    }

    async fn start_server(config: ServerConfig) -> RmqttResult<()> {
        let mut context_builder = ServerContext::new();
        
        if let Some(plugins_dir) = config.plugins_config_dir {
            context_builder = context_builder.plugins_config_dir(&plugins_dir);
        }

        for (plugin_name, plugin_config) in config.plugins_config {
            context_builder = context_builder.plugins_config_map_add(&plugin_name, &plugin_config);
        }

        let scx = context_builder.build().await;
        let mut server_builder = RmqttServer::new(scx);

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
            .send(ServerMessage::Start(config, deferred, Box::new(callback)))
    }

    fn stop_server(
        &self,
        deferred: Deferred,
        callback: impl FnOnce(&Channel, Deferred) + Send + 'static,
    ) -> Result<(), mpsc::SendError<ServerMessage>> {
        self.tx
            .send(ServerMessage::Stop(deferred, Box::new(callback)))
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
                ServerMessage::Start(_, deferred, _) |
                ServerMessage::Stop(deferred, _) => {
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

    Ok(())
}
