pub use self::outgoing::{
  NetSender
};

mod outgoing {
  use std::sync::mpsc::{channel, Sender};
  use std::thread;
  use std::thread::JoinHandle;
  use std::net:: UdpSocket;
  use packet_types::{
    RawPacket,
    SequencedAckedPacket
  };
  use constants::UDP_MARKER;
  use errors::socket_send_err;

  pub struct NetSender {
    pub socket_send_tx: Sender<SequencedAckedPacket>,
    pub thread_handle: JoinHandle<()>
  }

  impl NetSender {
    pub fn new(socket: UdpSocket) -> NetSender {
      let (socket_send_tx, socket_send_rx) = channel();

      let thread_handle = thread::spawn (move || {
        loop {
          let _ =
            socket_send_rx.recv()
              .map(|packet: SequencedAckedPacket| packet.serialize(UDP_MARKER))
              .map(|raw_payload: RawPacket| socket.send_to(&raw_payload.bytes[0..raw_payload.bytes.len()], raw_payload.addr))
              .map(|send_res| send_res.map_err(socket_send_err));
        }
      });

      NetSender {
        socket_send_tx: socket_send_tx,
        thread_handle: thread_handle
      }
    }
  }
}
