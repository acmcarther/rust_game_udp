pub use self::packet_building::{
  add_sequence_number,
  add_acks,
  serialize
};

mod packet_building {
  use byteorder::{ByteOrder, BigEndian};
  use constants::{
    UDP_MARKER,
  };
  use types::{
    RawSocketPayload,
    SocketPayload,
    SequencedSocketPayload,
    SequencedAckedSocketPayload,
  };

  pub fn add_sequence_number(payload: SocketPayload, sequence_num: u16) -> SequencedSocketPayload {
    SequencedSocketPayload { addr: payload.addr, seq_num: sequence_num, bytes: payload.bytes }
  }

  pub fn add_acks(payload: SequencedSocketPayload, ack_num: u16, ack_field: u32) -> SequencedAckedSocketPayload {
    SequencedAckedSocketPayload {
      addr: payload.addr,
      seq_num: payload.seq_num,
      ack_num: ack_num,
      ack_field: ack_field,
      bytes: payload.bytes
    }
  }

  pub fn serialize(payload: SequencedAckedSocketPayload) -> RawSocketPayload {
    let mut sequence_num_bytes = [0; 2];
    let mut ack_num_bytes = [0; 2];
    let mut ack_field_bytes = [0; 4];
    BigEndian::write_u16(&mut sequence_num_bytes, payload.seq_num);
    BigEndian::write_u16(&mut ack_num_bytes, payload.ack_num);
    BigEndian::write_u32(&mut ack_field_bytes, payload.ack_field);
    let marked_and_seq_bytes: Vec<u8> =
      UDP_MARKER.into_iter().cloned()
        .chain(sequence_num_bytes.iter().cloned())
        .chain(ack_num_bytes.iter().cloned())
        .chain(ack_field_bytes.iter().cloned())
        .chain(payload.bytes.iter().cloned()).collect();
    RawSocketPayload {addr: payload.addr, bytes: marked_and_seq_bytes}
  }

}
