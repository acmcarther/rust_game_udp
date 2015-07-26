pub use self::errors::{
  socket_bind_err,
  socket_recv_err,
  socket_send_err,
};

mod errors {
  use std::io::Error;

  pub fn socket_bind_err(err: Error) {
    println!("UDP: Error binding socket: {}", err)
  }

  pub fn socket_recv_err(err: Error) {
    println!("UDP: Error receiving from socket: {}", err)
  }

  pub fn socket_send_err(err: Error) {
    println!("UDP: Error sending to socket: {}", err)
  }

}
