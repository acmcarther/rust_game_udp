pub use self::constants::{
  UDP_MARKER,
  PACKET_DROP_TIME
};

mod constants {
  pub const UDP_MARKER: &'static [u8] = b"012";
  pub const PACKET_DROP_TIME: i64 = 5; // Seconds
}
