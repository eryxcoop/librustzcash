use secp256k1::SecretKey;

fn main() {
    let mut sk = SecretKey::from_slice(&[1u8; 32]).unwrap();
    sk.non_secure_erase();
    println!("Compiled!");
}
