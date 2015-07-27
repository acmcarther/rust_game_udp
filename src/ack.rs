pub use self::ack::{
  PeerAcks,
};

mod ack {
  #[derive(Debug)]
  pub struct PeerAcks {
    pub ack_num: u16,
    pub ack_field: u32
  }

  impl PeerAcks {
    pub fn add_seq_num(&mut self, seq_num: u16) {
      let ack_num_i32: i32 = self.ack_num as i32;
      let seq_num_i32: i32 = seq_num as i32;
      let ack_delta = seq_num_i32 - ack_num_i32;
      // New number is bigger than old number
      let is_normal_newer = ack_delta > 0 && ack_delta < ((u16::max_value() / 2) as i32);
      let is_wraparound_newer = ack_delta < 0 && ack_delta < (-((u16::max_value() / 2) as i32));

      if is_normal_newer {
        if ack_delta < 32 {
          self.ack_field = ((self.ack_field << 1) | 1) << ((ack_delta - 1) as u32);
          self.ack_num = seq_num;
        } else {
          self.ack_field = 0;
          self.ack_num = seq_num;
        }
      } else if is_wraparound_newer {
        let wrap_delta = seq_num.wrapping_sub(self.ack_num);
        if wrap_delta < 32 {
          self.ack_field = ((self.ack_field << 1) | 1) << ((wrap_delta - 1) as u32);
          self.ack_num = seq_num;
        } else {
          self.ack_field = 0;
          self.ack_num = seq_num;
        }
      } else {
        if self.ack_num > seq_num {
          let ack_delta_u16 = self.ack_num - seq_num;
          if ack_delta_u16 < 32 && ack_delta_u16 > 0 {
            self.ack_field = self.ack_field | (1 << (ack_delta_u16 - 1));
          }
        } else {
          let ack_delta_u16 = self.ack_num.wrapping_sub(seq_num);
          if ack_delta_u16 < 32 && ack_delta_u16 > 0 {
            self.ack_field = self.ack_field | (1 << (ack_delta_u16 - 1));
          }
        }
      }
    }
  }

  #[test]
  fn update_ack_map_for_normal_newer() {
    // In range, sets correct flag
    let mut peer_acks = PeerAcks { ack_num: 5, ack_field: 0b101};
    let seq_num = 10;
    peer_acks.add_seq_num(seq_num);
    assert!(peer_acks.ack_num == 10);
    assert!(peer_acks.ack_field == 0b10110000);

    // Out of range, sets ack num and empty flags
    let mut peer_acks = PeerAcks { ack_num: 5, ack_field: 0b101};
    let seq_num = 40;
    peer_acks.add_seq_num(seq_num);
    assert!(peer_acks.ack_num == 40);
    assert!(peer_acks.ack_field == 0);
  }

  #[test]
  fn update_ack_map_for_wraparound_newer() {
    // In range, sets correct flag
    let mut peer_acks = PeerAcks { ack_num: 65535, ack_field: 0b101};
    let seq_num = 4;
    peer_acks.add_seq_num(seq_num);
    assert!(peer_acks.ack_num == 4);
    assert!(peer_acks.ack_field == 0b10110000);

    // Out of range, sets ack num and empty flags
    let mut peer_acks = PeerAcks { ack_num: 65535, ack_field: 0b101};
    let seq_num = 40;
    peer_acks.add_seq_num(seq_num);
    assert!(peer_acks.ack_num == 40);
    assert!(peer_acks.ack_field == 0);
  }

  #[test]
  fn update_ack_map_for_normal_older() {
    // In range, sets correct flag
    let mut peer_acks = PeerAcks { ack_num: 20, ack_field: 0b101};
    let seq_num = 15;
    peer_acks.add_seq_num(seq_num);
    assert!(peer_acks.ack_num == 20);
    assert!(peer_acks.ack_field == 0b10101);

    // Out of range does nothing
    let mut peer_acks = PeerAcks { ack_num: 40, ack_field: 0b101};
    let seq_num = 5;
    peer_acks.add_seq_num(seq_num);
    assert!(peer_acks.ack_num == 40);
    assert!(peer_acks.ack_field == 0b101);
  }

  #[test]
  fn update_ack_map_for_wraparound_older() {
    // In range, sets correct flag
    let mut peer_acks = PeerAcks { ack_num: 5, ack_field: 0b101};
    let seq_num = 65535;
    peer_acks.add_seq_num(seq_num);
    assert!(peer_acks.ack_num == 5);
    assert!(peer_acks.ack_field == 0b100101);

    // Out of range does nothing
    let mut peer_acks = PeerAcks { ack_num: 40, ack_field: 0b101};
    let seq_num = 65535;
    peer_acks.add_seq_num(seq_num);
    assert!(peer_acks.ack_num == 40);
    assert!(peer_acks.ack_field == 0b101);
  }
}
