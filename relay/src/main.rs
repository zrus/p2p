use ::clap::Parser;
use ::futures::stream::StreamExt;
use ::futures::{executor::block_on, future::Either};
use ::libp2p::{
  core::multiaddr::Protocol,
  core::muxing::StreamMuxerBox,
  core::upgrade,
  core::{Multiaddr, Transport},
  identify, identity,
  identity::PeerId,
  noise, ping, quic, relay,
  swarm::{keep_alive, NetworkBehaviour, SwarmBuilder, SwarmEvent},
  tcp,
};

use ::std::{
  error::Error,
  net::{Ipv4Addr, Ipv6Addr},
  time::Duration,
};

fn main() -> Result<(), Box<dyn Error>> {
  env_logger::init();

  let opt = Opt::parse();
  println!("Opt: {opt:?}");

  // Create a static known PeerId based on given secret
  let local_key: identity::Keypair = generate_ed25519(opt.secret_key_seed);
  let local_peer_id = PeerId::from(local_key.public());
  println!("Local peer id: {local_peer_id:?}");

  let tcp_transport = tcp::async_io::Transport::default();

  let tcp_transport = tcp_transport
    .upgrade(upgrade::Version::V1Lazy)
    .authenticate(
      noise::Config::new(&local_key)
        .expect("Signing libp2p-noise static DH keypair failed."),
    )
    .multiplex(libp2p::yamux::Config::default());

  let quic_transport =
    quic::async_std::Transport::new(quic::Config::new(&local_key));

  let transport = quic_transport
    .or_transport(tcp_transport)
    .map(|either_output, _| match either_output {
      Either::Left((peer_id, muxer)) => (peer_id, StreamMuxerBox::new(muxer)),
      Either::Right((peer_id, muxer)) => (peer_id, StreamMuxerBox::new(muxer)),
    })
    .boxed();

  let mut relay_config = relay::Config::default();
  relay_config.max_circuit_duration = Duration::ZERO;

  let behaviour = Behaviour {
    relay: relay::Behaviour::new(local_peer_id, Default::default()),
    ping: ping::Behaviour::new(ping::Config::new()),
    identify: identify::Behaviour::new(identify::Config::new(
      "/RELAY/0.1.0".to_string(),
      local_key.public(),
    )),
    keep_alive: Default::default(),
  };

  let mut swarm =
    SwarmBuilder::without_executor(transport, behaviour, local_peer_id).build();

  // Listen on all interfaces
  let listen_addr_tcp = Multiaddr::empty()
    .with(if opt.use_ipv6 {
      Protocol::from(Ipv6Addr::UNSPECIFIED)
    } else {
      Protocol::from(Ipv4Addr::UNSPECIFIED)
    })
    .with(Protocol::Tcp(opt.port));
  swarm.listen_on(listen_addr_tcp)?;

  let listen_addr_quic = Multiaddr::empty()
    .with(if opt.use_ipv6 {
      Protocol::from(Ipv6Addr::UNSPECIFIED)
    } else {
      Protocol::from(Ipv4Addr::UNSPECIFIED)
    })
    .with(Protocol::Udp(opt.port))
    .with(Protocol::QuicV1);
  swarm.listen_on(listen_addr_quic)?;

  block_on(async {
    loop {
      match swarm.next().await.expect("Infinite Stream.") {
        SwarmEvent::Behaviour(event) => {
          if let BehaviourEvent::Identify(identify::Event::Received {
            info: identify::Info { observed_addr, .. },
            ..
          }) = &event
          {
            swarm.add_external_address(observed_addr.clone());
          }

          println!("{event:?}")
        }
        SwarmEvent::NewListenAddr { address, .. } => {
          println!("Listening on {address:?}");
        }
        _ => {}
      }
    }
  })
}

#[derive(NetworkBehaviour)]
struct Behaviour {
  relay: relay::Behaviour,
  ping: ping::Behaviour,
  identify: identify::Behaviour,
  keep_alive: keep_alive::Behaviour,
}

fn generate_ed25519(secret_key_seed: u8) -> identity::Keypair {
  let mut bytes = [0u8; 32];
  bytes[0] = secret_key_seed;

  identity::Keypair::ed25519_from_bytes(bytes)
    .expect("only errors on wrong length")
}

#[derive(Debug, Parser)]
#[clap(name = "libp2p relay")]
struct Opt {
  /// Determine if the relay listen on ipv6 or ipv4 loopback address. the default is ipv4
  #[clap(long = "ipv6", default_value = "false")]
  use_ipv6: bool,

  /// Fixed value to generate deterministic peer id
  #[clap(long, short = 's', default_value = "0")]
  secret_key_seed: u8,

  /// The port used to listen on all interfaces
  #[clap(long, short = 'p', default_value = "4111")]
  port: u16,
}
