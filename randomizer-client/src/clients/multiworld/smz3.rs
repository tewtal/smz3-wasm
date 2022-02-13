use crate::ClientContext;

/* SMZ3 Game mode updates, this takes the client context so it can talk to both the backend service and some kind of console connector */

pub async fn smz3(ctx: &ClientContext) {
    match (&ctx.console_connection, &ctx.client, &ctx.randomizer_service) {
        (Some(conn), Some(client), svc) => {
            log::debug!("Read: {:?}", conn.read_single(&ctx.device, 0, 0x100).await);
            log::debug!("Patch: {:?}", svc.get_patch(&client.client_token).await);
        },
        _ => ()
    }
}