use ::tiny_tokio_actor::{ActorContext, Handler, Message};
use futures::SinkExt;

use crate::{event::SysEvent, Peer};

#[derive(Debug, Clone)]
pub enum Command {
  SubscribeTopics {
    topics: Vec<String>,
  },
  UnsubscribeTopics {
    topics: Vec<String>,
  },
  PublishTopics {
    topics: Vec<String>,
  },
  ListenViaRelay {
    relay_address: ::libp2p::Multiaddr,
  },
  Dial {
    relay_address: Option<::libp2p::Multiaddr>,
    remote_peer_id: ::libp2p::PeerId,
  },
  Send {
    topics: Vec<String>,
    message: Vec<u8>,
  },
}

impl Message for Command {
  type Response = ();
}

#[async_trait::async_trait]
impl Handler<SysEvent, Command> for Peer {
  async fn handle(&mut self, msg: Command, _: &mut ActorContext<SysEvent>) {
    self.sender().send(msg).await.expect("Message send failed");
  }
}
