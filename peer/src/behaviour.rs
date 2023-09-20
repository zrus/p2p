use ::libp2p::{
  dcutr, gossipsub, identify, mdns, ping, relay, swarm::NetworkBehaviour,
};
use libp2p::swarm::keep_alive;

#[derive(NetworkBehaviour)]
pub struct Behaviour {
  pub ping: ping::Behaviour,
  pub mdns: mdns::tokio::Behaviour,
  pub gossip: gossipsub::Behaviour,
  pub identify: identify::Behaviour,
  pub relay: relay::client::Behaviour,
  pub dcutr: dcutr::Behaviour,
  pub keep_alive: keep_alive::Behaviour,
}
