use zeroize::Zeroize;
use bip32::ExtendedPrivateKey;
use secp256k1::SecretKey;

fn check_zeroize<T: Zeroize>(_: T) {}

fn main() {
    // This will only compile if ExtendedPrivateKey implements Zeroize
}
