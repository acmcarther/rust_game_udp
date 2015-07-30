pub use self::incoming::{
  NetReceiver
};

mod incoming {
  use std::sync::mpsc::{channel, Sender, Receiver};
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
        loop { receive_packet(&socket, &socket_recv_tx) }
      });

      NetReceiver {
        socket_recv_rx: socket_recv_rx,
        thread_handle: thread_handle
      }
    }

  }

  pub fn receive_packet(socket: &UdpSocket, socket_recv_tx: &Sender<SequencedAckedPacket>) {
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

  #[cfg(test)]
  mod tests {
    use std::net:: UdpSocket;
    use std::sync::mpsc::{channel, RecvError};
    use std::thread;
    use std::net::SocketAddr;
    use std::str::FromStr;
    use constants::UDP_MARKER;
    use super::receive_packet;
    use packet_types::SequencedAckedPacket;

    #[test]
    fn receive_not_marked() {
      let send_socket = UdpSocket::bind("127.0.0.1:54732").unwrap();
      let recv_socket = send_socket.try_clone().unwrap();
      let (socket_recv_tx, socket_recv_rx) = channel();

      let handle = thread::spawn(move || {
        receive_packet(&recv_socket, &socket_recv_tx)
      });

      let _ = send_socket.send_to(b"not_marker", "127.0.0.1:54732");
      let _ = handle.join();
      let result = socket_recv_rx.recv();

      assert_eq!(result.is_err(), true);
      assert_eq!(result.err().unwrap(), RecvError);
    }

    #[test]
    fn receive_marked() {
      let send_socket = UdpSocket::bind("127.0.0.1:54734").unwrap();
      let recv_socket = send_socket.try_clone().unwrap();
      let (socket_recv_tx, socket_recv_rx) = channel();

      let handle = thread::spawn(move || {
        receive_packet(&recv_socket, &socket_recv_tx)
      });
      let message = b"hello world!".into_iter().cloned().collect();

      let expected_packet = SequencedAckedPacket {
        addr: SocketAddr::from_str("127.0.0.1:54734").unwrap(),
        seq_num: 1,
        ack_num: 2,
        ack_field: 3,
        bytes: message
      };
      let raw_packet = expected_packet.clone().serialize(UDP_MARKER);

      let _ = send_socket.send_to(&raw_packet.bytes[0..raw_packet.bytes.len()], raw_packet.addr);
      let _ = handle.join();
      let result = socket_recv_rx.recv();

      assert_eq!(result.is_ok(), true);
      let full_result = result.unwrap();
      assert_eq!(full_result.seq_num, expected_packet.seq_num);
      assert_eq!(full_result.ack_num, expected_packet.ack_num);
      assert_eq!(full_result.ack_field, expected_packet.ack_field);

      // Trim packet
      let bytes_result =
        String::from_utf8(full_result.bytes).unwrap()
          .trim_matches('\0').to_string();

      assert_eq!(bytes_result, "hello world!");
    }
  }
}
