pub use self::outgoing::{
  NetSender
};

mod outgoing {
  use std::sync::mpsc::{channel, Sender, Receiver};
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
        loop { send_packet(&socket, &socket_send_rx) }
      });

      NetSender {
        socket_send_tx: socket_send_tx,
        thread_handle: thread_handle
      }
    }
  }

  pub fn send_packet(socket: &UdpSocket, socket_send_rx: &Receiver<SequencedAckedPacket>) {
    let _ =
      socket_send_rx.recv()
        .map(|packet: SequencedAckedPacket| packet.serialize(UDP_MARKER))
        .map(|raw_payload: RawPacket| socket.send_to(&raw_payload.bytes[0..raw_payload.bytes.len()], raw_payload.addr))
        .map(|send_res| send_res.map_err(socket_send_err));
  }

  #[cfg(test)]
  mod tests {
    use std::net:: UdpSocket;
    use std::sync::mpsc::{channel};
    use std::thread;
    use std::net::SocketAddr;
    use std::str::FromStr;
    use constants::UDP_MARKER;
    use super::send_packet;
    use packet_types::SequencedAckedPacket;

    #[test]
    fn send() {
      let send_socket = UdpSocket::bind("127.0.0.1:54739").unwrap();
      let recv_socket = send_socket.try_clone().unwrap();
      let (socket_recv_tx, socket_recv_rx) = channel();
      let message = b"hello world!".into_iter().cloned().collect();

      let expected_packet = SequencedAckedPacket {
        addr: SocketAddr::from_str("127.0.0.1:54739").unwrap(),
        seq_num: 1,
        ack_num: 2,
        ack_field: 3,
        bytes: message
      };

      let result_packet = expected_packet.clone();


      let handle = thread::spawn(move || {
        let mut buf = [0; 23];
        let result = recv_socket.recv_from(&mut buf);
        assert_eq!(result.is_ok(), true);
        assert_eq!(buf.to_vec(), result_packet.serialize(UDP_MARKER).bytes);
      });

      let _ = socket_recv_tx.send(expected_packet);
      send_packet(&send_socket, &socket_recv_rx);

      let _ = handle.join().map_err(|err| panic!(err));
    }
  }
}
