use ferrumc_macros::event_handler;
use ferrumc_net::connection::ConnectionState;
use ferrumc_net::connection::PacketWriter;
use ferrumc_net::errors::NetError;
use ferrumc_net::packets::outgoing::update_time::TickEvent;
use ferrumc_net::packets::outgoing::update_time::UpdateTimePacket;
use ferrumc_net::utils::broadcast::{BroadcastOptions, BroadcastToAll};
use ferrumc_state::GlobalState;
use tracing::warn;

#[event_handler]
async fn handle_tick(event: TickEvent, state: GlobalState) -> Result<TickEvent, NetError> {
    // info!("Tick {} ", event.tick);
    // TODO: Handle tick in terms of game logic here
    // this should call a function in world which handles the world state and calls the appropriate events which send their respective packets

    ///////

    let packet = UpdateTimePacket::new(event.tick, event.tick % 24000);

    let entities = state
        .universe
        .query::<(&mut PacketWriter, &ConnectionState)>()
        .into_entities()
        .into_iter()
        .filter_map(|entity| {
            let conn_state = state.universe.get::<ConnectionState>(entity).ok()?;
            if matches!(*conn_state, ConnectionState::Play) {
                Some(entity)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    tokio::spawn(async move {
        if let Err(e) = state
            .broadcast(&packet, BroadcastOptions::default().only(entities))
            .await
        {
            warn!("Failed to broadcast tick packet: {:?}", e);
        }
    });

    Ok(event)
}
