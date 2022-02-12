tonic::include_proto!("randomizer");

use js_sys::Function;
use serde::Serialize;
use wasm_bindgen::prelude::*;

// Re-export ConsoleInterface from this crate for manual use if needed
pub use console_interface::ConsoleInterface;

mod clients;

// Use `wee_alloc` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub enum Message {
    Disconnected
}

#[wasm_bindgen]
pub async fn get_session(session_guid: String, callback: Function) -> JsValue {
    let web_client = grpc_web_client::Client::new("https://localhost:7108".to_string());
    let mut client = randomizer_client::RandomizerClient::new(web_client);

    let request = tonic::Request::new(GetSessionRequest {
        session_guid: session_guid.to_string()
    });

    let response = client.get_session(request).await.unwrap().into_inner();
        
    let window = web_sys::window().unwrap();
    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_2(
        &callback,
        1000,
        &JsValue::from(Message::Disconnected as i32),
        &JsValue::from("Hello World".to_string())
    );

    serde_wasm_bindgen::to_value(&response).unwrap()
}
