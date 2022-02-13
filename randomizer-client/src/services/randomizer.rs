tonic::include_proto!("randomizer");
use serde::Serialize;

pub struct RandomizerService {
    client: grpc_web_client::Client
}

impl RandomizerService {
    pub fn new(uri: &str) -> Self {
        Self {
            client: grpc_web_client::Client::new(uri.to_string())
        }
    }

    pub async fn get_session(&self, session_guid: &str) -> Result<GetSessionResponse, tonic::Status> {
        let mut client = session_client::SessionClient::new(self.client.clone());
    
        let request = tonic::Request::new(GetSessionRequest {
            session_guid: session_guid.to_string()
        });
    
        let response = client.get_session(request).await?.into_inner();
        Ok(response)
    }

    pub async fn register_player(&self, session_guid: &str, world_id: i32) -> Result<RegisterPlayerResponse, tonic::Status> {
        let mut client = session_client::SessionClient::new(self.client.clone());

        let request = tonic::Request::new(RegisterPlayerRequest {
            session_guid: session_guid.to_string(),
            world_id
        });

        let response = client.register_player(request).await?.into_inner();
        Ok(response)
    }
    
    pub async fn login_player(&self, session_guid: &str, client_guid: &str) -> Result<RegisterPlayerResponse, tonic::Status> {
        let mut client = session_client::SessionClient::new(self.client.clone());

        let request = tonic::Request::new(LoginPlayerRequest {
            session_guid: session_guid.to_string(),
            client_guid: client_guid.to_string()
        });

        let response = client.login_player(request).await?.into_inner();
        Ok(response)
    }

    pub async fn get_patch(&self, client_token: &str) -> Result<GetPatchResponse, tonic::Status> {
        let mut client = metadata_client::MetadataClient::new(self.client.clone());
        
        let request = tonic::Request::new(GetPatchRequest {
            client_token: client_token.to_string()
        });

        let response = client.get_patch(request).await?.into_inner();
        Ok(response)
    }
}


