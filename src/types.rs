pub use self::types::{
  IOHandles,
  Network,
  PeerAcks,
  RawSocketPayload,
  SocketPayload,
  SequencedSocketPayload,
  SequencedAckedSocketPayload,
  PacketWithTries,
};

mod types {
  use std::thread::JoinHandle;
  use std::sync::mpsc::{Receiver, Sender};
  use std::net::SocketAddr;

  pub struct IOHandles {
    pub send_handle: JoinHandle<()>,
    pub recv_handle: JoinHandle<()>
  }

  pub struct Network {
    pub send_channel: Sender<SocketPayload>,
    pub recv_channel: Receiver<SocketPayload>,
    pub thread_handles: IOHandles
  }

  #[derive(Debug)]
  pub struct PeerAcks {
    pub ack_num: u16,
    pub ack_field: u32
  }

  #[derive(Clone)]
  pub struct RawSocketPayload {
    pub addr: SocketAddr,
    pub bytes: Vec<u8>
  }

  #[derive(Clone)]
  pub struct SocketPayload {
    pub addr: SocketAddr,
    pub bytes: Vec<u8>
  }

  #[derive(Clone)]
  pub struct SequencedSocketPayload {
    pub addr: SocketAddr,
    pub seq_num: u16,
    pub bytes: Vec<u8>
  }

  #[derive(Clone)]
  pub struct SequencedAckedSocketPayload {
    pub addr: SocketAddr,
    pub seq_num: u16,
    pub ack_num: u16,
    pub ack_field: u32,
    pub bytes: Vec<u8>
  }

  pub struct PacketWithTries {
    pub packet: SequencedAckedSocketPayload,
    pub tries: i32
  }

}
