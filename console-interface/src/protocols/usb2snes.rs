use ws_stream_wasm::*;
use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use futures::{stream::StreamExt, SinkExt};
use std::{sync::Arc, num::ParseIntError};
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
    PutAddress(Vec<String>),
    PutIPS,
    GetFile(String),
    PutFile(Vec<String>),
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
        let wsio = sock.wsio.as_mut().ok_or("Could not get websocket")?;

        let _ = wsio.send(WsMessage::Text(
            serde_json::to_string(&SnesRequest { Opcode: "Attach".into(), Space: "SNES".into(), Flags: None, Operands: Some(vec![device.to_string()]) })?
        )).await.map_err(|_| "Could not send attach request")?;
        
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
                    log::debug!("usb2snes: WebSocket disconnected, retrying connection");
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

    fn get_size(&self, addrs: &[String]) -> Result<usize, ParseIntError>  {
        addrs
        .iter()
        .skip(1)
        .step_by(2)
        .fold(Ok(0), |acc, size| {
            match acc {
                Ok(acc) => Ok(usize::from_str_radix(size, 16)? + acc),
                Err(e) => Err(e)
            }
        })
    }

    async fn send_command(&self, device: Option<&str>, command: Command) -> Result<CommandResponse, ConnectionError> {        
        self.update_connection_state(device).await?;        
        log::debug!("usb2snes: Sending command: {:?}", &command);
        let sock_l = self.socket.clone();
        let mut sock = sock_l.lock().await;
        let wsio = sock.wsio.as_mut().ok_or("Could not get websocket")?;
        
        let (opcode, operands, flags, space, response_type) = match command {
            Command::DeviceList =>          ("DeviceList", None, None, "SNES", CommandResponseType::Text),
            Command::Info =>                ("Info", None, None, "SNES", CommandResponseType::Text),
            Command::PutAddress(addrs) =>   ("PutAddress", Some(addrs), Some(vec!["NORESP".to_string()]), "SNES", CommandResponseType::None),
            Command::GetAddress(addrs) =>   { let size = self.get_size(&addrs)?; ("GetAddress", Some(addrs), Some(vec!["NORESP".to_string()]), "SNES", CommandResponseType::Binary(size)) },
            _ => return Err(format!("Attempted to use unsupported command: {:?}", &command).into())
        };

        let _ = wsio.send(WsMessage::Text(
            serde_json::to_string(
                &SnesRequest {
                    Opcode: opcode.into(),
                    Space: space.into(),
                    Flags: flags,
                    Operands: operands
                }
            )?
        )).await?;

        let _ = wsio.flush().await.map_err(|_| "Could not flush data")?;

        Ok(match response_type {
            CommandResponseType::None => CommandResponse::Empty,
            CommandResponseType::Text => {
                let response = wsio.next().await.ok_or("Could not read response data")?;
                match response {
                    WsMessage::Text(t) => CommandResponse::Response(serde_json::from_str(&t)?),
                    _ => return Err("Got binary response when expecting a text response".into())
                }
            },
            CommandResponseType::Binary(size) => {
                log::debug!("usb2snes: Reading binary data with size: {:X}", size);
                let mut data: Vec<u8> = Vec::new();
                while data.len() < size {
                    let response = wsio.next().await.ok_or("Error while reading binary response")?;    
                    match response {
                        WsMessage::Binary(mut d) => data.append(&mut d),
                        _ => return Err("Got text data when expecting binary data".into())
                    }
                }
                CommandResponse::Data(data)
            }
       })       
    }
}

#[async_trait(?Send)]
impl Connection for Usb2SnesConnection {
    
    async fn connect(&self) -> Result<bool, ConnectionError> {
        let (ws, wsio) = WsMeta::connect(&self.uri, None).await?;
        {
            let sock_l = self.socket.clone();
            let mut sock = sock_l.lock().await;
            sock.ws = Some(ws);
            sock.wsio = Some(wsio);
            sock.state = ConnectionState::Connected;
            log::debug!("usb2snes: Connected to {}", &self.uri);
        }

        Ok(true)
    }

    async fn disconnect(&self) -> Result<bool, ConnectionError> {
        let sock_l = self.socket.clone();
        let mut sock = sock_l.lock().await;
        if let Some(ws) = sock.ws.as_ref() {
            ws.close().await?;
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
                        _ => return Err("Unexpected Info response".into())
                    });                    
                }

                Ok(devices)
            },
            _ => Err("Unexpected DeviceList response".into())
        }
    }

    async fn read_single(&self, device: &str, address: u32, size: u32) -> Result<Vec<u8>, ConnectionError> {
        Ok(self.read_multi(device, &[address, size]).await?.remove(0))
    }
    
    // Issue a vectored read, translated to a VGET if possible
    // The usb2snes protocol doesn't officially support VGET requests larger than 255 bytes, so if the sum
    // of bytes requested is larger than 255 bytes, split each VGET request into a single GET.
    async fn read_multi(&self, device: &str, address_info: &[u32]) -> Result<Vec<Vec<u8>>, ConnectionError> 
    {
        match address_info.iter().skip(1).step_by(2).map(|s| *s as usize).sum::<usize>() {
            req_size if address_info.len() > 2 && req_size < 256 => {
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
                    _ => Err("Unexpected ReadMemmory response".into())
                }
            },
            _ => {
                let mut data = Vec::new();
                for addr_chunk in address_info.chunks(2) {
                    match self.send_command(Some(device), Command::GetAddress(addr_chunk.iter().map(|a| format!("{:X}", a)).collect())).await? {
                        CommandResponse::Data(response) => data.push(response),
                        _ => return Err("Unexpected ReadMemmory response".into())
                    }
                };
                Ok(data)
            }
        }
   }
}