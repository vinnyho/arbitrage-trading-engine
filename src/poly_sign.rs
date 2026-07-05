use base64::Engine;
use base64::engine::general_purpose::URL_SAFE;
use hmac::{Hmac, Mac};
use k256::ecdsa::signature::hazmat::PrehashSigner;
use k256::ecdsa::{RecoveryId, Signature, SigningKey};
use sha2::Sha256;
use tiny_keccak::{Hasher, Keccak};

const DOMAIN_TYPE: &[u8] =
    b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";
const ORDER_TYPE_STR: &str = "Order(uint256 salt,address maker,address signer,uint256 tokenId,uint256 makerAmount,uint256 takerAmount,uint8 side,uint8 signatureType,uint256 timestamp,bytes32 metadata,bytes32 builder)";
const SOLADY_TYPE_STR: &str = "TypedDataSign(Order contents,string name,string version,uint256 chainId,address verifyingContract,bytes32 salt)Order(uint256 salt,address maker,address signer,uint256 tokenId,uint256 makerAmount,uint256 takerAmount,uint8 side,uint8 signatureType,uint256 timestamp,bytes32 metadata,bytes32 builder)";
const EXCHANGE_ADDR: &str = "0xE111180000d2663C0091e4f400237545B87B996B";

pub const BUILDER_HEX: &str = "0000000000000000000000000000000000000000000000000000000000000000";
pub const SIGNATURE_TYPE_POLY_1271: u8 = 3;

fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut h = Keccak::v256();
    h.update(data);
    let mut out = [0u8; 32];
    h.finalize(&mut out);
    out
}

fn enc_u64(v: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&v.to_be_bytes());
    out
}

fn enc_addr(hex_addr: &str) -> Result<[u8; 32], anyhow::Error> {
    let bytes = hex::decode(hex_addr.trim_start_matches("0x"))?;
    anyhow::ensure!(bytes.len() == 20, "address must be 20 bytes");
    let mut out = [0u8; 32];
    out[12..].copy_from_slice(&bytes);
    Ok(out)
}

fn enc_decimal_u256(decimal: &str) -> Result<[u8; 32], anyhow::Error> {
    let mut out = [0u8; 32];
    for c in decimal.chars() {
        let digit = c
            .to_digit(10)
            .ok_or_else(|| anyhow::anyhow!("invalid decimal digit"))? as u16;
        let mut carry = digit;
        for byte in out.iter_mut().rev() {
            let val = (*byte as u16) * 10 + carry;
            *byte = val as u8;
            carry = val >> 8;
        }
    }
    Ok(out)
}

fn enc_bytes32(hex_str: &str) -> Result<[u8; 32], anyhow::Error> {
    let bytes = hex::decode(hex_str.trim_start_matches("0x"))?;
    anyhow::ensure!(bytes.len() == 32, "bytes32 must be exactly 32 bytes");
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn domain_separator() -> Result<[u8; 32], anyhow::Error> {
    let mut data = Vec::with_capacity(5 * 32);
    data.extend_from_slice(&keccak256(DOMAIN_TYPE));
    data.extend_from_slice(&keccak256(b"Polymarket CTF Exchange"));
    data.extend_from_slice(&keccak256(b"2"));
    data.extend_from_slice(&enc_u64(137));
    data.extend_from_slice(&enc_addr(EXCHANGE_ADDR)?);
    Ok(keccak256(&data))
}

pub fn sign_order(
    private_key_hex: &str,
    funder_addr: &str,
    token_id: &str,
    maker_amount: u64,
    taker_amount: u64,
    timestamp_ms: u64,
) -> Result<(String, u64), anyhow::Error> {
    let salt: u64 = (rand::random::<u32>()) as u64;
    let funder = enc_addr(funder_addr)?;
    let builder = enc_bytes32(BUILDER_HEX)?;

    let mut data = Vec::with_capacity(12 * 32);
    data.extend_from_slice(&keccak256(ORDER_TYPE_STR.as_bytes()));
    data.extend_from_slice(&enc_u64(salt));
    data.extend_from_slice(&funder);
    data.extend_from_slice(&funder);
    data.extend_from_slice(&enc_decimal_u256(token_id)?);
    data.extend_from_slice(&enc_u64(maker_amount));
    data.extend_from_slice(&enc_u64(taker_amount));
    data.extend_from_slice(&enc_u64(0));
    data.extend_from_slice(&enc_u64(SIGNATURE_TYPE_POLY_1271 as u64));
    data.extend_from_slice(&enc_u64(timestamp_ms));
    data.extend_from_slice(&[0u8; 32]);
    data.extend_from_slice(&builder);
    let contents_hash = keccak256(&data);

    let mut solady = Vec::with_capacity(7 * 32);
    solady.extend_from_slice(&keccak256(SOLADY_TYPE_STR.as_bytes()));
    solady.extend_from_slice(&contents_hash);
    solady.extend_from_slice(&keccak256(b"DepositWallet"));
    solady.extend_from_slice(&keccak256(b"1"));
    solady.extend_from_slice(&enc_u64(137));
    solady.extend_from_slice(&funder);
    solady.extend_from_slice(&[0u8; 32]);
    let solady_hash = keccak256(&solady);

    let app_domain = domain_separator()?;
    let mut digest_input = [0u8; 66];
    digest_input[0] = 0x19;
    digest_input[1] = 0x01;
    digest_input[2..34].copy_from_slice(&app_domain);
    digest_input[34..].copy_from_slice(&solady_hash);
    let hash = keccak256(&digest_input);

    let key_bytes = hex::decode(private_key_hex.trim_start_matches("0x"))?;
    let signing_key = SigningKey::from_slice(&key_bytes)?;
    let (sig, rec_id): (Signature, RecoveryId) = signing_key.sign_prehash(&hash)?;

    let r = sig.r().to_bytes();
    let s = sig.s().to_bytes();
    let v = rec_id.to_byte() + 27u8;

    let mut sig_bytes = [0u8; 65];
    sig_bytes[..32].copy_from_slice(&r);
    sig_bytes[32..64].copy_from_slice(&s);
    sig_bytes[64] = v;

    let mut signature = hex::encode(sig_bytes);
    signature.push_str(&hex::encode(app_domain));
    signature.push_str(&hex::encode(contents_hash));
    signature.push_str(&hex::encode(ORDER_TYPE_STR.as_bytes()));
    signature.push_str(&format!("{:04x}", ORDER_TYPE_STR.len()));

    Ok((format!("0x{}", signature), salt))
}

pub fn l2_signature(
    secret_b64: &str,
    timestamp: &str,
    method: &str,
    path: &str,
    body: &str,
) -> Result<String, anyhow::Error> {
    let secret = URL_SAFE.decode(secret_b64)?;
    let message = format!("{}{}{}{}", timestamp, method, path, body);
    let mut mac = Hmac::<Sha256>::new_from_slice(&secret)?;
    mac.update(message.as_bytes());
    Ok(URL_SAFE.encode(mac.finalize().into_bytes()))
}
