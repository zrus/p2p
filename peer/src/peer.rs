use ::std::{str::FromStr, time::Duration};

use ::anyhow::anyhow;
use ::async_trait::async_trait;
use ::futures::{
  channel::mpsc::{self, UnboundedReceiver, UnboundedSender},
  StreamExt,
};
use ::libp2p::{
  core::{transport::OrTransport, upgrade},
  dcutr,
  dns::TokioDnsConfig,
  gossipsub, identify,
  identity::Keypair,
  mdns,
  multiaddr::Protocol,
  noise, ping, relay, rendezvous,
  swarm::{keep_alive, SwarmBuilder, SwarmEvent},
  tcp, yamux, Multiaddr, PeerId, Transport,
};
use ::log::*;
use ::tiny_tokio_actor::{Actor, ActorContext, ActorError, Handler, Message};
use ::tokio::{io::AsyncBufReadExt, task::JoinHandle};

use crate::{
  alive_keeper,
  behaviour::{Behaviour, BehaviourEvent},
  command::Command,
  event::SysEvent,
  interface::IActor,
  utils::R,
};

#[derive(Debug, Clone)]
pub struct Envelope {
  topic: String,
  payload: Vec<u8>,
}

impl Message for Envelope {
  type Response = ();
}

#[async_trait]
impl Handler<SysEvent, Envelope> for Peer {
  async fn handle(&mut self, _: Envelope, _: &mut ActorContext<SysEvent>) {
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
  }
}

pub struct Peer {
  local_key: Keypair,
  local_peer_id: PeerId,
  handler: Option<JoinHandle<R>>,
  sender: Option<UnboundedSender<Command>>,
  receiver: Option<UnboundedReceiver<Command>>,
}

impl IActor for Peer {
  fn named() -> String {
    String::from("peer")
  }
}

impl Peer {
  pub fn sender(&mut self) -> &UnboundedSender<Command> {
    self.sender.as_mut().expect("Sender not set yet")
  }

  pub fn init(seed: u8) -> Self {
    let mut bytes = [0u8; 32];
    bytes[1] = seed;
    let local_key = Keypair::ed25519_from_bytes(bytes).unwrap();
    let local_peer_id = PeerId::from(local_key.public());
    Self {
      local_key,
      local_peer_id,
      handler: None,
      sender: None,
      receiver: None,
    }
  }

  async fn build(&mut self) -> R<JoinHandle<R>> {
    let mut receiver = self.receiver.take().unwrap();
    let local_key = self.local_key.clone();
    let local_peer_id = self.local_peer_id.clone();
    info!("Local peer id: {local_peer_id}");

    // Keep alive behaviour configuration
    let keep_alive = alive_keeper::Behaviour::default();

    // Ping behaviour configuration
    let ping_config = ping::Config::new()
      .with_interval(Duration::from_secs(5))
      .with_timeout(Duration::from_secs(5));
    let ping = ping::Behaviour::new(ping_config);

    // mDNS behaviour configuration
    let mdns = mdns::tokio::Behaviour::new(Default::default(), local_peer_id)?;

    // Gossipsub behaviour configuration
    let gossip_config = gossipsub::ConfigBuilder::default()
      .validation_mode(gossipsub::ValidationMode::Strict)
      .build()
      .map_err(|e| anyhow!(e))?;
    let privacy = gossipsub::MessageAuthenticity::Signed(local_key.clone());
    let gossip = gossipsub::Behaviour::new(privacy, gossip_config)
      .map_err(|e| anyhow!(e))?;

    // Identify behaviour configuration
    let identify_config =
      identify::Config::new("/p2p/0.1.0".to_owned(), local_key.public());
    let identify = identify::Behaviour::new(identify_config);

    // Relay client behaviour configuration
    let (relay_transport, relay) = relay::client::new(local_peer_id);

    // DCUtR behaviour configuration
    let dcutr = dcutr::Behaviour::new(local_peer_id);

    // Rendezvous client behaviour configuration
    let rendezvous = rendezvous::client::Behaviour::new(local_key.clone());

    // TCP transport
    let tcp_transport_config =
      tcp::Config::new().nodelay(true).port_reuse(true);
    let tcp_transport = tcp::tokio::Transport::new(tcp_transport_config);

    // Transport
    let transport =
      OrTransport::new(relay_transport, TokioDnsConfig::system(tcp_transport)?)
        .upgrade(upgrade::Version::V1Lazy)
        .authenticate(
          noise::Config::new(&local_key)
            .expect("signing libp2p-noise static keypair"),
        )
        .multiplex(yamux::Config::default())
        .timeout(Duration::from_secs(10))
        .boxed();

    let behaviour = Behaviour {
      ping,
      // mdns,
      gossip,
      identify,
      relay,
      dcutr,
      keep_alive,
      rendezvous,
    };

    let mut swarm =
      SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id)
        .build();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    let mut delay = ::tokio::time::interval(Duration::from_secs(1));
    delay.tick().await;
    loop {
      ::tokio::select! {
        e = swarm.select_next_some() => match e {
          SwarmEvent::NewListenAddr { listener_id: _, address } => {
            info!("New listening at {address}");
          },
          SwarmEvent::Behaviour(_) => {}
          SwarmEvent::IncomingConnection { .. } => {}
          SwarmEvent::ConnectionEstablished { .. } => {}
          e => panic!("{e:?}"),
        },
        _ = delay.tick() => {
          break;
        }
      }
    }
    drop(delay);

    Ok(::tokio::runtime::Handle::current().spawn(async move {
      let mut stdin = ::tokio::io::BufReader::new(::tokio::io::stdin()).lines();
      let mut publish_topics = vec![];
      
      let mut discover_tick = ::tokio::time::interval(Duration::from_secs(30));
      let mut cookie = None;
      let mut rendezvous_node = None;

      loop {
        ::tokio::select! {
          _ = discover_tick.tick(), if cookie.is_some() && rendezvous_node.is_some() => {
            swarm
              .behaviour_mut()
              .rendezvous
              .discover(
                Some(rendezvous::Namespace::from_static("aum_pos")),
                cookie.clone(),
                None,
                rendezvous_node.unwrap()
              );
          }
          Ok(Some(ref line)) = stdin.next_line() => {
            for topic in publish_topics.iter() {
              let topic = gossipsub::Sha256Topic::new(topic);
              if let Err(e) = swarm.behaviour_mut().gossip.publish(topic, line.clone()) {
                error!("Send failed: {e}");
              }
            }
          }
          Some(command) = receiver.next() => {
            match command {
              Command::SubscribeTopics { topics } => {
                for topic in topics {
                  _ = swarm.behaviour_mut().gossip.subscribe(&gossipsub::Sha256Topic::new(topic));
                }
              },
              Command::UnsubscribeTopics { topics } => {
                for topic in topics {
                  _ = swarm.behaviour_mut().gossip.unsubscribe(&gossipsub::Sha256Topic::new(topic));
                }
              },
              Command::PublishTopics { mut topics } => publish_topics.append(&mut topics),
              Command::ListenViaRelay { relay_address } => {
                let mut learned_observed_addr = false;
                let mut told_relay_observed_addr = false;

                swarm.dial(relay_address.clone())?;

                loop {
                  match swarm.select_next_some().await {
                    SwarmEvent::NewListenAddr { .. } => {}
                    SwarmEvent::Dialing { .. } => {}
                    SwarmEvent::ConnectionEstablished { .. } => {}
                    SwarmEvent::Behaviour(BehaviourEvent::Identify(identify::Event::Sent { .. })) => {
                      info!("Told relay its public address.");
                      told_relay_observed_addr = true;
                    }
                    SwarmEvent::Behaviour(BehaviourEvent::Identify(identify::Event::Received {
                      info: identify::Info { observed_addr, .. },
                      ..
                    })) => {
                      info!("Relay told us our public address: {observed_addr:?}");
                      swarm.add_external_address(observed_addr);
                      learned_observed_addr = true;
                    }
                    event => debug!("{event:?}"),
                  }

                  if learned_observed_addr && told_relay_observed_addr {
                    break;
                  }
                }

                swarm.listen_on(relay_address.with(libp2p::multiaddr::Protocol::P2pCircuit))?;
              },
              Command::Dial { relay_address, remote_peer_id } => {
                match relay_address {
                  Some(addr) => {
                    swarm.dial(
                      addr
                        .with(libp2p::multiaddr::Protocol::P2pCircuit)
                        .with(libp2p::multiaddr::Protocol::P2p(remote_peer_id)),
                      )?;
                  }
                  None => {
                    swarm.dial(
                      Multiaddr::from_str("/ip4/192.168.1.18/tcp/63413")?.with(Protocol::P2p(remote_peer_id))
                    )?;
                  }
                }
              },
              Command::Send { topics, message } => todo!(),
              Command::RendezvousRegister { point, addr } => {
                info!("Register to rendezvous..");
                let external_addr = "/ip4/113.161.95.53/tcp/4111".parse::<Multiaddr>()?;
                // let external_addr = "/ip4/127.0.0.1/tcp/0".parse::<Multiaddr>()?;
                swarm.add_external_address(external_addr);
                swarm.dial(addr.with(Protocol::P2p(point)))?;
                                
                'rdvz: loop {
                  match swarm.select_next_some().await {
                    SwarmEvent::ConnectionEstablished { peer_id, .. } if peer_id == point => {
                      swarm.behaviour_mut().rendezvous.register(
                        rendezvous::Namespace::from_static("aum_pos"),
                        point,
                        None,
                      )?;
                      break 'rdvz;
                    }
                    event => debug!("{event:?}"),
                  }
                }
              }
              Command::RendezvousDiscover { point, addr } => {
                info!("Discover by rendezvous..");
                swarm.dial(addr.with(Protocol::P2p(point)))?;

                'rdvz: loop {
                  match swarm.select_next_some().await {
                    SwarmEvent::ConnectionEstablished { peer_id, .. } if peer_id == point => {
                      info!(
                          "Connected to rendezvous point, discovering nodes in '{}' namespace ...",
                          "aum_pos"
                      );
                      swarm.behaviour_mut().rendezvous.discover(
                          Some(rendezvous::Namespace::from_static("aum_pos")),
                          None,
                          None,
                          point,
                      );
                      break 'rdvz;
                    }
                    event => debug!("{event:?}"),
                  }
                }
              }
            }
          }
          event = swarm.select_next_some() => match event {
            SwarmEvent::Behaviour(event) => {
              match event {
                BehaviourEvent::KeepAlive(_) => {},
                // BehaviourEvent::Mdns(mdns) => match mdns {
                //   mdns::Event::Discovered(discovered) => {
                //     for (peer, addr) in discovered {
                //       info!("New peer {peer} at {addr}");
                //       swarm.behaviour_mut().gossip.add_explicit_peer(&peer);
                //     }
                //   },
                //   mdns::Event::Expired(expired) => {
                //     for (peer, addr) in expired {
                //       warn!("Peer {peer} at {addr} expired");
                //       swarm.behaviour_mut().gossip.remove_explicit_peer(&peer);
                //     }
                //   },
                // },
                BehaviourEvent::Gossip(gossib) => match gossib {
                  gossipsub::Event::Message { propagation_source: _, message_id: _, message } => {
                    let msg = String::from_utf8_lossy(&message.data);
                    info!("Received: {msg}\n");
                  },
                  gossipsub::Event::Subscribed { peer_id, topic } => {
                    info!("Peer {peer_id} subscribed on {topic}!");
                  },
                  gossipsub::Event::Unsubscribed { .. } => todo!(),
                  gossipsub::Event::GossipsubNotSupported { peer_id } => {
                    warn!("Gossip not supported from {peer_id}");
                  },
                },
                BehaviourEvent::Identify(identify) => match identify {
                  identify::Event::Received { peer_id, info } => debug!("Received from {peer_id} with: {info:?}"),
                  e => debug!("{e:?}"),
                }
                BehaviourEvent::Relay(relay) => {
                  use libp2p::relay::client::Event as E;
                  match relay {
                    E::ReservationReqAccepted { .. } => {
                        info!("Relay accepted our reservation request");
                    },
                    e => info!("Relay event: {e:?}"),
                  }
                },
                BehaviourEvent::Dcutr(dcutr) => {
                  info!("DCUtR event: {dcutr:?}");
                  if let libp2p::dcutr::Event::DirectConnectionUpgradeSucceeded { remote_peer_id } = dcutr {
                    _ = swarm.behaviour_mut().gossip.add_explicit_peer(&remote_peer_id);
                  }
                },
                BehaviourEvent::Ping(ping) => {
                  debug!("{ping:?}");
                }
                BehaviourEvent::Rendezvous(rendezvous) => {
                  use rendezvous::client::Event::*;
                  match rendezvous {
                    Discovered { registrations, cookie: new_cookie, .. } => {
                      cookie.replace(new_cookie);

                      for registration in registrations {
                        for address in registration.record.addresses() {
                          let peer = registration.record.peer_id();
                          info!("Discovered peer {} at {}", peer, address);

                          let p2p_suffix = Protocol::P2p(peer);
                          let address_with_p2p =
                              if !address.ends_with(&Multiaddr::empty().with(p2p_suffix.clone())) {
                                  address.clone().with(p2p_suffix)
                              } else {
                                  address.clone()
                              };

                          swarm.dial(address_with_p2p).unwrap();
                        }
                      }
                    },
                    DiscoverFailed { rendezvous_node, namespace, error } => todo!(),
                    Registered { rendezvous_node, ttl, namespace } => {
                      info!(
                        "Registered for namespace '{}' at rendezvous point {} for the next {} seconds",
                        namespace,
                        rendezvous_node,
                        ttl
                      );
                    },
                    RegisterFailed { rendezvous_node, namespace, error } => {
                      error!(
                        "Failed to register: rendezvous_node={}, namespace={}, error_code={:?}",
                        rendezvous_node,
                        namespace,
                        error
                      );
                    },
                    Expired { peer } => todo!(),
                  }
                }
              }
            },
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
              info!("Connection established with {peer_id} - {endpoint:?}");
            },
            SwarmEvent::ConnectionClosed { peer_id, endpoint, cause, .. } => {
              warn!("Connection at {peer_id} - {endpoint:?} closed due to: {cause:?}");
              info!("Network info: {:?}", swarm.network_info());
              if endpoint.is_relayed() {
                _ = swarm.dial(endpoint.get_remote_address().clone());
              }
            },
            SwarmEvent::IncomingConnection { local_addr, .. } => {
              info!("Incomming connection: {local_addr}");
            },
            SwarmEvent::IncomingConnectionError { local_addr, error, .. } => {
              error!("Incoming connection error: {error} at {local_addr}");
            },
            SwarmEvent::OutgoingConnectionError { error, .. } => {
              error!("Outgoing connection error: {error}");
            },
            SwarmEvent::NewListenAddr { address, .. } => {
              info!("New listening at {address}");
            },
            SwarmEvent::ExpiredListenAddr { address, .. } => {
              warn!("Listening expired at {address}");
            },
            SwarmEvent::ListenerClosed { addresses, reason, .. } => {
              warn!("Listener closed on {addresses:?} due to {reason:?}");
            },
            SwarmEvent::ListenerError { error, .. } => {
              error!("Listen error: {error:?}");
            },
            SwarmEvent::Dialing { peer_id, connection_id: _ } => {
              if let Some(peer_id) = peer_id {
                info!("Dialing to {peer_id}..");
              }
            },
          }
        }
      }
    }))
  }
}

#[async_trait]
impl Actor<SysEvent> for Peer {
  async fn pre_start(
    &mut self,
    _ctx: &mut ActorContext<SysEvent>,
  ) -> Result<(), ActorError> {
    let (sender, receiver) = mpsc::unbounded();
    self.sender = Some(sender);
    self.receiver = Some(receiver);
    self.handler = Some(
      self
        .build()
        .await
        .map_err(|e| ActorError::RuntimeError(e.into()))?,
    );
    Ok(())
  }
}
