use crate::ClientContext;


/* SMZ3 Game mode updates, this takes the client context so it can talk to both the backend service and some kind of console connector */

#[derive(Default)]
pub struct SMZ3Client {
    _snes_out_ptr: u16,
    _out_ptr: u16,
    _message_base: u32,
    _items_base: u32
}

impl SMZ3Client {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub async fn update(&mut self, ctx: &ClientContext) -> Result<(), Box<dyn std::error::Error>> {
        let svc = &ctx.randomizer_service;
        let client = &ctx.client.as_ref().ok_or("Client must be initialized and authenticated")?;
        let conn = &ctx.console_connection.as_ref().ok_or("Console connection must be initialized")?;

        let _events = svc.get_events(&client.client_token, &[0,1,2], None, None, None, None).await?;
        let _memory = conn.read_multi(&ctx.device, &[0, 0x40, 0x200, 0x20]).await?;

        Ok(())
    }
}
