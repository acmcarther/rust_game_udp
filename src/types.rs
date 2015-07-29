pub use self::types::{
  IOHandles,
  Network,
};

mod types {
  use std::thread::JoinHandle;
  use std::sync::mpsc::{Receiver, Sender};
  use packet_types::Packet;

  pub struct IOHandles {
    pub send_handle: JoinHandle<()>,
    pub recv_handle: JoinHandle<()>,
    pub direct_handle: JoinHandle<()>
  }

  pub struct Network {
    pub send_channel: Sender<Packet>,
    pub recv_channel: Receiver<Packet>,
    pub thread_handles: IOHandles
  }
}
