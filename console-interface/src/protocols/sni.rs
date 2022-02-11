tonic::include_proto!("_");
use grpc_web_client::Client;
use async_trait::async_trait;
use crate::protocols::protocol::{Device, Connection, ConnectionError};

pub struct SNIConnection {
    client: Client,
}

impl SNIConnection {
    pub fn new(uri: &str) -> Self {
        Self {
            client: Client::new(uri.to_string())
        }
    }
}

#[async_trait(?Send)]
impl Connection for SNIConnection {
    async fn connect(&self) -> Result<bool, ConnectionError>
    {
        // There's no way to really "connect" to a gRPC-service, so let's just
        // issue a list_devices command, and if that works we're good to go        
        self.list_devices().await?;
        Ok(true)
    }

    async fn disconnect(&self) -> Result<bool, ConnectionError> {
        Ok(true)
    }

    async fn list_devices(&self) -> Result<Vec<Device>, ConnectionError>
    {
        let mut client = devices_client::DevicesClient::new(self.client.clone());
        let mut devices = Vec::new();
        
        let request = tonic::Request::new(DevicesRequest {
            kinds: vec![]
        });
    
        let response = client.list_devices(request).await?;
        let response = response.into_inner();
        for d in &response.devices {
            devices.push(Device {
                name: d.display_name.to_string(),
                uri: d.uri.to_string()
            });
        }

        Ok(devices)
    }

    async fn read_memory(&self, device: &str, address: u32, size: u32) -> Result<Vec<u8>, ConnectionError> 
    {
        let mut client = device_memory_client::DeviceMemoryClient::new(self.client.clone());
        let request = tonic::Request::new(SingleReadMemoryRequest {
            request: Some(ReadMemoryRequest {
                request_address: address,
                request_address_space: AddressSpace::FxPakPro.into(),
                request_memory_mapping: MemoryMapping::ExHiRom.into(),
                size
            }),
            uri: device.into()
        });

        let response = client.single_read(request).await?.into_inner();
        let mem_response = response.response.ok_or("No MemoryResponse in Response")?;
        Ok(mem_response.data)
    }
}