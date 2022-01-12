use anyhow::Result;
use ethers::prelude::*;
use std::{fs, path::Path, sync::Arc};

const BUILD_DIR: &str = "./examples/eth_test/out";
abigen!(
    ERC20MinterPauser,
    "./examples/eth_test/out/ERC20MinterPauser.abi",
    event_derives(serde::Deserialize, serde::Serialize)
);

pub fn make_factory<M>(name: &str, client: &Arc<M>) -> Result<ContractFactory<M>>
where
    M: Middleware,
{
    let name = String::from(name);
    let build_dir = Path::new(BUILD_DIR);

    let abi_raw = fs::read_to_string(&build_dir.join(name.clone() + ".abi"))?;
    let abi = serde_json::from_str(&abi_raw)?;

    let bin_raw = fs::read_to_string(&build_dir.join(name + ".bin"))?;
    let bin_raw = bin_raw.trim_end_matches("\n");
    let bin: Bytes = hex::decode(&bin_raw)?.into();

    Ok(ContractFactory::new(abi, bin, client.clone()))
}
