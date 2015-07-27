pub use self::helpers::{
  try_recv_all
};
mod helpers {
  use std::iter::{repeat};
  use std::sync::mpsc::{Receiver};

  pub fn try_recv_all<T>(ack_rx: &Receiver<T>) -> Vec<T> {
    repeat(()).map(|_| ack_rx.try_recv().ok())
      .take_while(|x| x.is_some())
      .map(|x| x.unwrap())
      .collect()
  }
}

