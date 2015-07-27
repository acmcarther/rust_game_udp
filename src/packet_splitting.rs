pub use self::packet_splitting::{
  strip_marker,
  strip_sequence,
  strip_acks
};

mod packet_splitting {
  use byteorder::{ByteOrder, BigEndian};
  use types::{
    RawSocketPayload,
    SocketPayload,
    SequencedSocketPayload,
    SequencedAckedSocketPayload,
  };

  pub fn strip_marker(payload: RawSocketPayload) -> SocketPayload {
    SocketPayload { addr: payload.addr, bytes: payload.bytes[3..256].into_iter().cloned().collect() }
  }

  pub fn strip_sequence(payload: SocketPayload) -> SequencedSocketPayload {
    let seq_num = BigEndian::read_u16(&payload.bytes[0..2]);
    SequencedSocketPayload {
      addr: payload.addr,
      seq_num: seq_num,
      bytes: payload.bytes[2..253].into_iter().cloned().collect()
    }
  }

  pub fn strip_acks(payload: SequencedSocketPayload) -> SequencedAckedSocketPayload {
    let ack_num = BigEndian::read_u16(&payload.bytes[0..2]);
    let ack_field = BigEndian::read_u32(&payload.bytes[2..6]);
    SequencedAckedSocketPayload {
      addr: payload.addr,
      seq_num: payload.seq_num,
      ack_num: ack_num,
      ack_field: ack_field,
      bytes: payload.bytes[6..251].into_iter().cloned().collect()
    }
  }

}
