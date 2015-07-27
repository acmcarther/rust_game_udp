#![feature(convert)]
extern crate byteorder;
extern crate tap;
extern crate time;

pub mod types;
pub mod packet_types;
mod constants;
mod helpers;
mod errors;
mod ack;

use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::{channel, Sender, Receiver, RecvError};
use std::collections::HashMap;
use std::thread;

use tap::{Tappable, TappableIter};
use time::{Duration, PreciseTime};

use errors::{socket_bind_err, socket_recv_err, socket_send_err};
use types::{
  IOHandles,
  Network,
};
use packet_types::{
  RawPacket,
  Packet,
  SequencedPacket,
  SequencedAckedPacket,
  PacketWithTries
};
use constants::{
  UDP_MARKER,
  PACKET_DROP_TIME,
  MAX_RESEND_ATTEMPTS
};

use ack::PeerAcks;

use helpers::try_recv_all;

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
      update_acks(&received_packet_rx, &mut ack_map);

      try_recv_all(&dropped_packet_rx)
        .into_iter()
        .map(|packet_with_tries: PacketWithTries| {
          let old_packet = packet_with_tries.packet;
          let tries = packet_with_tries.tries;
          let payload = Packet {addr: old_packet.addr, bytes: old_packet.bytes};
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
        .map(|(_, socket_addr)| RawPacket {addr: socket_addr, bytes: buf.to_vec()})
        .map(starts_with_marker);

      let _ = payload_result.map(|possible_payload| {
        possible_payload
          .map(|packet| packet.strip_marker())
          .map(|packet| packet.strip_sequence())
          .map(|packet| packet.strip_acks())
          .tap(|packet| delete_acked_packets(&packet, &mut packets_awaiting_ack))
          .tap(|packet| received_packet_tx.send((packet.addr, packet.seq_num)))
          .map(|packet| recv_tx.send(Packet {addr: packet.addr, bytes: packet.bytes} ))
      });
    }
  });
  let io_handles = IOHandles { send_handle: send_handle, recv_handle: recv_handle };
  Network { send_channel: send_tx, recv_channel: recv_rx, thread_handles: io_handles }
}

fn update_acks(received_packet_rx: &Receiver<(SocketAddr, u16)>, ack_map: &mut HashMap<SocketAddr, PeerAcks>) {
  try_recv_all(&received_packet_rx)
    .into_iter()
    .map(|(addr, seq_num)| update_ack_map(addr, seq_num, ack_map))
    .collect::<Vec<()>>();    // TODO: Remove collect
}

fn starts_with_marker(payload: RawPacket) -> Option<RawPacket> {
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

fn delete_acked_packets(packet: &SequencedAckedPacket, packets_awaiting_ack: &mut HashMap<(SocketAddr, u16), (SequencedAckedPacket, PreciseTime, i32)>) {
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
  packet_result: Result<Packet, RecvError>,
  send_attempted_tx: &Sender<(SequencedAckedPacket, PreciseTime, i32)>,
  send_socket: &UdpSocket,
  seq_num_map: &mut HashMap<SocketAddr, u16>,
  ack_map: &HashMap<SocketAddr, PeerAcks>,
  prior_attempts: i32
  ) {
  let _ =
    packet_result
      .map(|raw_payload: Packet| {
        let addr = raw_payload.addr.clone();
        (raw_payload, increment_seq_number(seq_num_map, addr))
      })
      .map(|(raw_payload, seq_num)| raw_payload.add_sequence_number(seq_num))
      .map(|raw_payload: SequencedPacket| {
        let default = PeerAcks{ack_num: 0, ack_field: 0}; // TODO: remove this when we dont need it
        let ack_data = ack_map.get(&raw_payload.addr).unwrap_or(&default);
        (raw_payload, ack_data.ack_num, ack_data.ack_field) // Fake ack_num for now
      })
      .map(|(payload, ack_num, ack_field)| payload.add_acks(ack_num, ack_field))
      .tap(|final_payload| send_attempted_tx.send((final_payload.clone(), PreciseTime::now(), prior_attempts)))
      .map(|packet| packet.serialize())
      .map(|raw_payload: RawPacket| send_socket.send_to(raw_payload.bytes.as_slice(), raw_payload.addr))
      .map(|send_res| send_res.map_err(socket_send_err));
}

fn update_ack_map(addr: SocketAddr, seq_num: u16, ack_map: &mut HashMap<SocketAddr, PeerAcks>) {
  let peer_acks = ack_map.entry(addr).or_insert(PeerAcks { ack_num: 0, ack_field: 0 });
  peer_acks.add_seq_num(seq_num);
}
