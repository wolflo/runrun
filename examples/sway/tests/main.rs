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
    runrun::start::<Ctx0, _, _>(BaseRunner::builder(), ()).await;
}
