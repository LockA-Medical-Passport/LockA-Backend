//! Generate a dedicated SEP-10 server seed locally. Store output in a secret manager.
use rand::{RngCore, rngs::OsRng};
fn main() {
    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);
    println!("{}", stellar_strkey::ed25519::PrivateKey(seed));
}
