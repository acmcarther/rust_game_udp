pub use self::incoming::{
  NetReceiver
};

mod incoming {
  use std::sync::mpsc::{channel, Receiver};
  use std::thread;
  use std::thread::JoinHandle;
  use std::net:: UdpSocket;

  use packet_types::{
    RawPacket,
    SequencedAckedPacket
  };
  use constants::UDP_MARKER;
  use errors::socket_recv_err;

  pub struct NetReceiver{
    pub socket_recv_rx: Receiver<SequencedAckedPacket>,
    pub thread_handle: JoinHandle<()>
  }

  impl NetReceiver {
    pub fn new(socket: UdpSocket) -> NetReceiver {
      let (socket_recv_tx, socket_recv_rx) = channel();

      let thread_handle = thread::spawn (move || {
        loop {
          let mut buf = [0; 256];
          let _ = socket.recv_from(&mut buf)
            .map_err(socket_recv_err)
            .map(|(_, socket_addr)| RawPacket {addr: socket_addr, bytes: buf.to_vec()})
            .ok()
            .and_then(|packet| packet.strip_marker(UDP_MARKER))
            .map(|packet| packet.strip_sequence())
            .map(|packet| packet.strip_acks())
            .map(|packet| socket_recv_tx.send(packet));
        }
      });

      NetReceiver {
        socket_recv_rx: socket_recv_rx,
        thread_handle: thread_handle
      }
    }
  }
}
