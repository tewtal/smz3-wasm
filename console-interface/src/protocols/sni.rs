tonic::include_proto!("_");

use grpc_web_client::Client;
use async_trait::async_trait;
use std::sync::Arc;
use std::collections::HashMap;
use futures::lock::Mutex;

use crate::protocols::protocol::{Device, Connection, ConnectionError};

pub struct SNIConnection {
    client: Client,
    mappings: Arc<Mutex<HashMap<String, i32>>>
}

impl SNIConnection {
    pub fn new(uri: &str) -> Self {
        Self {
            client: Client::new(uri.to_string()),
            mappings: Arc::new(Mutex::new(HashMap::new()))
        }
    }

    async fn get_mapping(&self, device: &str) -> Result<i32, ConnectionError> {
        let mut client = device_memory_client::DeviceMemoryClient::new(self.client.clone());
        let map_lock = self.mappings.clone();
        let mut mappings = map_lock.lock().await;
        match mappings.get(device) {
            Some(m) => Ok(*m),
            _ => {
                let mapping_request = tonic::Request::new(DetectMemoryMappingRequest {
                    fallback_memory_mapping: None,
                    rom_header00_ffb0: None,
                    uri: device.into()
                });

                let mapping_response = client.mapping_detect(mapping_request).await?.into_inner();
                mappings.insert(device.to_string(), mapping_response.memory_mapping);
                Ok(mapping_response.memory_mapping)
            }
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
                uri: d.uri.to_string(),
                info: None
            });
        }

        Ok(devices)
    }

    async fn read_single(&self, device: &str, address: u32, size: u32) -> Result<Vec<u8>, ConnectionError> {
        Ok(self.read_multi(device, &[address, size]).await?.remove(0))
    }

    async fn read_multi(&self, device: &str, address_info: &[u32]) -> Result<Vec<Vec<u8>>, ConnectionError> 
    {
        let mut client = device_memory_client::DeviceMemoryClient::new(self.client.clone());
        let memory_mapping = self.get_mapping(device).await?;
        let request = tonic::Request::new(MultiReadMemoryRequest {
            requests: address_info.chunks(2).map(|req| ReadMemoryRequest {
                request_address: req[0],
                request_address_space: AddressSpace::FxPakPro.into(),
                request_memory_mapping: memory_mapping,
                size: req[1]
            }).collect(),
            uri: device.into()
        });

        let mut response = client.multi_read(request).await?.into_inner();
        Ok(response.responses.drain(..).map(|r| r.data).collect())
    }

    async fn write_single(&self, device: &str, address: u32, data: &[u8])-> Result<bool, ConnectionError> {
        Ok(self.write_multi(device, &[address], &[data.to_vec()]).await?)
    }

    async fn write_multi(&self, device: &str, addresses: &[u32], data: &[Vec<u8>]) -> Result<bool, ConnectionError> {
        let mut client = device_memory_client::DeviceMemoryClient::new(self.client.clone());
        let memory_mapping = self.get_mapping(device).await?;        
        let request = tonic::Request::new(MultiWriteMemoryRequest {
            requests: addresses.iter().zip(data.iter()).map(|(address, data)| WriteMemoryRequest {
                data: data.to_vec(),
                request_address: *address,
                request_address_space: AddressSpace::FxPakPro.into(),
                request_memory_mapping: memory_mapping
            }).collect(),
            uri: device.into()        
        });

        let _ = client.multi_write(request).await?.into_inner();
        Ok(true)
    }
}