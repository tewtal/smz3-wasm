use futures_locks::RwLock;
use js_sys::{Promise};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;
use services::randomizer::{RandomizerService};
use console_interface::protocols::protocol;
pub use console_interface::ConsoleInterface;

mod clients;
mod services;

// Use `wee_alloc` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

static LOG_LEVEL: log::Level = if cfg!(debug_assertions) { log::Level::Debug } else { log::Level::Info };

#[wasm_bindgen]
pub enum Message {
    Disconnected
}

pub struct ClientContext {
    console_connection: Option<Box<dyn protocol::Connection>>,
    randomizer_service: RandomizerService,
    session: Option<services::randomizer::GetSessionResponse>,
    client: Option<services::randomizer::RegisterPlayerResponse>,
    device: String,
    session_guid: String,
}

#[wasm_bindgen]
pub struct RandomizerClient {
    context: RwLock<ClientContext>
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
    pub fn new(session_uri: String, session_guid: String) -> Self {
        Self {
            context: RwLock::new(ClientContext {
                console_connection: None,
                randomizer_service: RandomizerService::new(&session_uri),
                session: None,
                client: None,
                device: String::new(),
                session_guid
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
                    let conn = Self::initialize_console_connection().await.map_err(|e| JsValue::from(format!("Could not intiialize a console connection: {:?}", e)))?;
                    ctx.console_connection = Some(conn);
                    ctx.console_connection.as_ref().unwrap()
                }
            };

            let devices = connection.list_devices().await.map_err(|_| JsValue::from("Could not get device list"))?;
            serde_wasm_bindgen::to_value(&devices).map_err(|_| JsValue::from("Could not parse device list data"))
        })
    }

    pub fn start(&self, device: String) -> Promise {
        let m_ctx = self.context.clone();
        future_to_promise(async move {
            let mut ctx = m_ctx.write().await;
            ctx.device = device;
            crate::clients::multiworld::smz3::smz3(&ctx).await;
            Ok(JsValue::TRUE)
        })
    }
}
