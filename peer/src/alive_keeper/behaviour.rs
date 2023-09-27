use std::task::Poll;

use super::ConnectionHandler;
use libp2p::swarm::NetworkBehaviour;
use void::Void;

#[derive(Default)]
pub struct Behaviour;

impl NetworkBehaviour for Behaviour {
  type ConnectionHandler = ConnectionHandler;

  type ToSwarm = Void;

  fn handle_established_inbound_connection(
    &mut self,
    _: libp2p::swarm::ConnectionId,
    _: libp2p::PeerId,
    _: &libp2p::Multiaddr,
    _: &libp2p::Multiaddr,
  ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied>
  {
    Ok(ConnectionHandler)
  }

  fn handle_established_outbound_connection(
    &mut self,
    _: libp2p::swarm::ConnectionId,
    _: libp2p::PeerId,
    _: &libp2p::Multiaddr,
    _: libp2p::core::Endpoint,
  ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied>
  {
    Ok(ConnectionHandler)
  }

  fn on_connection_handler_event(
    &mut self,
    _: libp2p::PeerId,
    _: libp2p::swarm::ConnectionId,
    event: libp2p::swarm::THandlerOutEvent<Self>,
  ) {
    void::unreachable(event)
  }

  fn poll(
    &mut self,
    cx: &mut std::task::Context<'_>,
    params: &mut impl libp2p::swarm::PollParameters,
  ) -> std::task::Poll<
    libp2p::swarm::ToSwarm<Self::ToSwarm, libp2p::swarm::THandlerInEvent<Self>>,
  > {
    Poll::Pending
  }

  fn on_swarm_event(
    &mut self,
    _: libp2p::swarm::FromSwarm<Self::ConnectionHandler>,
  ) {
  }
}
