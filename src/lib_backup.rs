use std::sync::mpsc;
use std::thread;
use std::collections::HashMap;

use neon::{prelude::*, types::Deferred};
use rmqtt::{
    context::ServerContext, 
    net::Builder, 
    server::MqttServer as RmqttServer, 
    Result as RmqttResult,
};

type ServerCallback = Box<dyn FnOnce(&Channel, Deferred) + Send>;

#[derive(Debug)]
enum ServerMessage {
    Start { 
        config: Config,
        callback: ServerCallback 
    },
    Stop { 
        callback: ServerCallback 
    },
}

#[derive(Debug)]
struct Config {
    listeners: Vec<ListenerConfig>,
}

#[derive(Debug)]
struct ListenerConfig {
    name: String,
    address: String,
    port: u16,
    protocol: String,
    tls_config: Option<TlsConfig>,
}

#[derive(Debug)]
struct TlsConfig {
    cert_file: String,
    key_file: String,
}

struct MqttServerWrapper {
    sender: mpsc::Sender<ServerMessage>,
    _thread_handle: thread::JoinHandle<()>,
}

impl Finalize for MqttServerWrapper {}

impl MqttServerWrapper {
    fn new<'a, C>(cx: &mut C) -> neon::result::NeonResult<Self>
    where
        C: Context<'a>,
    {
        let (tx, rx) = mpsc::channel::<ServerMessage>();
        
        let thread_handle = thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            
            rt.block_on(async {
                let mut mqtt_server: Option<RmqttServer> = None;
                
                while let Ok(message) = rx.recv() {
                    match message {
                        ServerMessage::Start { config, callback } => {
                            let channel = cx.channel();
                            
                            match Self::create_server(config).await {
                                Ok(server) => {
                                    mqtt_server = Some(server);
                                    channel.send(move |mut cx| {
                                        let deferred = callback.into_inner(&mut cx)?;
                                        deferred.settle_with(&cx.channel(), move |mut cx| {
                                            Ok(cx.undefined())
                                        });
                                        Ok(())
                                    });
                                }
                                Err(e) => {
                                    let error_msg = format!("Failed to start server: {:?}", e);
                                    channel.send(move |mut cx| {
                                        let deferred = callback.into_inner(&mut cx)?;
                                        deferred.settle_with(&cx.channel(), move |mut cx| {
                                            cx.throw_error(error_msg)
                                        });
                                        Ok(())
                                    });
                                }
                            }
                        }
                        ServerMessage::Stop { callback } => {
                            if let Some(mut server) = mqtt_server.take() {
                                let _ = server.stop().await;
                            }
                            
                            let channel = cx.channel();
                            channel.send(move |mut cx| {
                                let deferred = callback.into_inner(&mut cx)?;
                                deferred.settle_with(&cx.channel(), move |mut cx| {
                                    Ok(cx.undefined())
                                });
                                Ok(())
                            });
                        }
                    }
                }
            });
        });

        Ok(MqttServerWrapper {
            sender: tx,
            _thread_handle: thread_handle,
        })
    }

    async fn create_server(config: Config) -> RmqttResult<RmqttServer> {
        let mut scx = ServerContext::new();
        
        for listener_config in config.listeners {
            let mut builder = Builder::new();
            
            builder
                .id(listener_config.name.clone())
                .addr(format!("{}:{}", listener_config.address, listener_config.port));
            
            match listener_config.protocol.as_str() {
                "tcp" => {
                    builder.tcp();
                }
                "tls" => {
                    if let Some(tls_cfg) = listener_config.tls_config {
                        builder.tls(tls_cfg.cert_file, tls_cfg.key_file);
                    } else {
                        return Err(rmqtt::Error::from("TLS configuration required for TLS protocol"));
                    }
                }
                "ws" => {
                    builder.ws();
                }
                "wss" => {
                    if let Some(tls_cfg) = listener_config.tls_config {
                        builder.wss(tls_cfg.cert_file, tls_cfg.key_file);
                    } else {
                        return Err(rmqtt::Error::from("TLS configuration required for WSS protocol"));
                    }
                }
                _ => {
                    return Err(rmqtt::Error::from(format!("Unsupported protocol: {}", listener_config.protocol)));
                }
            }
            
            scx.add_listner(builder);
        }
        
        let server = scx.build().await?;
        Ok(server)
    }
}

fn mqtt_server_new(mut cx: FunctionContext) -> JsResult<JsBox<MqttServerWrapper>> {
    let wrapper = MqttServerWrapper::new(&mut cx)?;
    Ok(cx.boxed(wrapper))
}

fn mqtt_server_start(mut cx: FunctionContext) -> JsResult<JsPromise> {
    let wrapper = cx.argument::<JsBox<MqttServerWrapper>>(0)?;
    let config_js = cx.argument::<JsObject>(1)?;
    
    // Parse config
    let listeners_js = config_js.get::<JsArray, _, _>(&mut cx, "listeners")?;
    let listeners_len = listeners_js.len(&mut cx);
    
    let mut listeners = Vec::new();
    for i in 0..listeners_len {
        let listener_js: Handle<JsObject> = listeners_js.get(&mut cx, i)?;
        
        let name: Handle<JsString> = listener_js.get(&mut cx, "name")?;
        let address: Handle<JsString> = listener_js.get(&mut cx, "address")?;
        let port: Handle<JsNumber> = listener_js.get(&mut cx, "port")?;
        let protocol: Handle<JsString> = listener_js.get(&mut cx, "protocol")?;
        
        let mut tls_config = None;
        if let Ok(tls_js) = listener_js.get::<JsObject, _, _>(&mut cx, "tls") {
            let cert_file: Handle<JsString> = tls_js.get(&mut cx, "certFile")?;
            let key_file: Handle<JsString> = tls_js.get(&mut cx, "keyFile")?;
            
            tls_config = Some(TlsConfig {
                cert_file: cert_file.value(&mut cx),
                key_file: key_file.value(&mut cx),
            });
        }
        
        listeners.push(ListenerConfig {
            name: name.value(&mut cx),
            address: address.value(&mut cx),
            port: port.value(&mut cx) as u16,
            protocol: protocol.value(&mut cx),
            tls_config,
        });
    }
    
    let config = Config { listeners };
    
    let (deferred, promise) = cx.promise();
    let callback = Box::new(move |channel: &Channel, deferred: Deferred| {
        deferred.settle_with(channel, move |mut cx| {
            Ok(cx.undefined())
        });
    });
    
    wrapper.sender.send(ServerMessage::Start { config, callback })
        .or_else(|err| cx.throw_error(err.to_string()))?;
    
    Ok(promise)
}

fn mqtt_server_stop(mut cx: FunctionContext) -> JsResult<JsPromise> {
    let wrapper = cx.argument::<JsBox<MqttServerWrapper>>(0)?;
    
    let (deferred, promise) = cx.promise();
    let callback = Box::new(move |channel: &Channel, deferred: Deferred| {
        deferred.settle_with(channel, move |mut cx| {
            Ok(cx.undefined())
        });
    });
    
    wrapper.sender.send(ServerMessage::Stop { callback })
        .or_else(|err| cx.throw_error(err.to_string()))?;
    
    Ok(promise)
}

#[neon::main]
fn main(mut cx: ModuleContext) -> NeonResult<()> {
    cx.export_function("mqtt_server_new", mqtt_server_new)?;
    cx.export_function("mqtt_server_start", mqtt_server_start)?;
    cx.export_function("mqtt_server_stop", mqtt_server_stop)?;
    
    Ok(())
}