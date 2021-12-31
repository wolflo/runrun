#![allow(dead_code)]

pub mod core;
pub mod eth;
pub mod hooks;
pub mod macros;
pub mod ty;

pub use runrun_derive::*;

#[cfg(test)]
mod tests {
    async fn test() {
        println!("lib test");
    }
}
