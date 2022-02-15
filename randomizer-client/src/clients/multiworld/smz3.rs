use crate::ClientContext;
use std::convert::TryInto;
use crate::services::randomizer::EventType;
use crate::services::randomizer::SessionEvent;


/* SMZ3 Game mode updates, this takes the client context so it can talk to both the backend service and some kind of console connector */

#[derive(Default)]
pub struct SMZ3Client {
    items_base: u32,
    verified_events: Vec<i32>,
}

impl SMZ3Client {
    pub fn new() -> Self {
        Self {
            items_base: 0xE04000,
            ..Default::default()
        }
    }

    pub async fn update(&mut self, ctx: &ClientContext) -> Result<(), Box<dyn std::error::Error>> {
        let svc = &ctx.randomizer_service;
        let client = &ctx.client.as_ref().ok_or("Client must be initialized and authenticated")?;
        let conn = &ctx.console_connection.as_ref().ok_or("Console connection must be initialized")?;

        // Read last written event id from console
        // The console ALWAYS controls all the data, so that in case SRAM is reset or whatever
        // we'll just fetch blank SRAM and things will be smooth
        
        // Read and verify read to make sure the input data is consistent        
        let in_ptrs = {
            let mut verified = false;
            let mut verified_in_ptrs = Vec::new();
            
            while !verified {
                let in_ptrs = conn.read_single(&ctx.device, self.items_base + 0x600, 0x10).await?;
                verified_in_ptrs = conn.read_single(&ctx.device, self.items_base + 0x600, 0x10).await?;
                if in_ptrs == verified_in_ptrs {
                    verified = true;
                } else {
                    log::debug!("smz3: Verification of read input pointers failed, trying again");
                }
            }
            verified_in_ptrs
        };

        let (_snes_read_ptr, snes_write_ptr, snes_event_id) = (
            u16::from_le_bytes(in_ptrs[0..2].try_into()?), 
            u16::from_le_bytes(in_ptrs[2..4].try_into()?), 
            i32::from_le_bytes(in_ptrs[8..12].try_into()?)
        );

        // Step 2. Request new item events since then
        let recv_events = svc.get_events(&client.client_token, 
            &[EventType::ItemFound as i32], 
            Some(snes_event_id + 1), 
            None, 
            None, 
            Some(client.world_id)).await?;

        // If there's any incoming messages, handle it and write to SNES
        if !recv_events.events.is_empty() {
            let mut recv_data: Vec<u8> = Vec::new();
            for ev in &recv_events.events {
                log::debug!("smz3: Received item event from world: {} with item: {}", ev.from_world_id, ev.item_id);
                recv_data.append(&mut u16::to_le_bytes(ev.from_world_id as u16).to_vec());
                recv_data.append(&mut u16::to_le_bytes(ev.item_id as u16).to_vec());
            }

            // Prepare full message data
            let addresses = &[self.items_base + (snes_write_ptr * 0x04) as u32, self.items_base + 0x602, self.items_base + 0x608];
            let data = &[recv_data, 
                        u16::to_le_bytes(((snes_write_ptr as usize) + recv_events.events.len()) as u16).to_vec(),
                        i32::to_le_bytes(recv_events.events.iter().map(|e| e.id).max().ok_or("Could not get max id of events")?).to_vec()];

            // Write this data to the snes, (and verify that it got written before doing anything further)
            // Any connection error will break us out of the loop as it should, but verify/rewrite will help against accidental
            // data corruption for whatever reason

            // Write the actual data first and verify that it's written
            {
                let mut verified = false;
                while !verified {
                    log::debug!("smz3: Writing item received data to SNES");
                    let _ = conn.write_single(&ctx.device, addresses[0], &data[0]).await?;
                    let verify_data = conn.read_single(&ctx.device, addresses[0], data[0].len() as u32).await?;
                    
                    if verify_data == data[0] {
                        verified = true;
                    } else {
                        log::debug!("smz3: Verification of written data of received items failed, trying again");
                    }                    
                }
            }

            // The data is ok, write the updated pointers
            // If this fails, it's fine since worst case we just wrote some data previously that'll get overwritten again            
            {
                let mut verified = false;
                while !verified {
                    log::debug!("smz3: Writing item received pointers to SNES");
                    let _ = conn.write_multi(&ctx.device, &addresses[1..], &data[1..]).await?;
                    let verify_data = conn.read_multi(&ctx.device, 
                        &[addresses[1], data[1].len() as u32,
                                    addresses[2], data[2].len() as u32]).await?;
                    
                    if verify_data == data[1..] {
                        verified = true;
                    } else {
                        log::debug!("smz3: Verification of written pointers of received items failed, trying again");
                    }   
                }
            }
            
            // Append the correct written events to the list of events to report back
            self.verified_events.append(&mut recv_events.events.iter().map(|e| e.id).collect());
        }

        // Double-read again to really make sure the data makes sense
        let out_ptrs = {
            let mut verified = false;
            let mut verified_out_ptrs = Vec::new();
            
            while !verified {
                let out_ptrs = conn.read_single(&ctx.device, self.items_base + 0x680, 0x04).await?;
                verified_out_ptrs = conn.read_single(&ctx.device, self.items_base + 0x680, 0x04).await?;
                if out_ptrs == verified_out_ptrs {
                    verified = true;
                } else {
                    log::debug!("smz3: Verification of read output pointers failed, trying again");
                }
            }
            verified_out_ptrs
        };

        // Ok, verified data, let's extract the write pointers
        let (sync_read_ptr, snes_write_ptr) = (
            u16::from_le_bytes(out_ptrs[0..2].try_into()?), 
            u16::from_le_bytes(out_ptrs[2..4].try_into()?), 
        );

        // Check if there are any new messages to send
        if sync_read_ptr < snes_write_ptr {
            let messages = snes_write_ptr - sync_read_ptr;
            let send_data = conn.read_single(&ctx.device, self.items_base + 0x700 + ((sync_read_ptr as u32) * 0x08), (messages * 0x08) as u32).await?;
            
            log::debug!("smz3: {} new messages from SNES, syncptr: {}, writeptr: {}", messages, sync_read_ptr, snes_write_ptr);

            // Report all messages back to the server before writing anything back to the snes.
            // That way if it fails, we'll just try to re-send the same things and the server will have to deal
            // with it and it'll be more failsafe on this side.

            for i in 0..messages {
                let offset = (0x08 * i) as usize;
                let (world_id, item_id, item_index) = (
                    u16::from_le_bytes(send_data[offset..offset+2].try_into()?),
                    u16::from_le_bytes(send_data[offset+2..offset+4].try_into()?),
                    u16::from_le_bytes(send_data[offset+4..offset+6].try_into()?)
                );

                log::debug!("smz3: Sending item {} at location {} from world {} to world {}", item_id, item_index, &client.world_id, world_id);
                let _ = svc.send_event(&client.client_token, SessionEvent {
                        id: 0,
                        event_type: EventType::ItemFound as i32,
                        from_world_id: client.world_id,
                        item_id: item_id as i32,
                        item_location: item_index as i32,
                        sequence_num: (sync_read_ptr + i) as i32,
                        to_world_id: world_id as i32,
                        confirmed: false,
                        message: format!("Sent item {} at location {} from world {} to world {}", item_id, item_index, &client.world_id, world_id),
                        time_stamp: "".into()
                    }).await?;
            }

            // If we get here, all the events were correctly sent to the server and we can write the confirmation to the SNES
            let mut verified = false;
            let new_sync_read_ptr = sync_read_ptr + messages;
            while !verified {
                log::debug!("smz3: Updating outgoing message pointer on the SNES to: {}", new_sync_read_ptr);
                let _ = conn.write_single(&ctx.device, self.items_base + 0x680, &u16::to_le_bytes(new_sync_read_ptr)).await?;
                let verify_data = conn.read_single(&ctx.device, self.items_base + 0x680, 0x02).await?;
                let verify_value = u16::from_le_bytes(verify_data[..2].try_into()?);
                if verify_value == new_sync_read_ptr {
                    verified = true;
                } else {
                    log::debug!("smz3: Verification of written data of sent items failed, trying again");
                }
            }            
        }

        // Send item confirmation, at this point it doesn't matter too much if we error out
        if !self.verified_events.is_empty() {
            log::debug!("smz3: Reporting {} incoming item events as confirmed", self.verified_events.len());
            let _ = svc.confirm_events(&client.client_token, &self.verified_events).await?;
            self.verified_events = Vec::new();
        }

        // All done
        Ok(())
    }
}