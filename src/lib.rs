use neon::prelude::*;
use neon::types::buffer::TypedArray;
use std::collections::HashMap;

mod rs;

use rs::{MqttServerWrapper, SendResultExt, ServerConfig};

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

            listeners.push(rs::ListenerConfig {
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
        let channel = cx.channel();
        if let Ok(mut callbacks) = crate::rs::HOOK_CALLBACKS.lock() {
            callbacks.channel = Some(channel);
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
            if let Ok(Some(on_client_publish_authorize)) =
                hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onClientPublishAuthorize")
            {
                callbacks.on_client_publish_authorize =
                    Some(on_client_publish_authorize.root(&mut cx));
            }
            if let Ok(Some(on_message_delivered)) =
                hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onMessageDelivered")
            {
                callbacks.on_message_delivered = Some(on_message_delivered.root(&mut cx));
            }
            if let Ok(Some(on_message_acked)) =
                hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onMessageAcked")
            {
                callbacks.on_message_acked = Some(on_message_acked.root(&mut cx));
            }
            if let Ok(Some(on_message_dropped)) =
                hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onMessageDropped")
            {
                callbacks.on_message_dropped = Some(on_message_dropped.root(&mut cx));
            }
            // Lifecycle hooks (optional) aligned to RMQTT
            if let Ok(Some(on_client_connect)) =
                hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onClientConnect")
            {
                callbacks.on_client_connect = Some(on_client_connect.root(&mut cx));
            }
            if let Ok(Some(on_client_connack)) =
                hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onClientConnack")
            {
                callbacks.on_client_connack = Some(on_client_connack.root(&mut cx));
            }
            if let Ok(Some(on_client_connected)) =
                hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onClientConnected")
            {
                callbacks.on_client_connected = Some(on_client_connected.root(&mut cx));
            }
            if let Ok(Some(on_client_disconnected)) =
                hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onClientDisconnected")
            {
                callbacks.on_client_disconnected = Some(on_client_disconnected.root(&mut cx));
            }
            if let Ok(Some(on_session_created)) =
                hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onSessionCreated")
            {
                callbacks.on_session_created = Some(on_session_created.root(&mut cx));
            }
            if let Ok(Some(on_session_subscribed)) =
                hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onSessionSubscribed")
            {
                callbacks.on_session_subscribed = Some(on_session_subscribed.root(&mut cx));
            }
            if let Ok(Some(on_session_unsubscribed)) =
                hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onSessionUnsubscribed")
            {
                callbacks.on_session_unsubscribed = Some(on_session_unsubscribed.root(&mut cx));
            }
            if let Ok(Some(on_session_terminated)) =
                hooks_obj.get_opt::<JsFunction, _, _>(&mut cx, "onSessionTerminated")
            {
                callbacks.on_session_terminated = Some(on_session_terminated.root(&mut cx));
            }
        }

        Ok(cx.undefined())
    }
}

// SendResultExt is implemented in server.rs; no duplication here.

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
