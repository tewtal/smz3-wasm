use async_trait::async_trait;
use serde::Serialize;

pub type ConnectionError = Box<dyn std::error::Error>;

#[derive(Debug)]
pub enum Protocol {
    Sni,
    Usb2Snes
}

#[derive(Serialize)]
pub struct Device {
    pub name: String,
    pub uri: String,
    pub info: Option<Vec<String>>
} 

#[async_trait(?Send)]
pub trait Connection {
    async fn connect(&self) -> Result<bool, ConnectionError>;
    async fn disconnect(&self) -> Result<bool, ConnectionError>;
    async fn list_devices(&self) -> Result<Vec<Device>, ConnectionError>;
    async fn read_multi(&self, device: &str, address_info: &[u32]) -> Result<Vec<Vec<u8>>, ConnectionError>;
    async fn read_single(&self, device: &str, address: u32, size: u32) -> Result<Vec<u8>, ConnectionError>;
}

pub fn create_connection(protocol: &Protocol) -> Box<dyn Connection> {
    match protocol {
        Protocol::Sni => create_connection_with_uri(protocol, "http://127.0.0.1:8190"),
        Protocol::Usb2Snes => create_connection_with_uri(protocol, "ws://127.0.0.1:23074"),
    }
}

pub fn create_connection_with_uri(protocol: &Protocol, uri: &str) -> Box<dyn Connection> {
    match protocol {
        Protocol::Sni => Box::new(crate::protocols::sni::SNIConnection::new(uri)),
        Protocol::Usb2Snes => Box::new(crate::protocols::usb2snes::Usb2SnesConnection::new(uri))
    }
}
