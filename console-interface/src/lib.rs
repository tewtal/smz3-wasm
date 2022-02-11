#![allow(clippy::unused_unit)]

use wasm_bindgen::prelude::*;
use js_sys::{Promise, Uint8Array};
use protocols::protocol::{Connection, Protocol, create_connection, create_connection_with_uri};
use wasm_bindgen_futures::{future_to_promise};
use std::sync::{Arc};

mod protocols;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub struct ConsoleInterface {
    connection: Arc<Box<dyn Connection>>
}

#[wasm_bindgen]
impl ConsoleInterface {    
    #[wasm_bindgen(constructor)]
    pub fn new(proto: String, uri: Option<String>) -> Self {
        let protocol = match proto.to_lowercase().as_str() {
            "sni" => Protocol::Sni,
            _ => Protocol::Usb2Snes,
        };

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

    #[wasm_bindgen]
    pub fn connect(&self) -> Promise {
        let conn = self.connection.clone();
        future_to_promise(async move {
            conn.connect().await.map_err(|_| "Could not connect to device")?;
            Ok(JsValue::TRUE)
        })
    }

    #[wasm_bindgen]
    pub fn disconnect(&self) -> Promise {
        let conn = self.connection.clone();
        future_to_promise(async move {
            conn.disconnect().await.map_err(|_| "Could not disconnect from device")?;
            Ok(JsValue::TRUE)
        })
    }

    #[wasm_bindgen]
    pub fn list_devices(&self) -> Promise {
        let conn = self.connection.clone();
        future_to_promise(async move {
            let devices = conn.list_devices().await.map_err(|e| format!("Device list request failed: {:?}", e))?;
            let js_devices = serde_wasm_bindgen::to_value(&devices).map_err(|_| JsValue::from("Could not parse device list"))?;
            Ok(js_devices)
        })
    }

    #[wasm_bindgen]
    pub fn read_memory(&self, device: String, address: u32, size: u32) -> Promise {
        let conn = self.connection.clone();
        future_to_promise(async move {
            let data = conn.read_memory(&device, address, size).await.map_err(|e| format!("Read memory request failed: {:?}", e))?;
            Ok(JsValue::from(Uint8Array::from(data.as_slice())))
        })
    }
}