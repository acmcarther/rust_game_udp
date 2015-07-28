pub use self::packet_types::{
  RawPacket,
  Packet,
  SequencedPacket,
  SequencedAckedPacket,
  PacketWithTries
};

mod packet_types {
  use std::net::SocketAddr;
  use byteorder::{ByteOrder, BigEndian};

  #[derive(Clone, Debug)]
  pub struct RawPacket {
    pub addr: SocketAddr,
    pub bytes: Vec<u8>
  }

  impl RawPacket {
    pub fn strip_marker(self, marker: &[u8]) -> Option<Packet> {
      if self.starts_with_marker(marker) {
        Some (Packet { addr: self.addr, bytes: self.bytes.into_iter().skip(marker.len()).collect() })
      } else {
        None
      }
    }

    pub fn starts_with_marker(&self, marker: &[u8]) -> bool {
      let self_len = self.bytes.len();
      let marker_len = marker.len();

      !(self_len < marker_len) && self.bytes[0..marker.len()] == *marker
    }
  }

  #[derive(Clone)]
  pub struct Packet {
    pub addr: SocketAddr,
    pub bytes: Vec<u8>
  }

  impl Packet {
    pub fn add_sequence_number(self, sequence_num: u16) -> SequencedPacket {
      SequencedPacket { addr: self.addr, seq_num: sequence_num, bytes: self.bytes }
    }

    pub fn strip_sequence(self) -> SequencedPacket {
      let seq_num = BigEndian::read_u16(&self.bytes[0..2]);
      SequencedPacket {
        addr: self.addr,
        seq_num: seq_num,
        bytes: self.bytes.into_iter().skip(2).collect()
      }
    }
  }

  #[derive(Clone)]
  pub struct SequencedPacket {
    pub addr: SocketAddr,
    pub seq_num: u16,
    pub bytes: Vec<u8>
  }

  impl SequencedPacket {
    pub fn add_acks(self, ack_num: u16, ack_field: u32) -> SequencedAckedPacket {
      SequencedAckedPacket {
        addr: self.addr,
        seq_num: self.seq_num,
        ack_num: ack_num,
        ack_field: ack_field,
        bytes: self.bytes
      }
    }

    pub fn strip_acks(self) -> SequencedAckedPacket {
      let ack_num = BigEndian::read_u16(&self.bytes[0..2]);
      let ack_field = BigEndian::read_u32(&self.bytes[2..6]);
      SequencedAckedPacket {
        addr: self.addr,
        seq_num: self.seq_num,
        ack_num: ack_num,
        ack_field: ack_field,
        bytes: self.bytes.into_iter().skip(6).collect()
      }
    }
  }

  #[derive(Clone)]
  pub struct SequencedAckedPacket {
    pub addr: SocketAddr,
    pub seq_num: u16,
    pub ack_num: u16,
    pub ack_field: u32,
    pub bytes: Vec<u8>
  }

  impl SequencedAckedPacket {
    pub fn serialize(self, marker: &[u8]) -> RawPacket {
      let mut sequence_num_bytes = [0; 2];
      let mut ack_num_bytes = [0; 2];
      let mut ack_field_bytes = [0; 4];
      BigEndian::write_u16(&mut sequence_num_bytes, self.seq_num);
      BigEndian::write_u16(&mut ack_num_bytes, self.ack_num);
      BigEndian::write_u32(&mut ack_field_bytes, self.ack_field);
      let marked_and_seq_bytes: Vec<u8> =
        marker.into_iter().cloned()
          .chain(sequence_num_bytes.iter().cloned())
          .chain(ack_num_bytes.iter().cloned())
          .chain(ack_field_bytes.iter().cloned())
          .chain(self.bytes.iter().cloned()).collect();
      RawPacket {addr: self.addr, bytes: marked_and_seq_bytes}
    }
  }

  pub struct PacketWithTries {
    pub packet: SequencedAckedPacket,
    pub tries: i32
  }

  #[cfg(test)]
  mod tests {
    use std::net::SocketAddr;
    use std::str::FromStr;
    use packet_types::{
      RawPacket,
      Packet,
      SequencedPacket,
      SequencedAckedPacket,
    };

    fn dummy_socket_addr() -> SocketAddr {
      SocketAddr::from_str("127.0.0.1:1000").unwrap()
    }

    #[test]
    fn raw_packet_strip_marker_too_big() {
      let packet = RawPacket { addr: dummy_socket_addr(), bytes: vec![1] };
      let marker = &[1, 2, 3, 4];
      let result = packet.strip_marker(marker);
      assert_eq!(result.is_none(), true);
    }

    #[test]
    fn raw_packet_strip_marker_no_match() {
      let packet = RawPacket { addr: dummy_socket_addr(), bytes: vec![1, 2, 3] };
      let marker = &[2, 1];
      let result = packet.strip_marker(marker);
      assert_eq!(result.is_none(), true);
    }

    #[test]
    fn raw_packet_strip_marker_match() {
      let packet = RawPacket { addr: dummy_socket_addr(), bytes: vec![1, 2, 3, 4] };
      let marker = &[1, 2];
      let result = packet.strip_marker(marker);
      assert_eq!(result.is_some(), true);

      let unwrapped_result = result.unwrap();
      assert_eq!(unwrapped_result.addr, dummy_socket_addr());
      assert_eq!(unwrapped_result.bytes, vec![3, 4]);
    }

    #[test]
    fn packet_strips_sequence_number() {
      let packet = Packet { addr: dummy_socket_addr(), bytes: vec![1, 2, 3] };
      let result = packet.strip_sequence();
      // Sequence is big endian: 1 * 256, 2 * 1 = 258
      assert_eq!(result.addr, dummy_socket_addr());
      assert_eq!(result.seq_num, 258);
      assert_eq!(result.bytes, vec![3]);
    }

    #[test]
    fn sequenced_packet_add_acks() {
      let packet = SequencedPacket { addr: dummy_socket_addr(), seq_num: 5, bytes: vec![1, 2, 3] };
      let result = packet.add_acks(1, 0b11101101);
      assert_eq!(result.addr, dummy_socket_addr());
      assert_eq!(result.seq_num, 5);
      assert_eq!(result.ack_num, 1);
      assert_eq!(result.ack_field, 0b11101101);
      assert_eq!(result.bytes, vec![1, 2, 3]);
    }

    #[test]
    fn sequenced_packet_strips_acks() {
      let packet = SequencedPacket { addr: dummy_socket_addr(), seq_num: 5, bytes: vec![1, 2, 1, 2, 3, 4, 7] };
      let result = packet.strip_acks();
      assert_eq!(result.addr, dummy_socket_addr());
      assert_eq!(result.seq_num, 5);
      // Ack num is big endian: 1 * 256, 2 * 1 = 258
      assert_eq!(result.ack_num, 258);
      // Ack field is big endian: 1 * 0b0001, 2 * 0b0010, etc.
      assert_eq!(result.ack_field, 0b1000000100000001100000100);
      assert_eq!(result.bytes, vec![7]);
    }

    #[test]
    fn sequenced_acked_packet_serialize() {
      let marker = &[1, 2, 3, 4];
      let packet = SequencedAckedPacket {
        addr: dummy_socket_addr(),
        seq_num: 300,
        ack_num: 600,
        ack_field: 111111111,
        bytes: vec![1, 2, 3, 4, 5]
      };
      let result = packet.serialize(marker);
      assert_eq!(result.addr, dummy_socket_addr());

      let expected_bytes: Vec<u8> = vec![
        1, 2, 3, 4,       // Marker
        1, 44,            // Sequence Num
        2, 88,            // Ack Num
        6, 159, 107, 199, // Ack Field
        1, 2, 3, 4, 5     // Payload
      ];

      assert_eq!(result.bytes, expected_bytes);
    }
  }
}
