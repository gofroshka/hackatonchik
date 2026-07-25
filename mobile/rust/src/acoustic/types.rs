#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Clone)]
pub struct TxInfo {
    pub id: String,
    pub name: String,
    pub content_type: String,
    pub original_size: u64,
    pub encoded_size: u64,
    pub packet_count: u32,
    pub estimated_seconds: f64,
}

#[flutter_rust_bridge::frb(non_opaque)]
pub struct TxChunk {
    pub pcm16_le: Vec<u8>,
    pub progress: f64,
    pub packet_index: u32,
    pub packet_count: u32,
    pub done: bool,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Default)]
pub struct MobileReceiveEvent {
    pub kind: String,
    pub id: String,
    pub name: Option<String>,
    pub content_type: Option<String>,
    pub path: Option<String>,
    pub text: Option<String>,
    pub message: Option<String>,
    pub original_size: Option<u64>,
    pub completed_groups: Option<u32>,
    pub total_groups: Option<u32>,
}
