use core::{
    array::TryFromSliceError,
    convert::{TryFrom, TryInto},
};

use ecdsa::elliptic_curve::generic_array::GenericArray;
use hmac::{Hmac, Mac};
use rand::{CryptoRng, RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use sha2::{digest::Update, Sha256};
use tracing::error;
use zeroize::Zeroize;

use crate::{
    collections::TypedUsize,
    sdk::api::{TofnFatal, TofnResult},
};

#[derive(Debug, Clone, Zeroize)]
#[zeroize(drop)]
pub struct SecretRecoveryKey(pub(crate) [u8; 64]);

impl TryFrom<&[u8]> for SecretRecoveryKey {
    type Error = TryFromSliceError;

    fn try_from(v: &[u8]) -> Result<Self, Self::Error> {
        Ok(Self(v.try_into()?))
    }
}

const SESSION_NONCE_LENGTH_MIN: usize = 4;
const SESSION_NONCE_LENGTH_MAX: usize = 256;

pub(crate) fn rng_seed<K>(
    tag: u8,
    party_id: TypedUsize<K>,
    secret_recovery_key: &SecretRecoveryKey,
    session_nonce: &[u8],
) -> TofnResult<impl CryptoRng + RngCore> {
    if session_nonce.len() < SESSION_NONCE_LENGTH_MIN
        || session_nonce.len() > SESSION_NONCE_LENGTH_MAX
    {
        error!(
            "invalid session_nonce length {} not in [{},{}]",
            session_nonce.len(),
            SESSION_NONCE_LENGTH_MIN,
            SESSION_NONCE_LENGTH_MAX
        );
        return Err(TofnFatal);
    }

    // TODO: Use protocol domain separation: https://github.com/axelarnetwork/tofn/issues/184
    let seed = Hmac::<Sha256>::new(secret_recovery_key.0[..].into())
        .chain(tag.to_be_bytes())
        .chain(party_id.to_bytes())
        .chain(session_nonce)
        .finalize()
        .into_bytes()
        .into();

    Ok(ChaCha20Rng::from_seed(seed))
}

/// Initialize a RNG by hashing the arguments.
/// Intended for use generating a ECDSA signing key.
pub(crate) fn rng_seed_ecdsa_signing_key(
    protocol_tag: u8,
    tag: u8,
    secret_recovery_key: &SecretRecoveryKey,
    session_nonce: &[u8],
) -> TofnResult<impl CryptoRng + RngCore> {
    if session_nonce.len() < SESSION_NONCE_LENGTH_MIN
        || session_nonce.len() > SESSION_NONCE_LENGTH_MAX
    {
        error!(
            "invalid session_nonce length {} not in [{},{}]",
            session_nonce.len(),
            SESSION_NONCE_LENGTH_MIN,
            SESSION_NONCE_LENGTH_MAX
        );
        return Err(TofnFatal);
    }

    // Take care not to copy [secret_recovery_key]
    // This explicit declaration ensures that we use the following reference-to-reference conversion:
    // https://docs.rs/generic-array/0.14.4/src/generic_array/lib.rs.html#553-563
    let hmac_key: &GenericArray<_, _> = (secret_recovery_key.0[..]).into();

    let seed = Hmac::<Sha256>::new(hmac_key)
        .chain(protocol_tag.to_be_bytes())
        .chain(tag.to_be_bytes())
        .chain(session_nonce)
        .finalize()
        .into_bytes()
        .into();

    Ok(ChaCha20Rng::from_seed(seed))
}

/// Initialize a RNG by hashing the arguments.
/// Intended for use generating an ephemeral scalar for ECDSA signatures in the spirit of RFC 6979,
/// except this implementation does not conform to RFC 6979.
/// Compare with RustCrypto: <https://github.com/RustCrypto/signatures/blob/54925be85d4eeb0540bf7c687ab08152a858871a/ecdsa/src/rfc6979.rs#L16-L40>
pub(crate) fn rng_seed_ecdsa_ephemeral_scalar_with_party_id<K>(
    tag: u8,
    party_id: TypedUsize<K>,
    signing_key: &k256::Scalar,
    msg_to_sign: &k256::Scalar,
) -> TofnResult<impl CryptoRng + RngCore> {
    let mut signing_key_bytes = signing_key.to_bytes();
    let msg_to_sign_bytes = msg_to_sign.to_bytes();

    // TODO: Use protocol domain separation: https://github.com/axelarnetwork/tofn/issues/184
    let seed = Hmac::<Sha256>::new(&Default::default())
        .chain(tag.to_be_bytes())
        .chain(party_id.to_bytes())
        .chain(signing_key_bytes)
        .chain(msg_to_sign_bytes)
        .finalize()
        .into_bytes()
        .into();

    signing_key_bytes.zeroize();

    Ok(ChaCha20Rng::from_seed(seed))
}

/// Initialize a RNG by hashing the arguments.
/// Intended for use generating an ephemeral scalar for ECDSA signatures in the spirit of RFC 6979,
/// except this implementation does not conform to RFC 6979.
/// Compare with RustCrypto: <https://github.com/RustCrypto/signatures/blob/54925be85d4eeb0540bf7c687ab08152a858871a/ecdsa/src/rfc6979.rs#L16-L40>
pub(crate) fn rng_seed_ecdsa_ephemeral_scalar(
    protocol_tag: u8,
    tag: u8,
    signing_key: &k256::Scalar,
    message_digest: &k256::Scalar,
) -> TofnResult<impl CryptoRng + RngCore> {
    let mut signing_key_bytes = signing_key.to_bytes();
    let msg_to_sign_bytes = message_digest.to_bytes();

    let seed = Hmac::<Sha256>::new(&Default::default())
        .chain(protocol_tag.to_be_bytes())
        .chain(tag.to_be_bytes())
        .chain(signing_key_bytes)
        .chain(msg_to_sign_bytes)
        .finalize()
        .into_bytes()
        .into();

    signing_key_bytes.zeroize();

    Ok(ChaCha20Rng::from_seed(seed))
}

#[cfg(test)]
/// return the all-zero array with the first bytes set to the bytes of `index`
pub fn dummy_secret_recovery_key(index: usize) -> SecretRecoveryKey {
    let index_bytes = index.to_be_bytes();
    let mut result = [0; 64];
    for (i, &b) in index_bytes.iter().enumerate() {
        result[i] = b;
    }
    SecretRecoveryKey(result)
}
