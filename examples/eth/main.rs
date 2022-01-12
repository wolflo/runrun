use ethers::{prelude::LocalWallet, utils::Ganache};
// use runrun::{core::start, eth::EthRunner, register_ctx};
use runrun::{eth_stream::start_eth, register_ctx};

mod init;
use init::{Ctx0, Inner};

mod block0;
mod block1;
mod block2;
mod block3;
mod block4;
use block1::Ctx1;
use block2::Ctx2;
use block3::Ctx3;
use block4::Ctx4;

mod utils;

register_ctx!(Ctx0, [Ctx1, Ctx2]);
register_ctx!(Ctx1, [Ctx3, Ctx4]);
register_ctx!(Ctx2);
register_ctx!(Ctx3);
register_ctx!(Ctx4);

#[tokio::main]
async fn main() {
    // Start a ganache node
    let node = Ganache::new().spawn();
    let accts: Vec<LocalWallet> = node.keys()[..5].iter().map(|x| x.clone().into()).collect();

    // Run all tests, starting at Ctx0
    start_eth::<Inner, _, Ctx0, _>((node.endpoint(), accts)).await;
    // start::<EthRunner<Ctx0>, _, _>((node.endpoint(), accts)).await;
}
