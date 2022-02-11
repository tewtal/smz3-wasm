use ws_stream_wasm::*;
use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use futures::{stream::StreamExt, SinkExt};
use std::sync::Arc;
use futures::lock::Mutex;
use crate::protocols::protocol::{Device, Connection, ConnectionError};

#[allow(non_snake_case)]
#[derive(Serialize)]
struct SnesRequest {
    pub Opcode: String,
    pub Space: String,
    pub Flags: Option<Vec<String>>,
    pub Operands: Option<Vec<String>>
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
struct SnesResponse {
    pub Results: Vec<String>
}

#[derive(Clone)]
enum ConnectionState {
    Disconnected,
    Connected,
    Attached
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

    async fn attach(&self, device: &str) -> Result<bool, ConnectionError> {
        let sock_l = self.socket.clone();
        let mut sock = sock_l.lock().await;
        let wsio = sock.wsio.as_mut().ok_or("Could not get websocket")?;

        let _ = wsio.send(WsMessage::Text(
            serde_json::to_string(
                &SnesRequest {
                    Opcode: "Attach".into(),
                    Space: "SNES".into(),
                    Flags: None,
                    Operands: Some(vec![device.to_string()])
                }
            )?
        )).await.map_err(|_| "Could not send attach request")?;
        
        sock.state = ConnectionState::Attached;
        sock.device = device.to_string();
        Ok(true)
    }
    
    async fn attach_or_reconnect(&self, device: Option<&str>) -> Result<bool, ConnectionError> {
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
}

#[async_trait(?Send)]
impl Connection for Usb2SnesConnection {
    
    async fn connect(&self) -> Result<bool, ConnectionError>
    {
        let (ws, wsio) = WsMeta::connect(&self.uri, None).await?;
        {
            let sock_l = self.socket.clone();
            let mut sock = sock_l.lock().await;
            sock.ws = Some(ws);
            sock.wsio = Some(wsio);
            sock.state = ConnectionState::Connected;
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

    async fn list_devices(&self) -> Result<Vec<Device>, ConnectionError>
    {
        self.attach_or_reconnect(None).await?;
        let sock_l = self.socket.clone();
        let response = {
            let mut sock = sock_l.lock().await;
            let wsio = sock.wsio.as_mut().ok_or("Could not get websocket")?;
            
            let _ = wsio.send(WsMessage::Text(
                serde_json::to_string(
                    &SnesRequest {
                        Opcode: "DeviceList".into(),
                        Space: "SNES".into(),
                        Flags: None,
                        Operands: None
                    }
                )?
            )).await?;

            let _ = wsio.flush().await?;
            wsio.next().await
        };
        
        match response {
            Some(WsMessage::Text(t)) => {
                let snes_response: SnesResponse = serde_json::from_str(&t)?;
                let devices: Vec<Device> = snes_response.Results
                    .iter()
                    .map(|r| Device {
                        name: r.into(),
                        uri: r.into()
                    })
                    .collect();

                Ok(devices)
            },
            _ => Err("Invalid response".into())
        }
    }

    async fn read_memory(&self, device: &str, address: u32, size: u32) -> Result<Vec<u8>, ConnectionError> 
    {
        self.attach_or_reconnect(Some(device)).await?;
        let sock_l = self.socket.clone();
        let response = {
            let mut sock = sock_l.lock().await;
            let wsio = sock.wsio.as_mut().ok_or("Could not get websocket")?;

            let _ = wsio.send(WsMessage::Text(
                serde_json::to_string(
                    &SnesRequest {
                        Opcode: "GetAddress".into(),
                        Space: "SNES".into(),
                        Flags: None,
                        Operands: Some(vec![format!("{:X}", address), format!("{:X}", size)])
                    }
                )?
            )).await.map_err(|_| "Could not send getaddress request")?;

            let _ = wsio.flush().await.map_err(|_| "Could not flush data")?;        

            let mut data: Vec<u8> = Vec::new();

            while data.len() < size as usize {
                let response = wsio.next().await.ok_or("Error while reading binary response")?;

                match response {
                    WsMessage::Binary(mut d) => data.append(&mut d),
                    _ => return Err("Got text data when expecting binary data".into())
                }
            }

            data
        };

        Ok(response)
   }
}