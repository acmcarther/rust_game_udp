pub use self::constants::{
  UDP_MARKER,
  PACKET_DROP_TIME,
  MAX_RESEND_ATTEMPTS
};

mod constants {
  pub const UDP_MARKER: &'static [u8] = b"012";
  pub const PACKET_DROP_TIME: i64 = 5; // Seconds
  pub const MAX_RESEND_ATTEMPTS: i32 = 5;
}
