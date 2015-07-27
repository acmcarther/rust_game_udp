#![feature(convert)]
extern crate byteorder;
extern crate tap;
extern crate time;

pub mod types;
mod constants;
mod errors;

use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::{channel, Sender, Receiver, RecvError};
use std::collections::HashMap;
use std::iter::{repeat};
use std::thread;

use byteorder::{ByteOrder, BigEndian};
use tap::{Tappable, TappableIter};
use time::{Duration, PreciseTime};

use errors::{socket_bind_err, socket_recv_err, socket_send_err};
use types::{
  IOHandles,
  PeerAcks,
  Network,
  RawSocketPayload,
  SocketPayload,
  SequencedSocketPayload,
  SequencedAckedSocketPayload,
  PacketWithTries
};
use constants::{
  UDP_MARKER,
  PACKET_DROP_TIME,
  MAX_RESEND_ATTEMPTS
};

type OwnAcks = (SocketAddr, u16, u32);
type DroppedPacket = (SocketAddr, u16);

pub fn start_network(addr: SocketAddr) -> Network {

  let (send_tx, send_rx) = channel();
  let (recv_tx, recv_rx) = channel();

  let (send_attempted_tx, send_attempted_rx) = channel();
  let (dropped_packet_tx, dropped_packet_rx) = channel();
  let (received_packet_tx, received_packet_rx) = channel();

  let send_socket =
    UdpSocket::bind(addr)
      .map_err(socket_bind_err)
      .unwrap();

  let recv_socket = send_socket.try_clone().unwrap();

  // Sending messages
  let send_handle = thread::spawn (move || {
    let mut seq_num_map = HashMap::new();
    let mut ack_map = HashMap::new();
    loop {
      try_recv_all(&received_packet_rx)
        .into_iter()
        .map(|(addr, seq_num)| update_ack_map(addr, seq_num, &mut ack_map))
        .collect::<Vec<()>>();    // TODO: Remove collect

      //let acks = try_recv_all(&ack_rx);
      try_recv_all(&dropped_packet_rx)
        .into_iter()
        .map(|packet_with_tries: PacketWithTries| {
          let old_packet = packet_with_tries.packet;
          let tries = packet_with_tries.tries;
          let payload = SocketPayload {addr: old_packet.addr, bytes: old_packet.bytes};
          deliver_packet(Ok(payload), &send_attempted_tx, &send_socket, &mut seq_num_map, &ack_map, tries + 1);
        })
        .collect::<Vec<()>>();    // TODO: Remove collect

      deliver_packet(send_rx.recv(), &send_attempted_tx, &send_socket, &mut seq_num_map, &ack_map, 0);
    }
  });

  // Receiving messages
  let recv_handle = thread::spawn (move || {
    let mut packets_awaiting_ack = HashMap::new();
    loop {
      let mut buf = [0; 256];

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
        .filter(|&(_,_,tries)| tries < MAX_RESEND_ATTEMPTS)
        .map(|(packet, _, tries)| {let _ = dropped_packet_tx.send(PacketWithTries{packet: packet, tries: tries});}).collect::<Vec<()>>(); // TODO: Remove collect

      // Add all new sent packets packets_awaiting_ack
      try_recv_all(&send_attempted_rx)
        .into_iter()
        .map(|(sent_packet, timestamp, attempts)| {
          packets_awaiting_ack.insert(
            (sent_packet.addr.clone(), sent_packet.seq_num.clone()),
            (sent_packet, timestamp, attempts)
          );
        }).collect::<Vec<()>>();   // TODO: Get rid of this collect

      let payload_result = recv_socket.recv_from(&mut buf)
        .map_err(socket_recv_err)
        .map(|(_, socket_addr)| RawSocketPayload {addr: socket_addr, bytes: buf.to_vec()})
        .map(starts_with_marker);

      let _ = payload_result.map(|possible_payload| {
        possible_payload
          .map(strip_marker)
          .map(strip_sequence)
          .map(strip_acks)
          .tap(|packet| delete_acked_packets(&packet, &mut packets_awaiting_ack))
          .tap(|packet| received_packet_tx.send((packet.addr, packet.seq_num)))
          .map(|payload| recv_tx.send(SocketPayload{addr: payload.addr, bytes: payload.bytes}))
      });
    }
  });
  let io_handles = IOHandles { send_handle: send_handle, recv_handle: recv_handle };
  Network { send_channel: send_tx, recv_channel: recv_rx, thread_handles: io_handles }
}

fn starts_with_marker(payload: RawSocketPayload) -> Option<RawSocketPayload> {
  if &payload.bytes[0..3] == UDP_MARKER {
    Some(payload)
  } else {
    None
  }
}

fn increment_seq_number(seq_num_map: &mut HashMap<SocketAddr, u16>, addr: SocketAddr) -> u16 {
  let count = seq_num_map.entry(addr).or_insert(0);
  *count += 1;
  count.clone()
}

fn add_sequence_number(payload: SocketPayload, sequence_num: u16) -> SequencedSocketPayload {
  SequencedSocketPayload { addr: payload.addr, seq_num: sequence_num, bytes: payload.bytes }
}

fn add_acks(payload: SequencedSocketPayload, ack_num: u16, ack_field: u32) -> SequencedAckedSocketPayload {
  SequencedAckedSocketPayload {
    addr: payload.addr,
    seq_num: payload.seq_num,
    ack_num: ack_num,
    ack_field: ack_field,
    bytes: payload.bytes
  }
}

fn serialize(payload: SequencedAckedSocketPayload) -> RawSocketPayload {
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

fn strip_marker(payload: RawSocketPayload) -> SocketPayload {
  SocketPayload { addr: payload.addr, bytes: payload.bytes[3..256].into_iter().cloned().collect() }
}

fn strip_sequence(payload: SocketPayload) -> SequencedSocketPayload {
  let seq_num = BigEndian::read_u16(&payload.bytes[0..2]);
  SequencedSocketPayload {
    addr: payload.addr,
    seq_num: seq_num,
    bytes: payload.bytes[2..253].into_iter().cloned().collect()
  }
}

fn strip_acks(payload: SequencedSocketPayload) -> SequencedAckedSocketPayload {
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

fn try_recv_all<T>(ack_rx: &Receiver<T>) -> Vec<T> {
  repeat(()).map(|_| ack_rx.try_recv().ok())
    .take_while(|x| x.is_some())
    .map(|x| x.unwrap())
    .collect()
}

fn delete_acked_packets(packet: &SequencedAckedSocketPayload, packets_awaiting_ack: &mut HashMap<(SocketAddr, u16), (SequencedAckedSocketPayload, PreciseTime, i32)>) {
  let ack_num = packet.ack_num;
  let ack_field = packet.ack_field;
  let past_acks = (0..32).map(|bit_idx| {
    // Builds a bit mask, and checks if bit is present by comparing result to 0
    (bit_idx, 0 != ((1 << bit_idx) & ack_field))
  });

  // Remove initial ack
  packets_awaiting_ack.remove(&(packet.addr, ack_num));

  // Remove subsequent acks
  past_acks.map(|(idx, was_acked)| {
    if was_acked {
      packets_awaiting_ack.remove(&(packet.addr, ack_num - idx));
    }
  }).collect::<Vec<()>>(); // TODO: no collect
}

fn deliver_packet(
  packet_result: Result<SocketPayload, RecvError>,
  send_attempted_tx: &Sender<(SequencedAckedSocketPayload, PreciseTime, i32)>,
  send_socket: &UdpSocket,
  seq_num_map: &mut HashMap<SocketAddr, u16>,
  ack_map: &HashMap<SocketAddr, PeerAcks>,
  prior_attempts: i32
  ) {
  let _ =
    packet_result
      .map(|raw_payload: SocketPayload| {
        let addr = raw_payload.addr.clone();
        (raw_payload, increment_seq_number(seq_num_map, addr))
      })
      .map(|(raw_payload, seq_num)| add_sequence_number(raw_payload, seq_num))
      .map(|raw_payload: SequencedSocketPayload| {
        let default = PeerAcks{ack_num: 0, ack_field: 0}; // TODO: remove this when we dont need it
        let ack_data = ack_map.get(&raw_payload.addr).unwrap_or(&default);
        (raw_payload, ack_data.ack_num, ack_data.ack_field) // Fake ack_num for now
      })
      .map(|(payload, ack_num, ack_field)| add_acks(payload, ack_num, ack_field))
      .tap(|final_payload| send_attempted_tx.send((final_payload.clone(), PreciseTime::now(), prior_attempts)))
      .map(serialize)
      .map(|raw_payload: RawSocketPayload| send_socket.send_to(raw_payload.bytes.as_slice(), raw_payload.addr))
      .map(|send_res| send_res.map_err(socket_send_err));
}

fn update_ack_map(addr: SocketAddr, seq_num: u16, ack_map: &mut HashMap<SocketAddr, PeerAcks>) {
  let peer_acks = ack_map.entry(addr).or_insert(PeerAcks { ack_num: 0, ack_field: 0 });
  update_peer_acks(seq_num, peer_acks);
}

// TODO: More thorough testing
fn update_peer_acks(seq_num: u16, peer_acks: &mut PeerAcks) {
  let ack_num_i32: i32 = peer_acks.ack_num as i32;
  let seq_num_i32: i32 = seq_num as i32;
  let ack_delta = seq_num_i32 - ack_num_i32;
  // New number is bigger than old number
  let is_normal_newer = ack_delta > 0 && ack_delta < ((u16::max_value() / 2) as i32);
  let is_wraparound_newer = ack_delta < 0 && ack_delta < (-((u16::max_value() / 2) as i32));

  if is_normal_newer {
    if ack_delta < 32 {
      peer_acks.ack_field = ((peer_acks.ack_field << 1) | 1) << ((ack_delta - 1) as u32);
      peer_acks.ack_num = seq_num;
    } else {
      peer_acks.ack_field = 0;
      peer_acks.ack_num = seq_num;
    }
  } else if is_wraparound_newer {
    let wrap_delta = seq_num.wrapping_sub(peer_acks.ack_num);
    if wrap_delta < 32 {
      peer_acks.ack_field = ((peer_acks.ack_field << 1) | 1) << ((wrap_delta - 1) as u32);
      peer_acks.ack_num = seq_num;
    } else {
      peer_acks.ack_field = 0;
      peer_acks.ack_num = seq_num;
    }
  } else {
    if peer_acks.ack_num > seq_num {
      let ack_delta_u16 = peer_acks.ack_num - seq_num;
      if ack_delta_u16 < 32 && ack_delta_u16 > 0 {
        peer_acks.ack_field = peer_acks.ack_field | (1 << (ack_delta_u16 - 1));
      }
    } else {
      let ack_delta_u16 = peer_acks.ack_num.wrapping_sub(seq_num);
      if ack_delta_u16 < 32 && ack_delta_u16 > 0 {
        peer_acks.ack_field = peer_acks.ack_field | (1 << (ack_delta_u16 - 1));
      }
    }
  }
}

#[test]
fn update_ack_map_for_normal_newer() {
  // In range, sets correct flag
  let mut peer_acks = PeerAcks { ack_num: 5, ack_field: 0b101};
  let seq_num = 10;
  update_peer_acks(seq_num, &mut peer_acks);
  assert!(peer_acks.ack_num == 10);
  assert!(peer_acks.ack_field == 0b10110000);

  // Out of range, sets ack num and empty flags
  let mut peer_acks = PeerAcks { ack_num: 5, ack_field: 0b101};
  let seq_num = 40;
  update_peer_acks(seq_num, &mut peer_acks);
  assert!(peer_acks.ack_num == 40);
  assert!(peer_acks.ack_field == 0);
}

#[test]
fn update_ack_map_for_wraparound_newer() {
  // In range, sets correct flag
  let mut peer_acks = PeerAcks { ack_num: 65535, ack_field: 0b101};
  let seq_num = 4;
  update_peer_acks(seq_num, &mut peer_acks);
  assert!(peer_acks.ack_num == 4);
  assert!(peer_acks.ack_field == 0b10110000);

  // Out of range, sets ack num and empty flags
  let mut peer_acks = PeerAcks { ack_num: 65535, ack_field: 0b101};
  let seq_num = 40;
  update_peer_acks(seq_num, &mut peer_acks);
  assert!(peer_acks.ack_num == 40);
  assert!(peer_acks.ack_field == 0);
}

#[test]
fn update_ack_map_for_normal_older() {
  // In range, sets correct flag
  let mut peer_acks = PeerAcks { ack_num: 20, ack_field: 0b101};
  let seq_num = 15;
  update_peer_acks(seq_num, &mut peer_acks);
  assert!(peer_acks.ack_num == 20);
  assert!(peer_acks.ack_field == 0b10101);

  // Out of range does nothing
  let mut peer_acks = PeerAcks { ack_num: 40, ack_field: 0b101};
  let seq_num = 5;
  update_peer_acks(seq_num, &mut peer_acks);
  assert!(peer_acks.ack_num == 40);
  assert!(peer_acks.ack_field == 0b101);
}

#[test]
fn update_ack_map_for_wraparound_older() {
  // In range, sets correct flag
  let mut peer_acks = PeerAcks { ack_num: 5, ack_field: 0b101};
  let seq_num = 65535;
  update_peer_acks(seq_num, &mut peer_acks);
  assert!(peer_acks.ack_num == 5);
  assert!(peer_acks.ack_field == 0b100101);

  // Out of range does nothing
  let mut peer_acks = PeerAcks { ack_num: 40, ack_field: 0b101};
  let seq_num = 65535;
  update_peer_acks(seq_num, &mut peer_acks);
  assert!(peer_acks.ack_num == 40);
  assert!(peer_acks.ack_field == 0b101);
}
