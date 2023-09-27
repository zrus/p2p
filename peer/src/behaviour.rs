use ::libp2p::{
  dcutr, gossipsub, identify, mdns, ping, relay, rendezvous,
  swarm::NetworkBehaviour,
};

use crate::alive_keeper;

#[derive(NetworkBehaviour)]
pub struct Behaviour {
  pub ping: ping::Behaviour,
  // pub mdns: mdns::tokio::Behaviour,
  pub gossip: gossipsub::Behaviour,
  pub identify: identify::Behaviour,
  pub relay: relay::client::Behaviour,
  pub dcutr: dcutr::Behaviour,
  pub keep_alive: alive_keeper::Behaviour,
  pub rendezvous: rendezvous::client::Behaviour,
}
