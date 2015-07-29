pub use self::actors::{
  NetSender,
  NetReceiver,
  Director,
};

mod outgoing;
mod incoming;
mod state;

mod actors {
  pub use actors::outgoing::NetSender;
  pub use actors::incoming::NetReceiver;
  pub use actors::state::Director;
}
