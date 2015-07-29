pub use self::state::{
  Director
};

mod state {
  use std::net::SocketAddr;
  use std::sync::mpsc::{channel, Sender, Receiver};
  use std::collections::HashMap;
  use std::thread;
  use std::thread::JoinHandle;
  use time::{Duration, PreciseTime};
  use packet_types::{
    Packet,
    SequencedPacket,
    SequencedAckedPacket,
    PacketWithTries
  };
  use constants::{
    MAX_RESEND_ATTEMPTS,
    PACKET_DROP_TIME,
  };
  use ack::PeerAcks;

  use helpers::try_recv_all;
  use itertools::Itertools;


  pub struct Director{
    pub api_out_rx: Receiver<Packet>,
    pub api_in_tx: Sender<Packet>,
    pub thread_handle: JoinHandle<()>
  }

  // TODO: Fix
  impl Director {
    pub fn new(socket_recv_rx: Receiver<SequencedAckedPacket>, socket_send_tx: Sender<SequencedAckedPacket>) -> Director {
      let (api_out_tx, api_out_rx) = channel();
      let (api_in_tx, api_in_rx) = channel();
      let mut seq_num_map = HashMap::new();
      let mut ack_map = HashMap::new();
      let mut packets_awaiting_ack = HashMap::new();

      let thread_handle = thread::spawn (move || {
        loop {
          let recv_packets = try_recv_all(&socket_recv_rx);
          let send_packets = try_recv_all(&api_in_rx);
          let dropped_packets = extract_dropped_packets(&mut packets_awaiting_ack);

          recv_packets.into_iter()
            .map(|packet| {
              delete_acked_packets(&packet, &mut packets_awaiting_ack);
              add_packet_to_ack_map(packet.addr.clone(), packet.seq_num.clone(), &mut ack_map);
              packet
            })
            .foreach(|packet| {let _ = api_out_tx.send(Packet {addr: packet.addr, bytes: packet.bytes});});

          dropped_packets.into_iter()
            .filter(|dropped_packet| dropped_packet.tries < MAX_RESEND_ATTEMPTS)
            .map(|dropped_packet| (dropped_packet.packet, dropped_packet.tries))
            .map(|(packet, tries)| (Packet{addr:packet.addr, bytes: packet.bytes}, tries))
            .chain(send_packets.into_iter().map(|packet| (packet, 0)))
            .map(|(packet, tries): (Packet, i32)| {
              let new_seq_num = increment_seq_number(&mut seq_num_map, packet.addr.clone());
              (packet.add_sequence_number(new_seq_num), tries)
            })
            .map(|(packet, tries): (SequencedPacket, i32)| {
              let default = PeerAcks {ack_num: 0, ack_field: 0}; // TODO: remove this when we dont need it
              let ack_data = ack_map.get(&packet.addr).unwrap_or(&default);
              (packet.add_acks(ack_data.ack_num, ack_data.ack_field), tries)
            })
            .map(|(final_payload, tries)| {
              add_packet_to_waiting(&final_payload, tries, &mut packets_awaiting_ack);
              final_payload
            })
            .foreach(|final_payload| {let _ = socket_send_tx.send(final_payload);});
        }
      });

      Director {
        api_out_rx: api_out_rx,
        api_in_tx: api_in_tx,
        thread_handle: thread_handle
      }
    }
  }
  fn extract_dropped_packets(packets_awaiting_ack: &mut HashMap<(SocketAddr, u16), (SequencedAckedPacket, PreciseTime, i32)>) -> Vec<PacketWithTries>{
    let now = PreciseTime::now();
    // Notify send thread of dropped packets
    //   Get keys first to sate the borrow checker
    let dropped_packet_keys: Vec<(SocketAddr, u16)> =
      packets_awaiting_ack.iter()
        .filter(|&(_, &(_, timestamp, _))| {
          let timestamp: PreciseTime = timestamp; // Compiler why?
          let time_elapsed: Duration = timestamp.to(now);
          time_elapsed.num_seconds() > PACKET_DROP_TIME
        })
        .map(|(key, &(_, _, _))| {
          let key: &(SocketAddr, u16) = key; // Compiler why?
          key.clone()
        }).collect();

    dropped_packet_keys.iter()
      .map(|key| packets_awaiting_ack.remove(&key))
      .filter(|result| result.is_some())
      .map(|result| result.unwrap())
      .map(|(packet, _, tries)| PacketWithTries {packet: packet, tries: tries})
      .collect()
  }

  fn delete_acked_packets(packet: &SequencedAckedPacket, packets_awaiting_ack: &mut HashMap<(SocketAddr, u16), (SequencedAckedPacket, PreciseTime, i32)>) {
    let ack_num = packet.ack_num;
    let ack_field = packet.ack_field;
    (0..32).map(|bit_idx| {
      // Builds a bit mask, and checks if bit is present by comparing result to 0
      (bit_idx, 0 != ((1 << bit_idx) & ack_field))
    }).foreach(|(idx, was_acked)| {
      if was_acked {
        packets_awaiting_ack.remove(&(packet.addr, ack_num - idx));
      }
    });

    // Remove initial ack
    packets_awaiting_ack.remove(&(packet.addr, ack_num));
  }

  fn increment_seq_number(seq_num_map: &mut HashMap<SocketAddr, u16>, addr: SocketAddr) -> u16 {
    let count = seq_num_map.entry(addr).or_insert(0);
    *count += count.wrapping_add(1);
    count.clone()
  }


  fn add_packet_to_waiting(packet: &SequencedAckedPacket, tries: i32, packets_awaiting_ack: &mut HashMap<(SocketAddr, u16), (SequencedAckedPacket, PreciseTime, i32)>) {
    packets_awaiting_ack.insert(
      (packet.addr.clone(), packet.seq_num.clone()),
      (packet.clone(), PreciseTime::now(), tries + 1)
    );
  }

  fn add_packet_to_ack_map(addr: SocketAddr, seq_num: u16, ack_map: &mut HashMap<SocketAddr, PeerAcks>) {
    let peer_acks = ack_map.entry(addr).or_insert(PeerAcks { ack_num: 0, ack_field: 0 });
    peer_acks.add_seq_num(seq_num); // TODO: Rename this so it doesn't sound like we're making a new packet
  }
}
