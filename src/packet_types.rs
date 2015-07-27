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
  use constants::{
    UDP_MARKER,
  };

  #[derive(Clone)]
  pub struct RawPacket {
    pub addr: SocketAddr,
    pub bytes: Vec<u8>
  }

  impl RawPacket {
    pub fn strip_marker(self) -> Packet {
      Packet { addr: self.addr, bytes: self.bytes[3..256].into_iter().cloned().collect() }
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
        bytes: self.bytes[2..253].into_iter().cloned().collect()
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
        bytes: self.bytes[6..251].into_iter().cloned().collect()
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
    pub fn serialize(self) -> RawPacket {
      let mut sequence_num_bytes = [0; 2];
      let mut ack_num_bytes = [0; 2];
      let mut ack_field_bytes = [0; 4];
      BigEndian::write_u16(&mut sequence_num_bytes, self.seq_num);
      BigEndian::write_u16(&mut ack_num_bytes, self.ack_num);
      BigEndian::write_u32(&mut ack_field_bytes, self.ack_field);
      let marked_and_seq_bytes: Vec<u8> =
        UDP_MARKER.into_iter().cloned()
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
}
