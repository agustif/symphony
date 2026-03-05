#![forbid(unsafe_code)]

use anyhow::Result;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    println!("symphony-rust bootstrap ready");
    Ok(())
}
