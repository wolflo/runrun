#![allow(unused)]
use fuel_tx::Salt;
use fuels_abigen_macro::abigen;
use fuels_rs::contract::Contract;
use fuel_core::service::{Config, FuelService};
use fuel_gql_client::client::FuelClient;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use runrun::{register_ctx, core::{BaseRunner, Ctx, Built}};

mod utils;
use crate::utils::TestContract;

mod init;
use init::Ctx0;
mod block1;
use block1::Ctx1;

register_ctx!(Ctx0, [Ctx1]);
register_ctx!(Ctx1);

#[tokio::main]
async fn main() {
    let rng = &mut StdRng::seed_from_u64(2322u64);

    let server = FuelService::new_node(Config::local_node()).await.unwrap();

    let builder = BaseRunner::builder();
    runrun::start::<Ctx0, _, _>(builder, server.bound_address).await;
}
