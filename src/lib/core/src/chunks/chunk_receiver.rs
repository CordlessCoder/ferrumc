use ferrumc_world::chunk_format::Chunk;
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;

pub const VIEW_DISTANCE: i32 = 8;
pub struct ChunkReceiver {
    pub can_see: HashSet<(i32, i32, String)>,
    pub seen: HashSet<(i32, i32, String)>,
    pub last_chunk: (i32, i32, String),
    pub chunks_per_tick: f32,
    pub has_loaded: AtomicBool,
}

impl Default for ChunkReceiver {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum ChunkSendState {
    Fetching,
    Sending(Chunk),
    Sent,
}

impl ChunkReceiver {
    pub fn new() -> Self {
        Self {
            can_see: HashSet::new(),
            seen: HashSet::new(),
            last_chunk: (0, 0, "overworld".to_string()),
            chunks_per_tick: 0.0,
            has_loaded: AtomicBool::new(false),
        }
    }
}
