#![allow(unused)]

use aes::cipher::{
    block_padding::Pkcs7, BlockModeDecrypt, BlockModeEncrypt, KeyInit,
    KeyIvInit,
};
use base64::prelude::*;
use hkdf::Hkdf;
use hmac::Mac;
use rand::Rng;
use sha2::Sha256;

// 1. 类型别名: 纯原地计算模式
type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
type HmacSha256 = hmac::Hmac<Sha256>;

// 2. 协议物理尺寸常量 (单位: 字节)
pub const UUID_LEN: usize = 36;
pub const IV_LEN: usize = 16;
pub const HMAC_LEN: usize = 32;
pub const MIN_ENCRYPTED_PAYLOAD_LEN: usize = IV_LEN + 16 + HMAC_LEN; // 64 字节最小有效密文载荷

// 3. 缓冲区物理预算常量 (用于上层流式切片与内存规划)
pub const MAX_RAW_LEN: usize = 3072;          // 二进制包上限 (3072 * 4/3 = 4096)
pub const MAX_PLAINTEXT_LEN: usize = 2960;    // 单切片最大明文容量 (扣除 100B 协议头尾与 PKCS7 补齐(uudi36+iv16+hmac32)+pkcs7 16)
pub const MAX_B64_LEN: usize = 4096;          // 对应的 Base64 编码缓冲区推荐大小

/// 密码学底层错误代码
///
/// 1. `#[repr(u8)]`: 严格在内存中锁定为 1 字节标量。
/// 2. `Copy, Clone`: 标量按值在 CPU 寄存器中传递，零堆分配、绝无所有权转移。
/// 3. 彻底抹除明文字符串: 生产构建下无 `Debug`、无 `Display`、无 `Error` 特征，切断格式化机制。
#[repr(u8)]
#[derive(PartialEq, Eq, Clone, Copy)]
#[cfg_attr(any(debug_assertions, test), derive(Debug))] // Release 构建下彻底消失
pub enum CryptoError {
    Base64DecodeError = 1,
    PayloadTooShort = 2,
    InvalidKeyLength = 3,
    HmacVerificationFailed = 4,
    DecryptionFailed = 5,
    BufferTooSmall = 6,
}

// 如果平时单测或调试需要用 {:?} 打印纯数字,使用时直接let num = err as u8; // 干净利落，直接拿到数字
impl CryptoError {
    /// 零开销获取底层纯数字代号 (寄存器直读)
    #[inline(always)]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// 硬件级 Volatile 内存擦除 (阻止 LLVM 优化器作为 Dead Store 剔除)
#[inline(always)]
pub fn zeroize_slice(buf: &mut [u8]) {
    for byte in buf.iter_mut() {
        unsafe {
            core::ptr::write_volatile(byte, 0);
        }
    }
}

/// 会话级密码学上下文管理器
///
/// 在 Agent 启动建立会话时派生一次子密钥并长期复用，避免每次发包重复运行 6~8 次 HMAC-SHA256。
pub struct CryptoContext {
    enc_key: [u8; 32],
    mac_key: [u8; 32],
}

impl CryptoContext {
    /// 初始化并由主密钥派生子密钥
    pub fn new(master_key: &[u8]) -> Result<Self, CryptoError> {
        if master_key.len() != 32 {
            return Err(CryptoError::InvalidKeyLength);
        }

        let hk = Hkdf::<Sha256>::new(None, master_key);
        let mut enc_key = [0u8; 32];
        let mut mac_key = [0u8; 32];

        let _ = hk.expand([0x3A, 0xF9, 0x81, 0x5C, 0x22, 0xD4, 0x6E,0x01], &mut enc_key);
        let _ = hk.expand([0x7B, 0xC4, 0x19, 0x2E, 0x88, 0xFA, 0x43,0x02], &mut mac_key);

        Ok(Self { enc_key, mac_key })
    }

    /// 纯切片就地封包加密 (Caller-provided Buffer 模式)
    ///
    /// * `data`: 待发送明文切片 (不得超过 MAX_PLAINTEXT_LEN, 即 2960 字节)
    /// * `uuid`: 36 字节 Agent 唯一标识符
    /// * `raw_buf`: 调用者提供的二进制暂存缓冲区 (容量不得小于所需二进制包总长，建议 >= MAX_RAW_LEN)
    /// * `out_b64`: 调用者提供的 Base64 输出缓冲区 (用于网络发送，建议 >= MAX_B64_LEN)
    /// * 返回: 写入 out_b64 的实际 Base64 字节长度
    pub fn seal_message(
        &self,
        data: &[u8],
        uuid: &str,
        raw_buf: &mut [u8],
        out_b64: &mut [u8],
    ) -> Result<usize, CryptoError> {
        if uuid.len() != UUID_LEN {
            return Err(CryptoError::InvalidKeyLength);
        }
        if data.len() > MAX_PLAINTEXT_LEN {
            return Err(CryptoError::BufferTooSmall);
        }

        let padded_len = (data.len() / 16 + 1) * 16;
        let raw_total = UUID_LEN + IV_LEN + padded_len + HMAC_LEN;

        if raw_buf.len() < raw_total {
            return Err(CryptoError::BufferTooSmall);
        }

        // 1. 生成 16 字节纯真随机 IV
        let mut iv = [0u8; IV_LEN];
        rand::rng().fill_bytes(&mut iv);

        // 2. 在调用者提供的 raw_buf 中就地布局明文数据
        let uuid_start = 0;
        let iv_start = UUID_LEN;
        let ct_start = iv_start + IV_LEN;
        let ct_end = ct_start + padded_len;
        let hmac_start = ct_end;

        raw_buf[uuid_start..iv_start].copy_from_slice(uuid.as_bytes());
        raw_buf[iv_start..ct_start].copy_from_slice(&iv);
        raw_buf[ct_start..ct_start + data.len()].copy_from_slice(data);

        // 3. AES-256-CBC 原地加密
        let encryptor = Aes256CbcEnc::new_from_slices(&self.enc_key, &iv)
            .map_err(|_| CryptoError::InvalidKeyLength)?;

        if encryptor
            .encrypt_padded::<Pkcs7>(&mut raw_buf[ct_start..ct_end], data.len())
            .is_err()
        {
            zeroize_slice(raw_buf);
            zeroize_slice(&mut iv);
            return Err(CryptoError::DecryptionFailed);
        }

        // 4. 计算 HMAC-SHA256(IV + Ciphertext)
        let mut mac = HmacSha256::new_from_slice(&self.mac_key)
            .map_err(|_| CryptoError::InvalidKeyLength)?;
        mac.update(&raw_buf[iv_start..ct_end]);
        let tag = mac.finalize().into_bytes();
        raw_buf[hmac_start..raw_total].copy_from_slice(&tag);

        // 5. 编码至调用者的 out_b64 输出缓冲区
        let b64_len = BASE64_STANDARD
            .encode_slice(&raw_buf[..raw_total], out_b64)
            .map_err(|_| {
                zeroize_slice(raw_buf);
                CryptoError::BufferTooSmall
            })?;

        // 6. 物理擦除二进制暂存区与 IV 栈变量
        zeroize_slice(raw_buf);
        zeroize_slice(&mut iv);

        Ok(b64_len)
    }

    /// 纯切片单缓冲区原地解密 (In-place Decryption 模式):服务端发给客户端的指令数据,在客户端解密
    ///
    /// * `payload_b64`: 服务端返回给客户端的 Base64 字节切片
    /// * `raw_buf`: 调用者提供的单个工作缓冲区 (建议 >= MAX_RAW_LEN)。
    ///   解码、验真、解密均在此缓冲区内部就地完成，解密出的明文直接存放于 raw_buf[..plaintext_len]
    /// * 返回: 解密出的明文有效字节长度
    pub fn open_message(
        &self,
        payload_b64: &[u8],
        raw_buf: &mut [u8],
    ) -> Result<usize, CryptoError> {
        // 1. 就地 Base64 解码至 raw_buf
        let raw_len = BASE64_STANDARD
            .decode_slice(payload_b64, raw_buf)
            .map_err(|_| CryptoError::Base64DecodeError)?;

        // 2. 最小物理长度检验
        if raw_len < UUID_LEN + MIN_ENCRYPTED_PAYLOAD_LEN {
            zeroize_slice(&mut raw_buf[..raw_len]);
            return Err(CryptoError::PayloadTooShort);
        }

        let iv_start = UUID_LEN;
        let ct_start = iv_start + IV_LEN;
        let hmac_start = raw_len - HMAC_LEN;

        // 3. 提取 IV
        let mut iv = [0u8; IV_LEN];
        iv.copy_from_slice(&raw_buf[iv_start..ct_start]);

        // 4. 恒定时间验证 HMAC-SHA256(IV + Ciphertext)
        let mut mac = HmacSha256::new_from_slice(&self.mac_key)
            .map_err(|_| CryptoError::InvalidKeyLength)?;
        mac.update(&raw_buf[iv_start..hmac_start]);

        if mac.verify_slice(&raw_buf[hmac_start..raw_len]).is_err() {
            zeroize_slice(&mut raw_buf[..raw_len]);
            zeroize_slice(&mut iv);
            return Err(CryptoError::HmacVerificationFailed);
        }

        // 5. AES-256-CBC 原地解密
        let decryptor = Aes256CbcDec::new_from_slices(&self.enc_key, &iv)
            .map_err(|_| CryptoError::InvalidKeyLength)?;

        let plaintext_len = match decryptor.decrypt_padded::<Pkcs7>(&mut raw_buf[ct_start..hmac_start]) {
            Ok(p) => p.len(),
            Err(_) => {
                zeroize_slice(&mut raw_buf[..raw_len]);
                zeroize_slice(&mut iv);
                return Err(CryptoError::DecryptionFailed);
            }
        };

        // 6. 原地将明文搬移到缓冲区最前端，并洗白多余尾部内存
        raw_buf.copy_within(ct_start..ct_start + plaintext_len, 0);
        zeroize_slice(&mut raw_buf[plaintext_len..raw_len]);
        zeroize_slice(&mut iv);

        Ok(plaintext_len)
    }
}

// 离开作用域时自动物理清空会话密钥
impl Drop for CryptoContext {
    fn drop(&mut self) {
        zeroize_slice(&mut self.enc_key);
        zeroize_slice(&mut self.mac_key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mythic_crypto_caller_buffer_roundtrip() {
        let master_key = b"01234567890123456789012345678901"; // 32 字节主密钥
        let uuid = "12345678-1234-1234-1234-1234567890ab"; // 36 字节 UUID
        let original_msg = br#"{"action":"checkin","os":"windows 11", "user":"SYSTEM"}"#;

        let ctx = CryptoContext::new(master_key).expect("Key init failed");

        // 调用者外包缓冲区
        let mut raw_buf = [0u8; MAX_RAW_LEN];
        let mut b64_buf = [0u8; MAX_B64_LEN];

        // 1. 加密封包
        let b64_len = ctx
            .seal_message(original_msg, uuid, &mut raw_buf, &mut b64_buf)
            .expect("Seal failed");
        assert!(b64_len > 0);

        // 2. 解密封包 (复用 raw_buf，原地解密)
        let pt_len = ctx
            .open_message(&b64_buf[..b64_len], &mut raw_buf)
            .expect("Open failed");

        assert_eq!(&raw_buf[..pt_len], original_msg);
    }

    #[test]
    fn test_tampered_hmac_fails() {
        let master_key = b"01234567890123456789012345678901";
        let uuid = "12345678-1234-1234-1234-1234567890ab";
        let original_msg = b"secret_task_data";

        let ctx = CryptoContext::new(master_key).unwrap();
        let mut raw_buf = [0u8; MAX_RAW_LEN];
        let mut b64_buf = [0u8; MAX_B64_LEN];

        let b64_len = ctx
            .seal_message(original_msg, uuid, &mut raw_buf, &mut b64_buf)
            .unwrap();

        // 篡改 Base64 字节
        b64_buf[50] ^= 0xFF;

        let res = ctx.open_message(&b64_buf[..b64_len], &mut raw_buf);
        assert!(res.is_err());
    }
}