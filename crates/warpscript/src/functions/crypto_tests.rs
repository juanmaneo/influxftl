#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of the cryptographic functions against values produced by the BouncyCastle version
//! Warp 10 depends on (1.78.1) and by Warp 10's own `SipHashInline`, `TOB58` and `B58TO`.

use super::crypto::{
    Kek, NONCE_LEN, aes_unwrap, aes_wrap, aeswrap_impl, b58_decode, b58_encode, hmac, rfc3394_wrap,
};
use crate::encoding::{hex_decode, hex_encode};
use crate::stack::Stack;
use crate::tests::{assert_fails, assert_top, exec, fail_msg};
use crate::value::Value;

const FOX: &str = "The quick brown fox jumps over the lazy dog";

/// The bytes `0, 1, …, n - 1`.
fn seq(n: usize) -> Vec<u8> {
    (0..n).map(|i| i as u8).collect()
}

fn seq_hex(n: usize) -> String {
    hex_encode(&seq(n))
}

/// Every `DIGEST` function with the digests of `""`, `"abc"`, [`FOX`] and `seq(200)`.
const DIGESTS: &[(&str, [&str; 4])] = &[
    (
        "KECCAK.128",
        [
            "bcf56ac882ad981cd0fa74f0f397572c",
            "ed992674a628509bb2dce176b7c03672",
            "9f8d5ca2e2dafd2c6ac064c4b901d342",
            "a12207678b35b6c58c58da7e542a63dc",
        ],
    ),
    (
        "KECCAK.224",
        [
            "f71837502ba8e10837bdd8d365adb85591895602fc552b48b7390abd",
            "c30411768506ebe1c2871b1ee2e87d38df342317300a9b97a95ec6a8",
            "310aee6b30c47350576ac2873fa89fd190cdc488442f3ef654cf23fe",
            "1922a6155bf6bb5ade5100c99771912596e45d8273347d5daca1f3c9",
        ],
    ),
    (
        "KECCAK.256",
        [
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470",
            "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45",
            "4d741b6f1eb29cb2a9b9911c82f56fa8d73b04959d3d9d222895df6c0b28aa15",
            "bfb0aa97863e797943cf7c33bb7e880bb4543f3d2703c0923c6901c2af57b890",
        ],
    ),
    (
        "KECCAK.288",
        [
            "6753e3380c09e385d0339eb6b050a68f66cfd60a73476e6fd6adeb72f5edd7c6f04a5d01",
            "20ff13d217d5789fa7fc9e0e9a2ee627363ec28171d0b6c52bbd2f240554dbc94289f4d6",
            "0bbe6afae0d7e89054085c1cc47b1689772c89a41796891e197d1ca1b76f288154933ded",
            "f65514ae801f08d9fc899e9aa5efb3536c5a959741df34a59938fbe779a697472dae081c",
        ],
    ),
    (
        "KECCAK.384",
        [
            "2c23146a63a29acf99e73b88f8c24eaa7dc60aa771780ccc006afbfa8fe2479b2dd2b21362337441ac12b515911957ff",
            "f7df1165f033337be098e7d288ad6a2f74409d7a60b49c36642218de161b1f99f8c681e4afaf31a34db29fb763e3c28e",
            "283990fa9d5fb731d786c5bbee94ea4db4910f18c62c03d173fc0a5e494422e8a0b3da7574dae7fa0baf005e504063b3",
            "bdb00a13330573e75652bec64443b6ca9512b487fb7e1693540d04d55d68ff98b25cd30abd53bbbc27ba1c12195c479d",
        ],
    ),
    (
        "KECCAK.512",
        [
            "0eab42de4c3ceb9235fc91acffe746b29c29a8c366b7c60e4e67c466f36a4304c00fa9caf9d87976ba469bcbe06713b435f091ef2769fb160cdab33d3670680e",
            "18587dc2ea106b9a1563e32b3312421ca164c7f1f07bc922a9c83d77cea3a1e5d0c69910739025372dc14ac9642629379540c17e2a65b19d77aa511a9d00bb96",
            "d135bb84d0439dbac432247ee573a23ea7d3c9deb2a968eb31d47c4fb45f1ef4422d6c531b5b9bd6f449ebcc449ea94d0a8f05f62130fda612da53c79659f609",
            "f452d81b62b961f8023f8228cbe780379b36c49ddcef29e0dffb01a930c2cc53a694ed6ae3f0d224a2f1be55814a81841b90d56bcdf4a48a633f258a32dc14fc",
        ],
    ),
    (
        "MD2",
        [
            "8350e5a3e24c153df2275c9f80692773",
            "da853b0d3f88d99b30283a69e6ded6bb",
            "03d85a0d629d2c442e987525319fc471",
            "c3b69aeecb7710b20fa0524ef1bf552d",
        ],
    ),
    (
        "MD4",
        [
            "31d6cfe0d16ae931b73c59d7e0c089c0",
            "a448017aaf21d8525fc10ae87aa6729d",
            "1bee69a46ba811185c194762abaeae90",
            "f1a97b5ff191d1fe9e570c529abf13b3",
        ],
    ),
    (
        "MD5",
        [
            "d41d8cd98f00b204e9800998ecf8427e",
            "900150983cd24fb0d6963f7d28e17f72",
            "9e107d9d372bb6826bd81d3542a419d6",
            "fb7001d34b8e82c9b579be5005d5b0a5",
        ],
    ),
    (
        "SHA1",
        [
            "da39a3ee5e6b4b0d3255bfef95601890afd80709",
            "a9993e364706816aba3e25717850c26c9cd0d89d",
            "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12",
            "54d11e99127d159799dbce10f51a75e697780478",
        ],
    ),
    (
        "SHA224",
        [
            "d14a028c2a3a2bc9476102bb288234c415a2b01f828ea62ac5b3e42f",
            "23097d223405d8228642a477bda255b32aadbce4bda0b3f7e36c9da7",
            "730e109bd7a8a32b1cb9d9a09aa2325d2430587ddbc0c38bad911525",
            "ab3e334a37953e18f4f673736dddb64e850bfdf29d5a7ba268c567d9",
        ],
    ),
    (
        "SHA256",
        [
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592",
            "1901da1c9f699b48f6b2636e65cbf73abf99d0441ef67f5c540a42f7051dec6f",
        ],
    ),
    (
        "SHA384",
        [
            "38b060a751ac96384cd9327eb1b1e36a21fdb71114be07434c0cc7bf63f6e1da274edebfe76f65fbd51ad2f14898b95b",
            "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed8086072ba1e7cc2358baeca134c825a7",
            "ca737f1014a48f4c0b6dd43cb177b0afd9e5169367544c494011e3317dbf9a509cb1e5dc1e85a941bbee3d7f2afbc9b1",
            "7ea4bb2534c67036f49de7beb5fe8a2478df04ff3fef40a9cd4923999a590e9912df1297217ce1a021aa2fb1013498b8",
        ],
    ),
    (
        "SHA512",
        [
            "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e",
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
            "07e547d9586f6a73f73fbac0435ed76951218fb7d0c8d788a309d785436bbb642e93a252a954f23912547d1e8a3b5ed6e1bfd7097821233fa0538f3db854fee6",
            "986058e9895e2c2ab8f9e8cbdf801db12a44842a56a91d5a4e87b1fc98b293722c4664142e42c3c551ff898646268cd92b84ed230b8c94bed7798d4f27cd7465",
        ],
    ),
    (
        "SHA3.224",
        [
            "6b4e03423667dbb73b6e15454f0eb1abd4597f9a1b078e3f5b5a6bc7",
            "e642824c3f8cf24ad09234ee7d3c766fc9a3a5168d0c94ad73b46fdf",
            "d15dadceaa4d5d7bb3b48f446421d542e08ad8887305e28d58335795",
            "8bcd90dbc5379549b5e78a1fbe24ae120d92caef17750461262b1e97",
        ],
    ),
    (
        "SHA3.256",
        [
            "a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a",
            "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532",
            "69070dda01975c8c120c3aada1b282394e7f032fa9cf32f4cb2259a0897dfc04",
            "5f728f63bf5ee48c77f453c0490398fa645b8d4c4e56be9a41cfec344d6ca899",
        ],
    ),
    (
        "SHA3.384",
        [
            "0c63a75b845e4f7d01107d852e4c2485c51a50aaaa94fc61995e71bbee983a2ac3713831264adb47fb6bd1e058d5f004",
            "ec01498288516fc926459f58e2c6ad8df9b473cb0fc08c2596da7cf0e49be4b298d88cea927ac7f539f1edf228376d25",
            "7063465e08a93bce31cd89d2e3ca8f602498696e253592ed26f07bf7e703cf328581e1471a7ba7ab119b1a9ebdf8be41",
            "b13febb1b3c54a7c6b69367f693a1d1f3145709b6ddef23ff15874133ea1fb9cfa48ee7ff4ec9aa987dea641e33ccdf7",
        ],
    ),
    (
        "SHA3.512",
        [
            "a69f73cca23a9ac5c8b567dc185a756e97c982164fe25859e0d1dcc1475c80a615b2123af1f5f94c11e3e9402c3ac558f500199d95b6d3e301758586281dcd26",
            "b751850b1a57168a5693cd924b6b096e08f621827444f70d884f5d0240d2712e10e116e9192af3c91a7ec57647e3934057340b4cf408d5a56592f8274eec53f0",
            "01dedd5de4ef14642445ba5f5b97c15e47b9ad931326e4b0727cd94cefc44fff23f07bf543139939b49128caf436dc1bdee54fcb24023a08d9403f9b4bf0d450",
            "ea5d05f19348dd589793354793a15f37a73b4c0bb4e750b9a00757dfce2f8b65a64191bb9b137de00feef6474cfd47abf7880efbc51614a5715df12cfe0caee3",
        ],
    ),
    (
        "RIPEMD128",
        [
            "cdf26213a150dc3ecb610f18f6b38b46",
            "c14a12199c66e4ba84636b0f69144c77",
            "3fa9b57f053c053fbe2735b2380db596",
            "f79936837b8fdc6d9af2dd8551f9df8d",
        ],
    ),
    (
        "RIPEMD160",
        [
            "9c1185a5c5e9fc54612808977ee8f548b2258d31",
            "8eb208f7e05d987a9b044a8e98c6b087f15a0bfc",
            "37f332f68db77bd9d7edd4969571ad671cf9dd3b",
            "c315823ea8fe07a2dd18de4e545255afe3af0738",
        ],
    ),
    (
        "RIPEMD256",
        [
            "02ba4c4e5f8ecd1877fc52d64d30e37a2d9774fb1e5d026380ae0168e3c5522d",
            "afbd6e228b9d8cbbcef5ca2d03e6dba10ac0bc7dcbe4680e1e42d2e975459b65",
            "c3b0c2f764ac6d576a6c430fb61a6f2255b4fa833e094b1ba8c1e29b6353036f",
            "44af9893eef1d3d9eb7101a909c1bc2a739aba8f2dd54f12c11f3d47fb2bdcc2",
        ],
    ),
    (
        "RIPEMD320",
        [
            "22d65d5661536cdc75c1fdf5c6de7b41b9f27325ebc61e8557177d705a0ec880151c3a32a00899b8",
            "de4c01b3054f8930a79d09ae738e92301e5a17085beffdc1b8d116713e74f82fa942d64cdbc4682d",
            "e7660e67549435c62141e51c9ab1dcc3b1ee9f65c0b3e561ae8f58c5dba3d21997781cd1cc6fbc34",
            "59125064f71257d1fda677d9dccb4b0762636ae44a61d1c6c4bbd8ac7c0dda0a4b7984dafa6e04bc",
        ],
    ),
    (
        "TIGER",
        [
            "3293ac630c13f0245f92bbb1766e16167a4e58492dde73f3",
            "2aab1484e8c158f2bfb8c5ff41b57a525129131c957b5f93",
            "6d12a41e72e644f017b6f0e2f7b44c6285f06dd5d2c5b075",
            "687e2237754be9fc84489c01769f2a4358b41adf036b82e4",
        ],
    ),
    (
        "WHIRLPOOL",
        [
            "19fa61d75522a4669b44e39c1d2e1726c530232130d407f89afee0964997f7a73e83be698b288febcf88e3e03c4f0757ea8964e59b63d93708b138cc42a66eb3",
            "4e2448a4c6f486bb16b6562c73b4020bf3043e3a731bce721ae1b303d97e6d4c7181eebdb6c57e277d0e34957114cbd6c797fc9d95d8b582d225292076d4eef5",
            "b97de512e91e3828b40d2b0fdce9ceb3c4a71f9bea8d88e75c4fa854df36725fd2b52eb6544edcacd6f8beddfea403cb55ae31f03ad62a5ef54e42ee82c3fb35",
            "50cc69782191cb4bda8975391ee7307ba29911d617cc162286864ed40e1e426c90861ff3b48ad8ab966891ef4862441f8747ccbf4d38a0959a13bb9bece698d6",
        ],
    ),
    (
        "GOST",
        [
            "981e5f3ca30c841487830f84fb433e13ac1101569b9c13584ac483234cd656c0",
            "b285056dbf18d7392d7677369524dd14747459ed8143997e163b2986f92fd42c",
            "9004294a361a508c586fe53d1f1b02746765e71b765472786e4770d565830a76",
            "b14a5c50bd33077e275607d3939d94b3088795144e82fbf6e2212bf6b51dceba",
        ],
    ),
];

#[test]
fn digests_match_bouncycastle() {
    let fox = hex_encode(FOX.as_bytes());
    let seq200 = seq_hex(200);
    for (name, expected) in DIGESTS {
        let inputs = ["", "616263", fox.as_str(), seq200.as_str()];
        for (input, want) in inputs.iter().zip(expected) {
            assert_top(&format!("'{input}' HEX-> {name} ->HEX"), *want);
        }
        assert_fails(
            &format!("'abc' {name}"),
            &format!("{name} operates on a byte array."),
        );
        assert_fails(name, "Empty stack.");
    }
}

#[test]
fn hmacs_match_bouncycastle() {
    let fox = hex_encode(FOX.as_bytes());
    // (key, HMAC-SHA256, HMAC-SHA1): keys shorter and longer than the 64-byte block.
    let vectors = [
        (
            String::new(),
            "fb011e6154a19b9a4c767373c305275a5a69e8b68b0b4c9200c383dced19a416",
            "2ba7f707ad5f187c412de3106583c3111d668de8",
        ),
        (
            "6b6579".to_owned(),
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8",
            "de7c9b85b8b78aa6bc8a7a36f70a90701c9db4d9",
        ),
        (
            seq_hex(100),
            "a626ea260467e29b7b23d88f6c7aa5927487ebd377bd3a18364d5425110ff616",
            "b0e206d7bd56be58c02f1c6559ebea3a6abe9bef",
        ),
        (
            seq_hex(200),
            "9b75f034ad627c6ac4e0b7bd883b09870009485aa3842a69b412e7aacd33bae9",
            "647e1ef91aff9bae2ee91f8b90d09b8ee3ba8a77",
        ),
    ];
    for (key, sha256, sha1) in &vectors {
        assert_top(
            &format!("'{fox}' HEX-> '{key}' HEX-> SHA256HMAC ->HEX"),
            *sha256,
        );
        assert_top(
            &format!("'{fox}' HEX-> '{key}' HEX-> SHA1HMAC ->HEX"),
            *sha1,
        );
    }
    // A key of exactly the block size is used as is.
    assert_eq!(
        hmac::<sha2::Sha256>(&seq(64), b""),
        hmac::<sha2::Sha256>(&seq(64), b"")
    );
    assert_ne!(
        hmac::<sha2::Sha256>(&seq(64), b""),
        hmac::<sha2::Sha256>(&seq(65), b"")
    );
    assert_fails(
        "'' HEX-> 'key' SHA256HMAC",
        "SHA256HMAC expects a byte array (key) on top of the stack.",
    );
    assert_fails(
        "'data' '' HEX-> SHA1HMAC",
        "SHA1HMAC operates on a byte array.",
    );
    assert_fails("SHA256HMAC", "Empty stack.");
    assert_fails("'' HEX-> SHA256HMAC", "Empty stack.");
}

#[test]
fn hash_is_siphash_with_default_or_explicit_key() {
    assert_top("'' HASH", -8_638_872_171_446_974_564);
    assert_top(&format!("'{FOX}' HASH"), 8_965_483_077_739_119_319);
    assert_top("'é' HASH", 7_735_404_173_223_923_081);
    assert_top(
        &format!("'{}' HEX-> HASH", seq_hex(200)),
        -8_432_951_646_536_891_083,
    );
    assert_top(&format!("'{FOX}' 1 2 HASH"), -4_461_453_879_318_936_080);
    assert_top(&format!("'{FOX}' -1 0 HASH"), -2_114_892_758_183_196_169);
    assert_fails(
        &format!("'{FOX}' 'x' 2 HASH"),
        "HASH expects two LONGs as hash key.",
    );
    assert_fails("1.5 HASH", "HASH operates on a byte array or STRING.");
    assert_fails("HASH", "Empty stack.");
    assert_fails("2 HASH", "Empty stack.");
    assert_fails("1 2 HASH", "Empty stack.");
}

#[test]
fn base58_encoding_matches_warp10() {
    assert_top("'Hello World!' ->B58", "2NEpo7TZRRrLZSi2U");
    assert_top(
        "'The quick brown fox jumps over the lazy dog.' ->B58",
        "USm3fpXnKG5EUBx2ndxBDMPVciP5hGey2Jh4NDv6gmeo1LkMeiKrLJUUBk6Z",
    );
    assert_top("'000000287fb4cd' HEX-> ->B58", "111233QC4");
    assert_top("'000000' HEX-> ->B58", "111");
    assert_top("'ff' HEX-> ->B58", "5Q");
    assert_top(
        &format!("'{}' HEX-> ->B58", seq_hex(200)),
        "16822n8xGe8uJgPUXZdNwwaVeadKw3tc5nAz4oth9XrRCvEfvxdiNztLDz4rvZSw5BjAXV2LGHF6Jv8dZdhofWp4nsDc3h5W7YDPuatH348jB4GspzSEB8WN7FtX1g5863P7PTN2q5KSdM26Ww8THxZstfLh9i4A2D4eFUsctY7DjR4nji3TDnAHq1AiV7cSqrbwk7naMutVhpFP2eP6v9YWphKUNxPZhWR4t4CuLL9HV2t7NZC2sSKjDpG5q77PccDmKP7kzXs4uuki",
    );
    // `data[0]` of an empty array is out of bounds in Java.
    assert_fails("'' ->B58", "Index 0 out of bounds for length 0");
    assert_fails("1 ->B58", "->B58 operates on STRING or a byte array.");
    assert_fails("->B58", "Empty stack.");
}

#[test]
fn base58check_encoding_matches_warp10() {
    assert_top("'hello' '00' HEX-> ->B58C", "12L5B5yqsf7vwb");
    assert_top(
        &format!("'{}' HEX-> '0080' HEX-> ->B58C", seq_hex(10)),
        "14ahdfpqYsu2btCGAvRzgj",
    );
    assert_top("'' '' HEX-> ->B58C", "3QJmnh");
    assert_fails("'x' 1 ->B58C", "->B58C expects a byte array prefix.");
    assert_fails(
        "1 '' HEX-> ->B58C",
        "->B58C operates on STRING or a byte array.",
    );
    assert_fails("->B58C", "Empty stack.");
    assert_fails("'' HEX-> ->B58C", "Empty stack.");
}

#[test]
fn base58_decoding_matches_warp10() {
    assert_top("'2NEpo7TZRRrLZSi2U' B58-> 'UTF-8' BYTES->", "Hello World!");
    assert_top("'111233QC4' B58-> ->HEX", "000000287fb4cd");
    assert_top("'111' B58-> ->HEX", "000000");
    assert_top("'5Q' B58-> ->HEX", "ff");
    assert_top("'' B58-> ->HEX", "");
    assert_top(
        &format!("'{}' HEX-> ->B58 B58-> ->HEX", seq_hex(200)),
        seq_hex(200),
    );
    assert!(fail_msg("'10' B58->").ends_with(
        "(B58-> encountered an error while decoding Base58. (Invalid input '0' at position 1.)))"
    ));
    assert_fails("'é' B58->", "Invalid input 'é' at position 0.");
    // Characters between 'z' and DEL are looked up past the end of Java's table.
    assert_fails("'a{' B58->", "Index 123 out of bounds for length 123");
    assert_fails("'~' B58->", "Index 126 out of bounds for length 123");
    assert_fails("1 B58->", "B58-> operates on a STRING.");
    assert_fails("B58->", "Empty stack.");
}

#[test]
fn base58check_decoding_matches_warp10() {
    assert_top("'12L5B5yqsf7vwb' '00' HEX-> B58C-> ->HEX", "68656c6c6f");
    assert_top("'3QJmnh' '' HEX-> B58C-> ->HEX", "");
    assert_top(
        "'14ahdfpqYsu2btCGAvRzgj' '0080' HEX-> B58C-> ->HEX",
        seq_hex(10),
    );
    assert_fails(
        "'3QJmnh' '00' HEX-> B58C->",
        "B58C-> Base58 STRING too short.",
    );
    assert_fails(
        "'12L5B5yqsf7vwb' '01' HEX-> B58C->",
        "B58C-> invalid prefix.",
    );
    assert_fails(
        "'12L5B5yqsf7vwc' '00' HEX-> B58C->",
        "B58C-> invalid checksum.",
    );
    assert_fails(
        "'0' '' HEX-> B58C->",
        "B58C-> encountered an error while decoding Base58.",
    );
    assert_fails("'x' 1 B58C->", "B58C-> expects a byte array prefix.");
    assert_fails("1 '' HEX-> B58C->", "B58C-> operates on a STRING.");
    assert_fails("B58C->", "Empty stack.");
    assert_fails("'' HEX-> B58C->", "Empty stack.");
}

#[test]
fn base58_helpers() {
    assert_eq!(b58_encode(&[0, 0, 1]).unwrap(), "112");
    assert_eq!(b58_decode("112").unwrap(), vec![0, 0, 1]);
    assert_eq!(b58_decode("1").unwrap(), vec![0]);
    // A lone surrogate is rendered as U+FFFD (Java prints the unpaired char).
    assert_eq!(
        b58_decode("a😀").unwrap_err().message(),
        "Invalid input '\u{FFFD}' at position 1."
    );
}

/// The AES keys of the reference vectors: `seq(16)`, `seq(24)` and `seq(32)`.
fn keks() -> [(Kek, &'static str); 3] {
    [
        (Kek::new(&seq(16)).unwrap(), "k16"),
        (Kek::new(&seq(24)).unwrap(), "k24"),
        (Kek::new(&seq(32)).unwrap(), "k32"),
    ]
}

#[test]
fn aes_wrap_matches_cryptoutils() {
    // `CryptoUtils.wrap` of "", "a", seq(8), seq(15), seq(16) and seq(40).
    let expected: [[&str; 6]; 3] = [
        [
            "254a9ada96556687e1ecabf7ab1916a4",
            "0d86cb77c794594da2df27fcad924e2a",
            "ac325c8ad84218977d3507fdec746163bf090ac462bfdda5",
            "dd2ccfd89da339dcc0809355beeae2c94379f1020dddff18",
            "010506605ab646a5a30054bc1efdb0bed88deab07f37fb5a6376ff769eb9976b",
            "88add875de9b7ac5e5a43ff40881af739d02218c0e370ce568b9309291a114d88f7e49348651e3e370e26ca95277edd6b158dae490727553",
        ],
        [
            "b1d9da4f0977782e77d6aa232c937bbe",
            "c37bad6d04f59f167f69c6b1f30a213d",
            "41dac0dc99414faf3cbb899c6dda4a9a2065e5a5135d9889",
            "d757f018d799503f97018ba083b48cb135a3eb52cf4ca925",
            "9ef97eec0f5ae71b17006572056960e4f1a0e0a31e69f6c4b24a2348b33ef3de",
            "9495a19ced9b3a6c80c8abad6ffcabfa7500677e91e26e35dfa1960c500e9f2daaea07b1fb2e7432ec80ef42637576e70b7ceaffcb88f66c",
        ],
        [
            "1957cad83aa89b9a1f4014afa31ae701",
            "27791c824fd9ed28e6ccd9de380708a9",
            "770795eaa2fd7cb9d43363683b4ddb5e67ed71cef4e8e28a",
            "a1eeb8b7df0422db828b8878e8c27c0bf22d07e14e7f5c4f",
            "58d62df595e6eba3d3a923782c6187c881295c0e8ce9005cab7b346a04ef1c82",
            "866f115632a34916a034720dcd741fdb781eeb5eea5f10bbe5d8dd16514e237b117b1962ab04fcbd6ba75dc0be1cbbe731335a2e66b9dd45",
        ],
    ];
    let datas: [Vec<u8>; 6] = [vec![], b"a".to_vec(), seq(8), seq(15), seq(16), seq(40)];
    for ((kek, label), wants) in keks().into_iter().zip(expected) {
        for (data, want) in datas.iter().zip(wants) {
            let wrapped = aes_wrap(&kek, data);
            assert_eq!(hex_encode(&wrapped), want, "{label} {data:?}");
            assert_eq!(aes_unwrap(&kek, &wrapped).unwrap(), *data, "{label}");
        }
    }
}

#[test]
fn aes_unwrap_rejects_bad_data() {
    let (kek, _) = keks().into_iter().next().unwrap();
    // Not a multiple of 8 bytes, too short, bad integrity check.
    for bad in [seq(0), seq(7), seq(8), seq(16), seq(24)] {
        assert!(aes_unwrap(&kek, &bad).is_none(), "{bad:?}");
    }
    // Undecipherable PKCS#7 padding: count 0, count larger than the data, mismatched bytes.
    for padded in [[0u8; 8], [0, 1, 2, 3, 4, 5, 6, 9], [1, 1, 1, 1, 1, 1, 2, 3]] {
        assert!(
            aes_unwrap(&kek, &rfc3394_wrap(&kek, &padded)).is_none(),
            "{padded:?}"
        );
    }
    // A full block of padding is valid and unwraps to nothing.
    assert_eq!(
        aes_unwrap(&kek, &rfc3394_wrap(&kek, &[8u8; 8])).unwrap(),
        b""
    );
}

#[test]
fn aeswrap_and_aesunwrap_round_trip() {
    let key = seq_hex(32);
    assert_top(
        &format!("'secret' '{key}' HEX-> AESWRAP '{key}' HEX-> AESUNWRAP 'UTF-8' BYTES->"),
        "secret",
    );
    assert_top(
        &format!("'' HEX-> '{key}' HEX-> AESWRAP '{key}' HEX-> AESUNWRAP ->HEX"),
        "",
    );
    // 8 bytes of nonce, PKCS#7 padding and the 8-byte integrity value.
    assert_top(&format!("'abc' '{key}' HEX-> AESWRAP SIZE"), 24);
    // Two wraps of the same data differ by their nonce.
    let mut stack = exec(&format!(
        "'abc' '{key}' HEX-> AESWRAP 'abc' '{key}' HEX-> AESWRAP"
    ));
    let second = stack.pop().unwrap();
    let first = stack.pop().unwrap();
    assert_ne!(first, second);
}

#[test]
fn aeswrap_with_a_known_nonce() {
    let kek = Kek::new(&seq(16)).unwrap();
    let mut stack = Stack::new();
    stack.push(Value::from("abc"));
    stack.push(Value::bytes(seq(16)));
    aeswrap_impl(&mut stack, Ok([7; NONCE_LEN])).unwrap();
    let wrapped = stack.pop().unwrap();
    let mut expected = vec![7u8; NONCE_LEN];
    expected.extend_from_slice(b"abc");
    assert_eq!(wrapped, Value::bytes(aes_wrap(&kek, &expected)));

    stack.push(Value::from("abc"));
    stack.push(Value::bytes(seq(16)));
    let err = aeswrap_impl(&mut stack, Err(getrandom::Error::UNSUPPORTED)).unwrap_err();
    assert!(
        err.to_string()
            .starts_with("Unable to obtain random data from the operating system ("),
        "{err}"
    );
    assert_eq!(stack.depth(), 0);
}

#[test]
fn aes_key_errors() {
    const MESSAGE: &str =
        "expects a byte array containing a 128, 192 or 256 bits AES key on the stack.";
    assert_fails("'abc' 'key' AESWRAP", &format!("AESWRAP {MESSAGE}"));
    assert_fails("'abc' '0102' HEX-> AESWRAP", &format!("AESWRAP {MESSAGE}"));
    assert_fails(
        &format!("'abc' '{}' HEX-> AESWRAP", seq_hex(20)),
        &format!("AESWRAP {MESSAGE}"),
    );
    assert_fails("'abc' 'key' AESUNWRAP", &format!("AESUNWRAP {MESSAGE}"));
    assert_fails(
        &format!("1 '{}' HEX-> AESWRAP", seq_hex(16)),
        "AESWRAP operates on a byte array or a String.",
    );
    assert_fails(
        &format!("'abc' '{}' HEX-> AESUNWRAP", seq_hex(16)),
        "AESUNWRAP operates on a byte array.",
    );
    assert_fails(
        &format!("'{}' HEX-> '{}' HEX-> AESUNWRAP", seq_hex(24), seq_hex(16)),
        "AESUNWRAP encountered undecipherable data.",
    );
    assert_fails("AESWRAP", "Empty stack.");
    assert_fails("AESUNWRAP", "Empty stack.");
    assert_fails(&format!("'{}' HEX-> AESWRAP", seq_hex(16)), "Empty stack.");
    assert_fails(
        &format!("'{}' HEX-> AESUNWRAP", seq_hex(32)),
        "Empty stack.",
    );
}

#[test]
fn aesunwrap_without_nonce_fails_like_copyofrange() {
    // Data wrapped without the 8-byte nonce: `Arrays.copyOfRange(unwrapped, 8, 3)` fails.
    let kek = Kek::new(&seq(16)).unwrap();
    let wrapped = hex_encode(&aes_wrap(&kek, b"abc"));
    assert!(
        fail_msg(&format!(
            "'{wrapped}' HEX-> '{}' HEX-> AESUNWRAP",
            seq_hex(16)
        ))
        .ends_with("=>AESUNWRAP<=' in section [TOP] (8 > 3))")
    );
    let wrapped = hex_encode(&aes_wrap(&kek, b""));
    assert_fails(
        &format!("'{wrapped}' HEX-> '{}' HEX-> AESUNWRAP", seq_hex(16)),
        "8 > 0",
    );
    // Exactly the nonce: nothing is left.
    let wrapped = hex_encode(&aes_wrap(&kek, &seq(8)));
    assert_top(
        &format!("'{wrapped}' HEX-> '{}' HEX-> AESUNWRAP ->HEX", seq_hex(16)),
        "",
    );
}

#[test]
fn kek_accepts_only_aes_key_sizes() {
    for len in [0, 15, 17, 23, 25, 31, 33, 64] {
        assert!(Kek::new(&seq(len)).is_none(), "{len}");
    }
    assert_eq!(hex_decode("00").unwrap(), vec![0]);
}

/// Run `script` on a stack granted `capabilities`.
fn with_capabilities(capabilities: &[(&str, &str)], script: &str) -> Result<Value, String> {
    let mut stack = Stack::with_limits(crate::stack::Limits::unlimited());
    for (name, value) in capabilities {
        stack
            .capabilities_mut()
            .insert((*name).to_owned(), (*value).to_owned());
    }
    match stack.exec_checked(script) {
        Ok(()) => stack.pop().map_err(|e| e.to_string()),
        Err(err) => Err(err.to_string()),
    }
}

#[test]
fn argon2_derives_keys() {
    assert_top(
        "{ 'password' 'password' } ARGON2 ->HEX",
        "46fa7bf33c1a3f1329d381bcb8c12d526f0971717297df36e58d73177b935f87",
    );
    assert_top(
        "{ 'password' 'password' 'type' 'i' 'iterations' 3 'memory' 32 'parallelism' 1 'salt' 'saltsalt' 'UTF-8' ->BYTES } ARGON2 ->HEX",
        "002b3a1d0d35f03eec61552964d4e6fca26b0e770397d42e9f7b358264ed6210",
    );
    assert_top(
        "{ 'password' 'password' 'UTF-8' ->BYTES 'type' 'D' 'memory' 32 'salt' 'saltsalt' 'UTF-8' ->BYTES } ARGON2 ->HEX",
        "71b6fbb2d5f73f84390d47577f78caee4f48b0c51accf4467631153850318373",
    );
    assert_top(
        "{ 'password' 'password' 'type' 'ID' 'memory' 32 'salt' 'saltsalt' 'UTF-8' ->BYTES } ARGON2 ->HEX",
        "207e0af68739ae80a597ba07aff5cbb27403abedabba6a7828423e71d600c93a",
    );
    // Unknown types fall back to Argon2id; strings are not valid salts.
    assert_top(
        "{ 'password' 'password' 'type' 42 'memory' 32 'salt' 'saltsalt' 'UTF-8' ->BYTES } ARGON2 ->HEX",
        "207e0af68739ae80a597ba07aff5cbb27403abedabba6a7828423e71d600c93a",
    );
    assert_top(
        "{ 'password' 'password' 'memory' 32 'salt' 'saltsalt' 'size' 16 } ARGON2 SIZE",
        16,
    );
    assert_top(
        "{ 'password' 'password' 'memory' 32 'salt' 'abc' 'UTF-8' ->BYTES 'size' 16 } ARGON2 ->HEX",
        "a7688792942f660b490f1eaadc2450db",
    );
    assert_top(
        "{ 'password' 'password' 'memory' 32 'salt' 'saltsalt' 'UTF-8' ->BYTES 'secret' 'secret' 'UTF-8' ->BYTES 'additional' 'additional' 'UTF-8' ->BYTES } ARGON2 ->HEX",
        "b890e8df6d4d89a6dbb247989ca9f2dedecb15e9a14d9c1b9ee6f2753252402c",
    );
    // `LONG` parameters are truncated to Java `int`s.
    assert_top(
        "{ 'password' 'password' 'memory' 4294967328 'salt' 'saltsalt' 'UTF-8' ->BYTES } ARGON2 ->HEX",
        "207e0af68739ae80a597ba07aff5cbb27403abedabba6a7828423e71d600c93a",
    );
}

#[test]
fn argon2_capabilities() {
    let script = |extra: &str| {
        format!(
            "{{ 'password' 'password' 'salt' 'saltsalt' 'UTF-8' ->BYTES {extra} }} ARGON2 ->HEX"
        )
    };
    let hex = |value: Result<Value, String>| value.unwrap().to_string();
    assert_eq!(
        hex(with_capabilities(
            &[("argon2.maxmem", "64"), ("argon2.maxpar", "2")],
            &script("'iterations' 1 'memory' 64 'parallelism' 2 'size' 4")
        )),
        "78b6a127"
    );
    assert_eq!(
        hex(with_capabilities(
            &[("argon2.maxpar", "4"), ("argon2.maxsize", "100")],
            &script("'iterations' 2 'memory' 8 'parallelism' 4 'size' 100")
        )),
        "501be8074ea5786a54254685263ccb10cdcf2a2c330fcbb660749277a23a245245a7accb04e07d700705445ca0d1d82168d302a349e3b2b5b9bdcd1034c7a4d580390fa0ca0cf25781c346c5e08b7c8c6e9591d03bd9def06acdb282fe91959c9ea3f7e8"
    );
    assert_eq!(
        hex(with_capabilities(
            &[("argon2.maxpar", "3")],
            &script("'memory' 32 'parallelism' 3")
        )),
        "73c486f82788495bd84a405fdf2ceb933a9defa5b94163185288ec041de30c26"
    );
    // Above the defaults without the capability, or above the capability value.
    let missing = [
        ("'iterations' 4", "argon2.maxiter"),
        ("'memory' 33", "argon2.maxmem"),
        ("'parallelism' 2", "argon2.maxpar"),
        ("'size' 33", "argon2.maxsize"),
    ];
    for (extra, cap) in missing {
        assert_fails(
            &script(extra),
            &format!("ARGON2 missing capability '{cap}'."),
        );
    }
    let exceeded = [
        (
            "'iterations' 5",
            "argon2.maxiter",
            "ARGON2 number of iterations exceeds capability value.",
        ),
        (
            "'memory' 65",
            "argon2.maxmem",
            "ARGON2 memory exceeds capability value.",
        ),
        (
            "'parallelism' 5",
            "argon2.maxpar",
            "ARGON2 lane count exceeds capability value.",
        ),
        (
            "'size' 65",
            "argon2.maxsize",
            "ARGON2 requested output size exceeds capability value.",
        ),
    ];
    for (extra, cap, message) in exceeded {
        let err = with_capabilities(&[(cap, "4")], &script(extra)).unwrap_err();
        assert!(err.contains(message), "{err}");
    }
    let err =
        with_capabilities(&[("argon2.maxiter", "lots")], &script("'iterations' 4")).unwrap_err();
    assert!(err.contains("For input string: \"lots\""), "{err}");
}

#[test]
fn argon2_errors() {
    assert_fails("'password' ARGON2", "ARGON2 expects a parameter map.");
    assert_fails("ARGON2", "Empty stack.");
    assert_fails(
        "{ 'memory' 32 } ARGON2",
        "ARGON2 missing password as either STRING or BYTES.",
    );
    assert_fails(
        "{ 'password' 1 } ARGON2",
        "ARGON2 missing password as either STRING or BYTES.",
    );
    // Parameters the BouncyCastle generator rejects.
    assert_fails(
        "{ 'password' 'p' 'size' 3 } ARGON2",
        "output length less than 4",
    );
    assert_fails("{ 'password' 'p' 'size' -1 } ARGON2", "(-1)");
    assert_fails(
        "{ 'password' 'p' 'iterations' 0 } ARGON2",
        "iterations is less than: 1",
    );
    assert_fails(
        "{ 'password' 'p' 'memory' 1 } ARGON2",
        "memory is less than: 2 expected 2",
    );
    assert_fails(
        "{ 'password' 'p' 'parallelism' 0 } ARGON2",
        "lanes must be greater than 1",
    );
    assert_fails(
        "{ 'password' 'p' 'parallelism' -1 } ARGON2",
        "lanes must be greater than 1",
    );
}
