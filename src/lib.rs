extern crate byteorder;
extern crate tap;
extern crate time;
extern crate itertools;

pub mod types;
pub mod packet_types;
mod constants;
mod helpers;
mod errors;
mod ack;
mod actors;

use std::net::{SocketAddr, UdpSocket};

use errors::socket_bind_err;
use types::{
  IOHandles,
  Network,
};

use actors::{NetSender, NetReceiver, Director};

type OwnAcks = (SocketAddr, u16, u32);
type DroppedPacket = (SocketAddr, u16);

pub fn start_network(addr: SocketAddr) -> Network {

  let send_socket =
    UdpSocket::bind(addr)
      .map_err(socket_bind_err)
      .unwrap();

  let recv_socket = send_socket.try_clone().unwrap();

  let net_sender = NetSender::new(send_socket);
  let net_receiver = NetReceiver::new(recv_socket);
  let director = Director::new(net_receiver.socket_recv_rx, net_sender.socket_send_tx);

  let io_handles = IOHandles {
    send_handle: net_sender.thread_handle,
    recv_handle: net_receiver.thread_handle,
    direct_handle: director.thread_handle
  };

  Network {
    send_channel: director.api_in_tx,
    recv_channel: director.api_out_rx,
    thread_handles: io_handles
  }
}
