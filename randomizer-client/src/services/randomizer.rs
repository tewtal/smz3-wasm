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

    pub async fn unregister_player(&self, client_token: &str) -> Result<UnregisterPlayerResponse, tonic::Status> {
        let mut client = session_client::SessionClient::new(self.client.clone());

        let request = tonic::Request::new(UnregisterPlayerRequest {
            client_token: client_token.to_string(),
            sram_backup: None
        });

        let response = client.unregister_player(request).await?.into_inner();
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

    pub async fn _get_spoiler(&self, client_token: &str) -> Result<GetSpoilerResponse, tonic::Status> {
        let mut client = metadata_client::MetadataClient::new(self.client.clone());

        let request = tonic::Request::new(GetSpoilerRequest {
            client_token: client_token.to_string()
        });

        let response = client.get_spoiler(request).await?.into_inner();
        Ok(response)
    }

    pub async fn get_events(&self, client_token: &str, event_types: &[i32], 
                                   from_event_id: Option<i32>, to_event_id: Option<i32>,
                                   from_world_id: Option<i32>, to_world_id: Option<i32>) -> Result<GetEventsResponse, tonic::Status> 
    {        
        let mut client = event_client::EventClient::new(self.client.clone());
        let request = tonic::Request::new(GetEventsRequest {
            client_token: client_token.to_string(),
            from_event_id,
            to_event_id,
            from_world_id,
            to_world_id,
            event_types: event_types.to_vec()
        });

        let response = client.get_events(request).await?.into_inner();
        Ok(response)
    }

    pub async fn get_report(&self, client_token: &str, seed_id: i32, from_event_id: i32, world_id: i32, event_types: &[i32]) -> Result<GetReportResponse, tonic::Status>
    {
        let mut client = event_client::EventClient::new(self.client.clone());
        let request = tonic::Request::new(GetReportRequest {
            client_token: client_token.to_string(),
            seed_id,
            from_event_id,
            world_id,
            event_types: event_types.to_vec()
        });

        let response = client.get_report(request).await?.into_inner();
        Ok(response)        
    }

    pub async fn send_event(&self, client_token: &str, session_event: SessionEvent) -> Result<SendEventResponse, tonic::Status> {
        let mut client = event_client::EventClient::new(self.client.clone());
        
        let request = tonic::Request::new(SendEventRequest {
            client_token: client_token.to_string(),
            event: Some(session_event)
        });

        let response = client.send_event(request).await?.into_inner();
        Ok(response)
    }

    pub async fn confirm_events(&self, client_token: &str, events_ids: &[i32])-> Result<ConfirmEventsResponse, tonic::Status> {
        let mut client = event_client::EventClient::new(self.client.clone());

        let request = tonic::Request::new(ConfirmEventsRequest {
            client_token: client_token.to_string(),
            event_ids: events_ids.to_vec()
        });

        let response = client.confirm_events(request).await?.into_inner();
        Ok(response)
    }
}


