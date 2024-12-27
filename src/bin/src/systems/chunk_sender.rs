use crate::systems::definition::System;
use async_trait::async_trait;
use ferrumc_core::identity::player_identity::PlayerIdentity;
use ferrumc_core::transform::position::Position;
use ferrumc_net::connection::StreamWriter;
use ferrumc_net::packets::outgoing::chunk_and_light_data::ChunkAndLightData;
use ferrumc_net::packets::outgoing::set_center_chunk::SetCenterChunk;
use ferrumc_net::utils::ecs_helpers::EntityExt;
use ferrumc_net_codec::encode::NetEncodeOpts;
use ferrumc_state::GlobalState;
use std::ops::Div;
use std::simd::num::SimdFloat;
use std::simd::{f64x2, StdFloat};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info};
use ferrumc_ecs::components::storage::ComponentRef;
use ferrumc_net::utils::entity_exts::packets::SendPacketToConnExt;

const CHUNK_RADIUS: i32 = 4;

pub(super) struct ChunkSenderSystem {
    pub stop: AtomicBool,
}

impl ChunkSenderSystem {
    pub const fn new() -> Self {
        Self {
            stop: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl System for ChunkSenderSystem {
    async fn start(self: Arc<Self>, state: GlobalState) {
        info!("Chunk sender system started");

        while !self.stop.load(Ordering::Relaxed) {
            debug!("Sending chunks to players");

            // Get all entities with a player identity and position (Valid receivers)
            // We're doing `.entities()` here because we don't want to lock any component
            // Since chunk fetching takes a while.
            let players = (&state)
                .universe
                .query::<(&PlayerIdentity, &Position)>()
                .into_entities();

            debug!("Sending to {} players", players.len());

            let player_futures = players.into_iter().filter_map(|entity| {
                let Ok(chunk_position) = entity.get::<Position>(&state).map(pos_into_chunk_pos).ok()?;
                debug!("Sending to player at {:?}", chunk_position);

                entity.send_packet(&state, &SetCenterChunk::new(chunk_position.0, chunk_position.1)).await.ok()?;

                let start = std::time::Instant::now();
                let mut chunk_range = (chunk_position.0 - CHUNK_RADIUS..chunk_position.0 + CHUNK_RADIUS)
                    .flat_map(|z| {
                        (chunk_position.1 - CHUNK_RADIUS..chunk_position.1 + CHUNK_RADIUS)
                            .map(move |x| (x, z, "overworld"))
                    })
                    .collect::<Vec<_>>();

                chunk_range.sort_by_key(|&(x, z, _)| {
                    let dx = x - chunk_position.0;
                    let dz = z - chunk_position.1;
                    (((dx ^ 2) + (dz ^ 2)) as f64).sqrt() as i32
                });

                let Ok(chunk_futures) = state.world.load_chunk_batch(chunk_range).await.map(|chunk| {
                    chunk.iter()
                        .map(|c| ChunkAndLightData::from_chunk(c)
                            .unwrap_or(ChunkAndLightData::empty(c.x, c.z)))
                        .map(|packet| entity.send_packet_owned(&state, packet))
                        .collect::<Vec<_>>()
                }) else {
                    return None;
                };

                debug!(
                    "Sent {} chunks to player at {:?} in {:?}",
                    (CHUNK_RADIUS * 2) * 2,
                    chunk_position,
                    start.elapsed()
                );

                // Wait for all the futures to complete
                Some(futures::future::join_all(chunk_futures))
            }).collect::<Vec<_>();

            tokio::time::sleep(Duration::from_secs(5)).await;
        }

        /*while !self.stop.load(Ordering::Relaxed) {
            debug!("Sending chunks to players");
            let players = state
                .universe
                .query::<(&PlayerIdentity, &Position, &mut StreamWriter)>();
            // TODO: This is so ass. Please fix this.
            for (_entity, (player, position, mut conn)) in players {
                debug!(
                    "Sending chunks to player: {} @ {}",
                    player.username, position
                );
                // Haha SIMD go brrrrt
                let [chunk_x, chunk_z] = f64x2::from_array([position.x, position.z])
                    .floor()
                    .div(f64x2::from_array([16f64, 16f64]))
                    .cast::<i32>()
                    .to_array();
                if let Err(e) = conn
                    .send_packet(
                        &SetCenterChunk::new(chunk_x, chunk_z),
                        &NetEncodeOpts::WithLength,
                    )
                    .await
                {
                    error!(
                        "Unable to set the center chunk for {} @ {}, {}: {}",
                        &player.username, chunk_x, chunk_z, e
                    );
                    continue;
                }
                let start = std::time::Instant::now();
                let mut chunk_range = (chunk_x - CHUNK_RADIUS..chunk_x + CHUNK_RADIUS)
                    .flat_map(|z| {
                        (chunk_z - CHUNK_RADIUS..chunk_z + CHUNK_RADIUS)
                            .map(move |x| (x, z, "overworld"))
                    })
                    .collect::<Vec<_>>();

                chunk_range.sort_by_key(|&(x, z, _)| {
                    let dx = x - chunk_x;
                    let dz = z - chunk_z;
                    (((dx ^ 2) + (dz ^ 2)) as f64).sqrt() as i32
                });

                match state.world.load_chunk_batch(chunk_range).await {
                    Ok(chunks) => {
                        for chunk in chunks {
                            match ChunkAndLightData::from_chunk(&chunk) {
                                Ok(data) => {
                                    if let Err(e) =
                                        conn.send_packet(&data, &NetEncodeOpts::WithLength).await
                                    {
                                        error!(
                                            "Unable to send chunk data to {} @ {}, {}: {}",
                                            &player.username, chunk.x, chunk.z, e
                                        );
                                        if let Err(e) = conn
                                            .send_packet(
                                                &ChunkAndLightData::empty(chunk.x, chunk.z),
                                                &NetEncodeOpts::WithLength,
                                            )
                                            .await
                                        {
                                            error!(
                                                "Unable to send empty chunk data to {} @ {}, {}: {}",
                                                &player.username, chunk.x, chunk.z, e
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!(
                                        "Unable to convert chunk to chunk and light data for {} @ {}, {}: {}",
                                        &player.username, chunk.x, chunk.z, e
                                    );
                                    if let Err(e) = conn
                                        .send_packet(
                                            &ChunkAndLightData::empty(chunk.x, chunk.z),
                                            &NetEncodeOpts::WithLength,
                                        )
                                        .await
                                    {
                                        error!(
                                            "Unable to send empty chunk data to {} @ {}, {}: {}",
                                            &player.username, chunk.x, chunk.z, e
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!(
                            "Unable to load chunks for {} @ {}, {}: {}",
                            &player.username, chunk_x, chunk_z, e
                        );
                    }
                }

                debug!(
                    "Sent {} chunks to player: {} @ {:.2},{:.2} in {:?}",
                    (CHUNK_RADIUS * 2) * 2,
                    player.username,
                    position.x,
                    position.z,
                    start.elapsed()
                );
            }

            tokio::time::sleep(Duration::from_secs(5)).await;
        }*/
    }

    async fn stop(self: Arc<Self>, _state: GlobalState) {
        info!("Stopping chunk sender system");
        self.stop.store(true, Ordering::Relaxed);
    }

    fn name(&self) -> &'static str {
        "chunk_sender"
    }
}


fn pos_into_chunk_pos(pos: ComponentRef<Position>) -> (i32, i32) {
    let [chunk_x, chunk_z] = f64x2::from_array([pos.x, pos.z])
        .floor()
        .div(f64x2::from_array([16f64, 16f64]))
        .cast::<i32>()
        .to_array();

    (chunk_x, chunk_z)
}