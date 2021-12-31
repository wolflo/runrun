use ethers::{prelude::LocalWallet, utils::Ganache};
use runrun::{core::start, eth::EthRunner};

mod init;
use init::Ctx0;

mod block0;
mod block1;

mod utils;

#[tokio::main]
async fn main() {
    // Start a ganache node
    let node = Ganache::new().port(8547u16).spawn();
    let accts: Vec<LocalWallet> = node.keys()[..5].iter().map(|x| x.clone().into()).collect();

    // Run all tests, starting at Ctx0
    start::<EthRunner<Ctx0>, _, _>((node.endpoint(), accts)).await;
}
