#![allow(unused_variables)]
#![allow(dead_code)]

pub mod core;
pub mod eth;
pub mod hooks;
pub mod ty;

#[cfg(test)]
mod tests {
    async fn test() {
        println!("lib test");
    }
}
