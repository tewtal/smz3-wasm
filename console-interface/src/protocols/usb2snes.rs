use ws_stream_wasm::*;
use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use futures::{stream::StreamExt, SinkExt};
use std::{sync::Arc};
use futures::lock::Mutex;
use crate::protocols::protocol::{Device, Connection, ConnectionError};

#[allow(non_snake_case)]
#[derive(Debug, Serialize)]
struct SnesRequest {
    pub Opcode: String,
    pub Space: String,
    pub Flags: Option<Vec<String>>,
    pub Operands: Option<Vec<String>>
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct SnesResponse {
    pub Results: Vec<String>
}

#[derive(Clone)]
enum ConnectionState {
    Disconnected,
    Connected,
    Attached
}

#[allow(dead_code, non_camel_case_types)]
#[derive(Debug)]
pub enum FileType {
    Directory = 0,
    File = 1
}

#[allow(dead_code)]
#[derive(Debug)]
enum Command {
    DeviceList,
    Attach(String),
    AppVersion,
    Name(String),
    Close,
    Info,
    Boot(String),
    Menu,
    Reset,
    Binary,
    Stream,
    Fence,
    GetAddress(Vec<String>),
    PutAddress(Vec<String>, Vec<u8>),
    PutIPS,
    GetFile(String),
    PutFile(Vec<String>, Vec<u8>),
    List(String),
    Remove(String),
    Rename(Vec<String>),
    MakeDir(String)
}

#[derive(Debug)]
enum CommandResponseType {
    Text,
    Binary(usize),
    None
}

#[derive(Debug)]
enum CommandResponse {
    Response(SnesResponse),
    Data(Vec<u8>),
    Empty
}

pub struct Socket {
    ws: Option<WsMeta>,
    wsio: Option<WsStream>,
    state: ConnectionState,
    device: String
}

pub struct Usb2SnesConnection {
    uri: String,
    socket: Arc<Mutex<Socket>>,
}

impl Usb2SnesConnection {
    pub fn new(uri: &str) -> Self {
        Self {
            uri: uri.to_string(),
            socket: Arc::new(Mutex::new(Socket { ws: None, wsio: None, state: ConnectionState::Disconnected, device: String::new() })),
        }
    }

    // Handle attach as a special case so we don't end up with attach calling itself over and over
    async fn attach(&self, device: &str) -> Result<bool, ConnectionError> {
        let sock_l = self.socket.clone();
        let mut sock = sock_l.lock().await;
        let wsio = sock.wsio.as_mut().ok_or(ConnectionError("Could not get websocket".into()))?;

        let _ = wsio.send(WsMessage::Text(
            serde_json::to_string(&SnesRequest { Opcode: "Attach".into(), Space: "SNES".into(), Flags: None, Operands: Some(vec![device.to_string()]) })
            .map_err(|_| ConnectionError("Could not send attach request".into()))?
        )).await.map_err(|_| ConnectionError("Could not send attach request".into()))?;
        
        sock.state = ConnectionState::Attached;
        sock.device = device.to_string();
        log::debug!("usb2snes: Attached to device: {}", &sock.device);
        Ok(true)
    }
    
    async fn update_connection_state(&self, device: Option<&str>) -> Result<bool, ConnectionError> {
        let conn = Box::new(self as &dyn Connection);
        let sock = self.socket.clone();
        
        /* Capture socket state in an inner scope so the lock is not held */
        let (state, current_device) = {
            let mut sock = sock.lock().await;

            /* Check if we're still connected */
            if let Some(ws) = sock.ws.as_ref() {
                if ws.ready_state() != WsState::Open {
                    sock.ws = None;
                    sock.wsio = None;
                    sock.state = ConnectionState::Disconnected;
                    sock.device = String::new();
                    log::info!("usb2snes: WebSocket disconnected, retrying connection");
                }
            }
            
            (sock.state.clone(), sock.device.clone())
        };        
        
        /* Handle required connection state updates */
        match state {
            ConnectionState::Disconnected => Ok(conn.connect().await?),
            ConnectionState::Connected => 
                match device {
                    Some(d) => Ok(self.attach(d).await?),
                    _ => Ok(true)
            },
            ConnectionState::Attached => {
                match device {
                    Some(d) if d != current_device => Ok(self.attach(d).await?),
                    _ => Ok(true)
                }
            }
        }
    }

    fn get_size(&self, addrs: &[String]) -> Result<usize, ConnectionError>  {
        addrs
        .iter()
        .skip(1)
        .step_by(2)
        .fold(Ok(0), |acc, size| {
            match acc {
                Ok(acc) => Ok(usize::from_str_radix(size, 16).map_err(|_| ConnectionError("Could not get data size for request".into()))? + acc),
                Err(e) => Err(e)
            }
        })
    }

    async fn send_command(&self, device: Option<&str>, command: Command) -> Result<CommandResponse, ConnectionError> {        
        self.update_connection_state(device).await?;        
        log::debug!("usb2snes: Sending command: {:?}", &command);
        let sock_l = self.socket.clone();
        let mut sock = sock_l.lock().await;
        let wsio = sock.wsio.as_mut().ok_or(ConnectionError("Could not get websocket".into()))?;
        
        let (opcode, operands, flags, space, response_type) = match &command {
            Command::DeviceList =>                  ("DeviceList", None, None, "SNES", CommandResponseType::Text),
            Command::Info =>                        ("Info", None, None, "SNES", CommandResponseType::Text),
            Command::AppVersion =>                  ("AppVersion", None, None, "SNES", CommandResponseType::Text),
            Command::PutAddress(addrs, _) =>        ("PutAddress", Some(addrs), None, "SNES", CommandResponseType::None),
            Command::GetAddress(addrs) =>           { let size = self.get_size(&addrs)?; ("GetAddress", Some(addrs), None, "SNES", CommandResponseType::Binary(size)) },
            _ => return Err(ConnectionError(format!("Attempted to use unsupported command: {:?}", &command).into()))
        };

        let _ = wsio.send(WsMessage::Text(
            serde_json::to_string(
                &SnesRequest {
                    Opcode: opcode.into(),
                    Space: space.into(),
                    Flags: flags,
                    Operands: operands.cloned()
                }
            ).map_err(|_| ConnectionError("Could not send device command".into()))?
        )).await.map_err(|_| ConnectionError("Could not send device command".into()))?;

        let _ = wsio.flush().await.map_err(|_| ConnectionError("Could not flush data".into()))?;

        // Read the response if needed
        let response = match response_type {
            CommandResponseType::None => CommandResponse::Empty,
            CommandResponseType::Text => {
                let response = wsio.next().await.ok_or(ConnectionError("Could not read response data".into()))?;
                match response {
                    WsMessage::Text(t) => CommandResponse::Response(serde_json::from_str(&t).map_err(|_| ConnectionError("Could not read command response".into()))?),
                    _ => return Err(ConnectionError("Got binary response when expecting a text response".into()))
                }
            },
            CommandResponseType::Binary(size) => {
                log::debug!("usb2snes: Reading binary data with size: {:X}", size);
                let mut resp_data: Vec<u8> = Vec::new();
                while resp_data.len() < size {
                    let response = wsio.next().await.ok_or(ConnectionError("Error while reading binary response".into()))?;    
                    match response {
                        WsMessage::Binary(mut d) => resp_data.append(&mut d),
                        _ => return Err(ConnectionError("Got text data when expecting binary data".into()))
                    }
                }
                CommandResponse::Data(resp_data)
            }
        };

        // Send any binary data that might be included in a command
        match command {
            Command::PutAddress(_, d) | Command::PutFile(_, d) => {
                wsio.send(WsMessage::Binary(d)).await.map_err(|_| ConnectionError("Could not send binary data".into()))?;
                let _ = wsio.flush().await.map_err(|_| ConnectionError("Could not flush data".into()))?;           
            },
            _ => ()
        };

        Ok(response)
    }
}

#[async_trait(?Send)]
impl Connection for Usb2SnesConnection {
    
    async fn connect(&self) -> Result<bool, ConnectionError> {
        let (ws, wsio) = WsMeta::connect(&self.uri, None).await.map_err(|_| ConnectionError("Could not connect to websocket".into()))?;
        {
            let sock_l = self.socket.clone();
            let mut sock = sock_l.lock().await;
            sock.ws = Some(ws);
            sock.wsio = Some(wsio);
            sock.state = ConnectionState::Connected;
            log::info!("usb2snes: Connected to {}", &self.uri);
        }

        Ok(true)
    }

    async fn disconnect(&self) -> Result<bool, ConnectionError> {
        let sock_l = self.socket.clone();
        let mut sock = sock_l.lock().await;
        if let Some(ws) = sock.ws.as_ref() {
            ws.close().await.map_err(|_| ConnectionError("Could not close websocket".into()))?;
            sock.ws = None;
            sock.wsio = None;
            sock.state = ConnectionState::Disconnected;
            sock.device = String::new();
        }
        Ok(true)
    }

    // Get the device list from the server
    // For each device also attach and issue an info request
    async fn list_devices(&self) -> Result<Vec<Device>, ConnectionError>
    {
        match self.send_command(None, Command::DeviceList).await? {
            CommandResponse::Response(r) => {
                let mut devices = Vec::new();
                for device in &r.Results {
                    self.attach(device).await?;
                    devices.push(match self.send_command(Some(device), Command::Info).await? {
                        CommandResponse::Response(i) => {
                            Device {
                                name: device.to_string(),
                                uri: device.to_string(),
                                info: Some(i.Results)
                            }
                        },
                        _ => return Err(ConnectionError("Unexpected Info response".into()))
                    });                    
                }

                Ok(devices)
            },
            _ => Err(ConnectionError("Unexpected DeviceList response".into()))
        }
    }

    async fn read_single(&self, device: &str, address: u32, size: u32) -> Result<Vec<u8>, ConnectionError> {
        Ok(self.read_multi(device, &[address, size]).await?.remove(0))
    }
    
    // Issue a vectored read, translated to a VGET if possible
    // The usb2snes protocol doesn't officially support VGET requests larger than 255 bytes, so if the sum
    // of bytes requested is larger than 255 bytes, split each VGET request into single GET requests.
    async fn read_multi(&self, device: &str, address_info: &[u32]) -> Result<Vec<Vec<u8>>, ConnectionError> 
    {
        match address_info.iter().skip(1).step_by(2).map(|s| *s as usize).sum::<usize>() {
            req_size if address_info.len() > 2 && address_info.len() <= 16 && req_size < 256 => {
                match self.send_command(Some(device), Command::GetAddress(address_info.iter().map(|a| format!("{:X}", a)).collect())).await? {
                    CommandResponse::Data(response) => {
                        let mut position: usize = 0;
                        let mut data = Vec::new();
                        
                        for size in address_info.iter().skip(1).step_by(2).map(|s| *s as usize) {
                            data.push(response[position..position+size].to_vec());
                            position += size;
                        }
        
                        Ok(data)
                    },
                    _ => Err(ConnectionError("Unexpected ReadMemory response".into()))
                }
            },
            _ => {
                let mut data = Vec::new();
                for addr_chunk in address_info.chunks(2) {
                    match self.send_command(Some(device), Command::GetAddress(addr_chunk.iter().map(|a| format!("{:X}", a)).collect())).await? {
                        CommandResponse::Data(response) => data.push(response),
                        _ => return Err(ConnectionError("Unexpected ReadMemory response".into()))
                    }
                };
                Ok(data)
            }
        }
    }
   
    async fn write_single(&self, device: &str, address: u32, data: &[u8])-> Result<(), ConnectionError> {
        Ok(self.write_multi(device, &[address], &[data.to_vec()]).await?)
    }
    
    // Issue a vectored write, translated to a VPUT if possible
    // The usb2snes protocol doesn't officially support VPUT requests larger than 255 bytes, so if the sum
    // of bytes to send is larger than 255 bytes, split each VPUT request into single PUT requests.
    async fn write_multi(&self, device: &str, addresses: &[u32], data: &[Vec<u8>]) -> Result<(), ConnectionError> {
        match data.iter().map(|d| d.len()).sum::<usize>() {
            req_size if addresses.len() >= 2 && addresses.len() <= 8 && req_size < 256 => {
                let address_info = addresses.iter().zip(data.iter().map(|d| d.len() as u32)).flat_map(|(a, s)| vec![format!("{:X}", a), format!("{:X}", s)]).collect();
                match self.send_command(Some(device), Command::PutAddress(address_info, data.iter().flat_map(|d| d.clone()).collect::<Vec<u8>>())).await? {
                    CommandResponse::Empty => (),
                    _ => return Err(ConnectionError("Unexpected PutAddress response".into()))
                }
            },
            _ => {
                for (address, data) in addresses.iter().zip(data.iter()) {
                    match self.send_command(Some(device), Command::PutAddress(vec![format!("{:X}", address), format!("{:X}", data.len())], data.to_vec())).await? {
                        CommandResponse::Empty => (),
                        _ => return Err(ConnectionError("Unexpected PutAddress response".into()))
                    }
                }
            }
        };

        // Silly workaround for the WASM websocket library that can't detect if we're disconnected without reading.
        // So let's send an AppVersion command to force the issue and detect if we're still connected.
        // If we're disconnected it'll trigger an error so the caller can know that the transfer most likely did not go through.
        let _ = self.send_command(None, Command::AppVersion).await?;
        Ok(())
    }
}