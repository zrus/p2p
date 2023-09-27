use ::clap::Parser;
use ::env_logger::{init_from_env, Env};
use ::log::*;
use ::peer::{Command, Peer};

#[derive(Debug, Parser)]
struct Opts {
  /// Fixed value to generate deterministic peer id
  #[clap(short = 's', default_value = "1")]
  secret_key_seed: u8,
  /// The listening address
  #[clap(short = 'r')]
  relay_address: Option<::libp2p::Multiaddr>,
  /// Peer ID of the peer to dial
  #[clap(short = 'p')]
  remote_peer_id: Option<::libp2p::PeerId>,
  /// Topics to publish messages
  #[clap(long = "pub", value_delimiter = ',')]
  publish: Vec<String>,
  /// Topics to subscribe
  #[clap(long = "sub", value_delimiter = ',')]
  subscribe: Vec<String>,
  #[clap(
    long = "rp",
    default_value = "12D3KooWD5GfYYkeeXQfC4NmMoxnVeLNdTQ4bU616FVegHWKxPnH"
  )]
  rendezvous_point: ::libp2p::PeerId,
  #[clap(long = "ra", default_value = "/ip4/113.161.95.53/tcp/62649")]
  rendezvous_addr: ::libp2p::Multiaddr,
  #[clap(short, default_value = "false")]
  discovery: bool,
}

fn main() -> Result<(), Box<dyn ::std::error::Error>> {
  let env = Env::default().default_filter_or("info");
  init_from_env(env);
  let opts = Opts::parse();
  info!("Opts: {opts:?}");

  ::peer::init();
  let peer = Peer::init(opts.secret_key_seed);
  ::peer::run(peer)?;

  if !opts.publish.is_empty() {
    ::peer::actor::<Peer>()?.tell(Command::PublishTopics {
      topics: opts.publish,
    })?;
  }
  if !opts.subscribe.is_empty() {
    ::peer::actor::<Peer>()?.tell(Command::SubscribeTopics {
      topics: opts.subscribe,
    })?;
  }
  if let Some(ref addr) = opts.relay_address {
    ::peer::actor::<Peer>()?.tell(Command::ListenViaRelay {
      relay_address: addr.clone(),
    })?;
  }
  if let Some(ref peer_id) = opts.remote_peer_id {
    ::peer::actor::<Peer>()?.tell(Command::Dial {
      relay_address: opts.relay_address,
      remote_peer_id: peer_id.clone(),
    })?;
  }
  if opts.discovery {
    ::peer::actor::<Peer>()?.tell(Command::RendezvousDiscover {
      point: opts.rendezvous_point,
      addr: opts.rendezvous_addr,
    })?;
  } else {
    ::peer::actor::<Peer>()?.tell(Command::RendezvousRegister {
      point: opts.rendezvous_point,
      addr: opts.rendezvous_addr,
    })?;
  }

  ::peer::block_until_stopped();
  Ok(())
}
