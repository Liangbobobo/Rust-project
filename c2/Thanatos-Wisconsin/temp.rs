use aes::cipher::{
    block_padding::Pkcs7, BlockModeDecrypt, BlockModeEncrypt, KeyInit, KeyIvInit,
};
use base64::prelude::*;
use hmac::Mac;
use rand::Rng;
use sha2::Sha256;

type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
type HmacSha256 = hmac::Hmac<Sha256>;

const UUID_LEN: usize = 36;
const IV_LEN: usize = 16;
const HMAC_LEN: usize = 32;
const MIN_ENCRYPTED_PAYLOAD_LEN: usize = IV_LEN + 16 + HMAC_LEN;

#[derive(Debug, PartialEq, Eq)]
pub enum CryptoError {
    Base64DecodeError,
    PayloadTooShort,
    InvalidKeyLength,
    HmacVerificationFailed,
    DecryptionFailed,
    Utf8Error,
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for CryptoError {}

/// 将数据封装为 Mythic 要求的格式并加密:
/// 结构: Base64( UUID(36B) + IV(16B) + Ciphertext + HMAC(32B) )
///
/// * `data`: 待发送的原始明文字节 (如 JSON 字符串)
/// * `key`: 32 字节 AES/HMAC 共享密钥
/// * `uuid`: 36 字节的 Agent 唯一标识符
pub fn seal_message(data: &[u8], key: &[u8], uuid: &str) -> Result<String, CryptoError> {
    if key.len() != 32 {
        return Err(CryptoError::InvalidKeyLength);
    }

    // 1. 生成 16 字节随机 IV (rand 0.10 标准写法)
    let mut iv = [0u8; IV_LEN];
    rand::rng().fill_bytes(&mut iv);

    // 2. AES-256-CBC 原地加密 (PKCS#7 填充)
    let encryptor = Aes256CbcEnc::new_from_slices(key, &iv)
        .map_err(|_| CryptoError::InvalidKeyLength)?;

    let padded_len = (data.len() / 16 + 1) * 16;
    let mut buf = vec![0u8; padded_len];
    buf[..data.len()].copy_from_slice(data);

    let ciphertext = encryptor
        .encrypt_padded::<Pkcs7>(&mut buf, data.len())
        .map_err(|_| CryptoError::DecryptionFailed)?;

    // 3. 计算 HMAC-SHA256(IV + Ciphertext)
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|_| CryptoError::InvalidKeyLength)?;
    mac.update(&iv);
    mac.update(ciphertext);
    let tag = mac.finalize().into_bytes();

    // 4. 组装最终二进制流: UUID(36) + IV(16) + Ciphertext + HMAC(32)
    let mut packet = Vec::with_capacity(UUID_LEN + IV_LEN + ciphertext.len() + HMAC_LEN);
    packet.extend_from_slice(uuid.as_bytes());
    packet.extend_from_slice(&iv);
    packet.extend_from_slice(ciphertext);
    packet.extend_from_slice(&tag);

    // 5. 整体 Base64 编码
    Ok(BASE64_STANDARD.encode(packet))
}

/// 解析服务端响应并解密还原明文:
/// 服务端返回结构: Base64( UUID(36B) + IV(16B) + Ciphertext + HMAC(32B) )
///
/// * `payload_b64`: 服务端返回的 Base64 字符串
/// * `key`: 32 字节 AES/HMAC 共享密钥
pub fn open_message(payload_b64: &str, key: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if key.len() != 32 {
        return Err(CryptoError::InvalidKeyLength);
    }

    // 1. Base64 解码
    let raw = BASE64_STANDARD
        .decode(payload_b64.trim())
        .map_err(|_| CryptoError::Base64DecodeError)?;

    // 2. 检查最小长度限制 (必须包含 36字节UUID + IV + 至少1个密文块 + HMAC)
    if raw.len() < UUID_LEN + MIN_ENCRYPTED_PAYLOAD_LEN {
        return Err(CryptoError::PayloadTooShort);
    }

    // 3. 切除头部的 36 字节 UUID
    let enc_part = &raw[UUID_LEN..];

    // 4. 拆解各字段切片
    let total_len = enc_part.len();
    let iv = &enc_part[..IV_LEN];
    let ciphertext = &enc_part[IV_LEN..total_len - HMAC_LEN];
    let expected_tag = &enc_part[total_len - HMAC_LEN..];

    // 5. 校验 HMAC-SHA256 (重要防护: 防止畸形包导致崩溃)
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|_| CryptoError::InvalidKeyLength)?;
    mac.update(iv);
    mac.update(ciphertext);
    mac.verify_slice(expected_tag)
        .map_err(|_| CryptoError::HmacVerificationFailed)?;

    // 6. AES-256-CBC 原地解密
    let decryptor = Aes256CbcDec::new_from_slices(key, iv)
        .map_err(|_| CryptoError::InvalidKeyLength)?;

    let mut buf = ciphertext.to_vec();
    let plaintext = decryptor
        .decrypt_padded::<Pkcs7>(&mut buf)
        .map_err(|_| CryptoError::DecryptionFailed)?;

    Ok(plaintext.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mythic_crypto_roundtrip() {
        let key = b"01234567890123456789012345678901"; // 32 字节密钥
        let uuid = "12345678-1234-1234-1234-1234567890ab"; // 36 字节 UUID
        let original_msg = br#"{"action":"checkin","user":"SYSTEM"}"#;

        // 加密
        let sealed = seal_message(original_msg, key, uuid).expect("Seal failed");
        assert!(!sealed.is_empty());

        // 解密
        let opened = open_message(&sealed, key).expect("Open failed");
        assert_eq!(opened, original_msg);
    }

    #[test]
    fn test_tampered_payload_hmac_fails() {
        let key = b"01234567890123456789012345678901";
        let uuid = "12345678-1234-1234-1234-1234567890ab";
        let original_msg = b"secret_task_data";

        let sealed = seal_message(original_msg, key, uuid).unwrap();
        let mut raw = BASE64_STANDARD.decode(&sealed).unwrap();

        // 篡改密文区的一个字节
        let tamper_idx = UUID_LEN + IV_LEN + 2;
        raw[tamper_idx] ^= 0xFF;

        let tampered_b64 = BASE64_STANDARD.encode(&raw);
        let res = open_message(&tampered_b64, key);

        // 必须被 HMAC 拦截
        assert_eq!(res, Err(CryptoError::HmacVerificationFailed));
    }
}