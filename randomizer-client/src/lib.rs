#![allow(clippy::unused_unit)]
use futures_locks::RwLock;
use js_sys::{Promise, Function};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;
use services::randomizer::{RandomizerService};
use console_interface::protocols::protocol::{self, ConnectionError};
pub use console_interface::ConsoleInterface;

mod clients;
mod services;

// Use `wee_alloc` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

static LOG_LEVEL: log::Level = if cfg!(debug_assertions) { log::Level::Debug } else { log::Level::Info };

#[wasm_bindgen]
pub enum Message {
    ConsoleDisconnected,
    ConsoleReconnecting,
    ConsoleConnected,
    ConsoleError
}
impl Message {
    // Send a message to a JS callback that something has happened
    pub async fn send(callback: &Function, message: Message, args: Option<&[&str]>) {
        let args = serde_wasm_bindgen::to_value(&args).unwrap_or_default();
        let _ = callback.call2(&JsValue::NULL, &JsValue::from(message as i32), &args);
    }
}

pub struct ClientContext {
    console_connection: Option<Box<dyn protocol::Connection>>,
    randomizer_service: RandomizerService,
    session: Option<services::randomizer::GetSessionResponse>,
    client: Option<services::randomizer::RegisterPlayerResponse>,
    device: String,
    connected: bool,
    session_guid: String,
    callback: Function
}

#[wasm_bindgen]
pub struct RandomizerClient {
    context: RwLock<ClientContext>,
    game_client: RwLock<Option<clients::multiworld::smz3::SMZ3Client>>
}

#[wasm_bindgen]
impl RandomizerClient {
    pub fn init() {
        wasm_logger::init(wasm_logger::Config::new(LOG_LEVEL));
    }

    async fn initialize_console_connection() -> Result<Box<dyn protocol::Connection>, Box<dyn std::error::Error>> {
        /* Can we connect with SNI gRPC? */
        log::debug!("client: Attempting to connect with SNI");
        let sni_connection = protocol::create_connection(&protocol::Protocol::Sni);
        if sni_connection.connect().await.is_ok() {
            return Ok(sni_connection)
        }

        /* Can we connect with USB2SNES on the new port? */
        log::debug!("client: Attempting to connect with USB2SNES");
        let usb_connection = protocol::create_connection_with_uri(&protocol::Protocol::Usb2Snes, "ws://localhost:23074");
        if usb_connection.connect().await.is_ok() {
            return Ok(usb_connection)
        }

        /* Can we connect with USB2SNES on the old port? */
        log::debug!("client: Attempting to connect with Legacy USB2SNES");
        let legacy_connection = protocol::create_connection_with_uri(&protocol::Protocol::Usb2Snes, "ws://localhost:8080");
        if legacy_connection.connect().await.is_ok() {
            return Ok(legacy_connection)
        }

        Err("Could not connect to any console device".into())
    }

    #[wasm_bindgen(constructor)]
    pub fn new(session_uri: String, session_guid: String, callback: Function) -> Self {
        Self {
            game_client: RwLock::new(None),
            context: RwLock::new(ClientContext {
                console_connection: None,
                randomizer_service: RandomizerService::new(&session_uri),
                session: None,
                client: None,
                device: String::new(),
                connected: false,
                session_guid,
                callback
            }),
         }
    }

    pub fn initialize(&self) -> Promise {
        let m_ctx = self.context.clone();
        future_to_promise(async move {
            let mut ctx = m_ctx.write().await;            
            ctx.session = Some(ctx.randomizer_service.get_session(&ctx.session_guid).await.map_err(|e| format!("Could not retrieve session data: {:?}", e.message()))?);
            serde_wasm_bindgen::to_value(&ctx.session).map_err(|_| JsValue::from("Could not parse session data"))
        })
    }

    pub fn get_session_data(&self) -> Promise {
        let m_ctx = self.context.clone();
        future_to_promise(async move {
            let ctx = m_ctx.read().await;            
            serde_wasm_bindgen::to_value(&ctx.session).map_err(|_| JsValue::from("Could not parse session data"))
        })        
    }

    pub fn register_player(&self, world_id: i32) -> Promise {
        let m_ctx = self.context.clone();
        future_to_promise(async move {
            let mut ctx = m_ctx.write().await;
            ctx.client = Some(ctx.randomizer_service.register_player(&ctx.session_guid, world_id).await.map_err(|e| format!("Could not register player: {:?}", e.message()))?);
            serde_wasm_bindgen::to_value(&ctx.client).map_err(|_| JsValue::from("Could not parse client data"))
        })
    }

    pub fn login_player(&self, client_guid: String) -> Promise {
        let m_ctx = self.context.clone();
        future_to_promise(async move {
            let mut ctx = m_ctx.write().await;
            ctx.client = Some(ctx.randomizer_service.login_player(&ctx.session_guid, &client_guid).await.map_err(|e| format!("Could not login player: {:?}", e.message()))?);
            serde_wasm_bindgen::to_value(&ctx.client).map_err(|_| JsValue::from("Could not parse client data"))
        })
    }

    pub fn get_client_data(&self) -> Promise {
        let m_ctx = self.context.clone();
        future_to_promise(async move {
            let ctx = m_ctx.read().await;
            serde_wasm_bindgen::to_value(&ctx.client).map_err(|_| JsValue::from("Could not parse client data"))
        })
    }

    pub fn get_patch(&self) -> Promise {
        let m_ctx = self.context.clone();
        future_to_promise(async move {
            let ctx = m_ctx.read().await;
            if let Some(client) = ctx.client.as_ref() {        
                let data = ctx.randomizer_service.get_patch(&client.client_token).await.map_err(|e| format!("Could not get patch data: {:?}", e.message()))?;
                serde_wasm_bindgen::to_value(&data.patch_data).map_err(|_| JsValue::from("Could not parse patch data"))
            } else {
                Err(JsValue::from("Not registered to a session yet."))
            }
        })
    }

    pub fn list_devices(&self) -> Promise {
        let m_ctx = self.context.clone();
        future_to_promise(async move {
            let mut ctx = m_ctx.write().await;

            let connection = match &ctx.console_connection {
                Some(conn) => conn,
                None => {
                    let conn = Self::initialize_console_connection().await.map_err(|e| JsValue::from(format!("Could not initialize a console connection: {:?}", e)))?;
                    ctx.console_connection = Some(conn);
                    ctx.connected = true;
                    Message::send(&ctx.callback, Message::ConsoleConnected, Some(&[&ctx.device])).await;
                    ctx.console_connection.as_ref().unwrap()
                }
            };

            let devices = connection.list_devices().await.map_err(|_| JsValue::from("Could not get device list"))?;
            serde_wasm_bindgen::to_value(&devices).map_err(|_| JsValue::from("Could not parse device list data"))
        })
    }

    pub fn get_events(&self, event_types: Vec<i32>, from_event_id: Option<i32>, to_event_id: Option<i32>, from_world_id: Option<i32>, to_world_id: Option<i32>) -> Promise {
        let m_ctx = self.context.clone();
        future_to_promise(async move {
            let ctx = m_ctx.read().await;
            let events = ctx.randomizer_service.get_events(&ctx.client.as_ref().unwrap().client_token, &event_types, from_event_id, to_event_id, from_world_id, to_world_id).await.map_err(|e| format!("Could not get patch data: {:?}", e.message()))?;
            serde_wasm_bindgen::to_value(&events).map_err(|_| JsValue::from("Could not parse event data"))
        })
    }

    pub fn start(&self, device: String) -> Promise {
        let m_ctx = self.context.clone();
        let m_cli = self.game_client.clone();
        future_to_promise(async move {            
            {
                let mut ctx = m_ctx.write().await;
                ctx.device = device;            
            }
            
            let ctx = m_ctx.read().await;
            let mut cli = m_cli.write().await;
            let session = ctx.session.as_ref().ok_or_else(|| JsValue::from("Could not get session data, make sure a session is established before running start"))?;
            let seed = session.seed.as_ref().ok_or_else(|| JsValue::from("Could not get seed data from session"))?;
            
            // Get the correct client depending on the game and game mode
            // TODO: Turn client into a dyn trait
            *cli = match (seed.game_id.to_lowercase().as_str(), seed.game_mode.to_lowercase().as_str()) {
                ("smz3", "multiworld") => Some(clients::multiworld::smz3::SMZ3Client::new()),
                _ => None
            };

            Ok(JsValue::TRUE)
        })
    }

    pub fn update(&self) -> Promise {
        let mut_ctx = self.context.clone();
        let client_ctx = self.game_client.clone();
        future_to_promise(async move {
            let mut ctx = mut_ctx.write().await;            
            if let Some(cli) = client_ctx.write().await.as_mut() {

                // If we're not connected, that means we got disconnected from the console during an update
                // and we'll have to attempt to reconnect here

                if !ctx.connected {
                    Message::send(&ctx.callback, Message::ConsoleReconnecting, None).await;
                    let conn = ctx.console_connection.as_ref().ok_or_else(|| JsValue::from("Tried to reconnect, but no client available?"))?;
                    let _ = conn.connect().await.map_err(|_| JsValue::from("Could not connect to device"))?;
                    let devices = conn.list_devices().await.map_err(|_| JsValue::from("Could not list devices"))?;
                    
                    if devices.is_empty() {
                        return Err(JsValue::from("Could get device list, but it's empty, trying again later"));
                    } else {
                        // Ok, there's a few devices, if the previous one we're connected to is there do nothing
                        if !devices.iter().any(|d| d.uri == ctx.device) {
                            // Otherwise we need to check if there's more than one, in that case we don't know what to do here
                            if devices.len() > 1 {
                                return Err(JsValue::from("Could get device list, but there's more than one device"));
                            } else {
                                // Only one to pick from, so let's take that one
                                ctx.device = devices[0].uri.to_string();
                            }
                        }
                    }

                    // ok, we're back and we got a device setup, continue as normal
                    ctx.connected = true;
                    Message::send(&ctx.callback, Message::ConsoleConnected, Some(&[&ctx.device])).await;
                }

                match cli.update(&ctx).await {
                    Err(e) => {
                        if e.downcast_ref::<ConnectionError>().is_some() {                            
                            // If we get a connection error, something bad happened to the device, we'll have to back off completely
                            // and try to reconnect to the first available device, if there are more than one device when we try to
                            // auto-reconnect we'll just completely bail out.
                            
                            Message::send(&ctx.callback, Message::ConsoleDisconnected, None).await;
                            let conn = ctx.console_connection.as_ref().ok_or_else(|| JsValue::from("Tried to reconnect, but no client available?"))?;
                            if let Err(e) = conn.disconnect().await {
                                log::debug!("Got error when attempting to close the connection: {:?}", e);
                            }
                            
                            ctx.connected = false;
                            Err(JsValue::from("The console connection has disconnected, attempting to reconnect"))
                        } else {
                            Err(JsValue::from(format!("Update error: {:?}", e)))
                        }
                    },
                    _ => Ok(JsValue::TRUE)
                }
            } else {
                Err(JsValue::from("No game client initialized, run start first"))
            }
        })
    }
}
