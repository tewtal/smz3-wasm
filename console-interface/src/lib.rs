#![allow(clippy::unused_unit)]
use wasm_bindgen::prelude::*;
use js_sys::{Promise, Uint8Array, Array};
use protocols::protocol::{Connection, Protocol, create_connection, create_connection_with_uri};
use wasm_bindgen_futures::{future_to_promise};
use std::iter::FromIterator;
use std::sync::{Arc};

pub mod protocols;

// #[global_allocator]
// static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

static LOG_LEVEL: log::Level = if cfg!(debug_assertions) { log::Level::Debug } else { log::Level::Info };

#[wasm_bindgen]
pub struct ConsoleInterface {
    connection: Arc<Box<dyn Connection>>
}

#[wasm_bindgen]
impl ConsoleInterface {
    #[wasm_bindgen]
    pub fn init() {
        wasm_logger::init(wasm_logger::Config::new(LOG_LEVEL));
    }

    #[wasm_bindgen(constructor)]
    pub fn new(proto: String, uri: Option<String>) -> Self {
        let protocol = match proto.to_lowercase().as_str() {
            "sni" => Protocol::Sni,
            _ => Protocol::Usb2Snes,
        };

        log::debug!("Created ConsoleInterface [{:?}] - {:?}", &protocol, &uri);

        Self {
            connection: Arc::new(
                if let Some(uri) = uri {
                    create_connection_with_uri(&protocol, &uri)
                } else {
                    create_connection(&protocol)
                }
            ),
        }
    }

    pub fn connect(&self) -> Promise {
        let conn = self.connection.clone();
        future_to_promise(async move {
            conn.connect().await.map_err(|_| "Could not connect to device")?;
            Ok(JsValue::TRUE)
        })
    }

    pub fn disconnect(&self) -> Promise {
        let conn = self.connection.clone();
        future_to_promise(async move {
            conn.disconnect().await.map_err(|_| "Could not disconnect from device")?;
            Ok(JsValue::TRUE)
        })
    }

    pub fn list_devices(&self) -> Promise {
        let conn = self.connection.clone();
        future_to_promise(async move {
            let devices = conn.list_devices().await.map_err(|e| format!("Device list request failed: {:?}", e))?;
            serde_wasm_bindgen::to_value(&devices).map_err(|_| JsValue::from("Could not parse device list"))
        })
    }

    pub fn read(&self, device: String, address: u32, size: u32) -> Promise {
        let conn = self.connection.clone();
        future_to_promise(async move {
            let data = conn.read_single(&device, address, size).await.map_err(|e| format!("Read memory request failed: {:?}", e))?;            
            Ok(JsValue::from(Uint8Array::from(data.as_slice())))
        })
    }

    pub fn read_multi(&self, device: String, address_info: Vec<u32>) -> Promise {
        let conn = self.connection.clone();
        future_to_promise(async move {
            let data = conn.read_multi(&device, &address_info).await.map_err(|e| format!("Read memory request failed: {:?}", e))?;
            let js_data = Array::from_iter(data.iter().map(|d| Uint8Array::from(d.as_slice())));
            Ok(JsValue::from(js_data))
        })
    }

    pub fn write(&self, device: String, address: u32, data: Uint8Array) -> Promise {
        let conn = self.connection.clone();
        future_to_promise(async move {
            conn.write_single(&device, address, &data.to_vec()).await.map_err(|e| format!("Write memory request failed: {:?}", e))?;            
            Ok(JsValue::TRUE)
        })
    }

    pub fn write_multi(&self, device: String, addresses: Vec<u32>, data: Vec<Uint8Array>) -> Promise {
        let conn = self.connection.clone();
        future_to_promise(async move {
            let data: Vec<Vec<u8>> = data.iter().map(|d| d.to_vec()).collect();
            conn.write_multi(&device, &addresses, &data).await.map_err(|e| format!("Write memory request failed: {:?}", e))?;
            Ok(JsValue::TRUE)
        })
    }
}