use std::task::Poll;

use libp2p::{
  core::upgrade::DeniedUpgrade,
  swarm::{
    handler, ConnectionHandler as ConnHandler, KeepAlive, SubstreamProtocol,
  },
};
use void::Void;

#[derive(Debug, Clone)]
pub struct ConnectionHandler;

impl ConnHandler for ConnectionHandler {
  type FromBehaviour = Void;

  type ToBehaviour = Void;

  type Error = Void;

  type InboundProtocol = DeniedUpgrade;

  type OutboundProtocol = DeniedUpgrade;

  type InboundOpenInfo = ();

  type OutboundOpenInfo = Void;

  fn listen_protocol(
    &self,
  ) -> libp2p::swarm::SubstreamProtocol<
    Self::InboundProtocol,
    Self::InboundOpenInfo,
  > {
    SubstreamProtocol::new(DeniedUpgrade, ())
  }

  fn on_behaviour_event(&mut self, e: Self::FromBehaviour) {
    void::unreachable(e)
  }

  fn connection_keep_alive(&self) -> libp2p::swarm::KeepAlive {
    KeepAlive::Yes
  }

  fn poll(
    &mut self,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<
    libp2p::swarm::ConnectionHandlerEvent<
      Self::OutboundProtocol,
      Self::OutboundOpenInfo,
      Self::ToBehaviour,
      Self::Error,
    >,
  > {
    Poll::Pending
  }

  fn on_connection_event(
    &mut self,
    event: libp2p::swarm::handler::ConnectionEvent<
      Self::InboundProtocol,
      Self::OutboundProtocol,
      Self::InboundOpenInfo,
      Self::OutboundOpenInfo,
    >,
  ) {
    use libp2p::swarm::handler::ConnectionEvent::*;
    match event {
      FullyNegotiatedInbound(handler::FullyNegotiatedInbound {
        protocol,
        ..
      }) => void::unreachable(protocol),
      FullyNegotiatedOutbound(handler::FullyNegotiatedOutbound {
        protocol,
        ..
      }) => void::unreachable(protocol),
      AddressChange(_)
      | DialUpgradeError(_)
      | ListenUpgradeError(_)
      | LocalProtocolsChange(_)
      | RemoteProtocolsChange(_) => {}
    }
  }
}
