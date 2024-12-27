use ferrumc_ecs::entities::Entity;
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use ferrumc_state::GlobalState;
use crate::connection::StreamWriter;
use crate::NetResult;
use crate::utils::ecs_helpers::EntityExt;

pub trait SendPacketToConnExt {
    async fn send_packet(&self, state: &GlobalState, packet: &(impl NetEncode + Sync)) -> NetResult<()> {
        self.send_packet_with_opts(state, packet, &NetEncodeOpts::WithLength).await
    }
    async fn send_packet_with_opts(&self, state: &GlobalState, packet: &(impl NetEncode + Sync), opts: &NetEncodeOpts) -> NetResult<()>;

    async fn send_packet_owned(&self, state: &GlobalState, packet: impl NetEncode + Sync) -> NetResult<()> {
        self.send_packet_with_opts(state, &packet, &NetEncodeOpts::WithLength).await
    }
}

impl SendPacketToConnExt for Entity {
    async fn send_packet_with_opts(&self, state: &GlobalState, packet: &(impl NetEncode + Sync), opts: &NetEncodeOpts) -> NetResult<()> {
        self.get_mut::<StreamWriter>(state)?.send_packet(packet, opts).await
    }
}