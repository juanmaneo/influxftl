// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the elliptic curve functions and of the signature of macros. The tables are what
//! Warp 10 3.5.0 (BouncyCastle) gives for the same scripts: the stack as JSON for the scripts
//! that work (`STACKTOLIST ->JSON`), the message for those that fail. What is random in Warp 10
//! (the keys and the signatures it makes) is not compared: the signatures it made are verified
//! here, and the ones made here are verified here.

use std::rc::Rc;

use num_bigint::BigInt;

use super::ec::{ecgen_with, ecsign_with, msign_with};
use crate::ec::{Curve, Key, PrivateKey};
use crate::error::Error;
use crate::macros::{Macro, Statement};
use crate::stack::Stack;
use crate::tests::{assert_fails, assert_top, exec, top};
use crate::value::Value;

const KEYS_VALUES: &[(&str, &str)] = &[
    (
        r"{ 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPRIVATE STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","d":"12345"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE "]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'secp256k1' 'Q' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' } ECPUBLIC "]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC TYPEOF STACKTOLIST ->JSON",
        r#"["KEY"]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE TYPEOF STACKTOLIST ->JSON",
        r#"["KEY"]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256r1","Q":"0426efcebd0ee9e34a669187e18b3a9122b2f733945b649cc9f9f921e9f9dad81290238bde9cc7bb330d150c67704dd25ae7055205744b6f31bf4070745872d0e6"}]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPRIVATE STACKTOLIST ->JSON",
        r#"[{"curve":"secp256r1","d":"12345"}]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE "]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'secp256r1' 'Q' '0426efcebd0ee9e34a669187e18b3a9122b2f733945b649cc9f9f921e9f9dad81290238bde9cc7bb330d150c67704dd25ae7055205744b6f31bf4070745872d0e6' } ECPUBLIC "]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC TYPEOF STACKTOLIST ->JSON",
        r#"["KEY"]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE TYPEOF STACKTOLIST ->JSON",
        r#"["KEY"]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp384r1","Q":"04e86b995e350e4d312a95a6b7d5d8cdcdb4682f588800a1cd6c972e1dcb2a5ac38d90046951b03ccb3b179a89fda88b1b485b9f2c1f44b0dd41ce6e3c3a690bbfaabec1e1ff4aefb2461fdda6629becbd43cf74c13d8c1a1625612243f0046148"}]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPRIVATE STACKTOLIST ->JSON",
        r#"[{"curve":"secp384r1","d":"12345"}]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE "]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'secp384r1' 'Q' '04e86b995e350e4d312a95a6b7d5d8cdcdb4682f588800a1cd6c972e1dcb2a5ac38d90046951b03ccb3b179a89fda88b1b485b9f2c1f44b0dd41ce6e3c3a690bbfaabec1e1ff4aefb2461fdda6629becbd43cf74c13d8c1a1625612243f0046148' } ECPUBLIC "]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC TYPEOF STACKTOLIST ->JSON",
        r#"["KEY"]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE TYPEOF STACKTOLIST ->JSON",
        r#"["KEY"]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp521r1","Q":"0400076ca1fa9613a70e8b2cecd8f64877968e32b91b16cd65166ca44b1439883a919e263c09cff2f7f39e770ebda9057d2f1198804a1c160f0bb99eb9ae6fab28f78b00b3121fe54f8e83d9bdea643e8f7a01fb6e0791d4d459ac5ba1ea434b87d2d6b9d6476a5f521ee0f6b8de3ce4f3474660235f88240ca7d0ddba59a07d493692bd6b"}]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPRIVATE STACKTOLIST ->JSON",
        r#"[{"curve":"secp521r1","d":"12345"}]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE "]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'secp521r1' 'Q' '0400076ca1fa9613a70e8b2cecd8f64877968e32b91b16cd65166ca44b1439883a919e263c09cff2f7f39e770ebda9057d2f1198804a1c160f0bb99eb9ae6fab28f78b00b3121fe54f8e83d9bdea643e8f7a01fb6e0791d4d459ac5ba1ea434b87d2d6b9d6476a5f521ee0f6b8de3ce4f3474660235f88240ca7d0ddba59a07d493692bd6b' } ECPUBLIC "]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC TYPEOF STACKTOLIST ->JSON",
        r#"["KEY"]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE TYPEOF STACKTOLIST ->JSON",
        r#"["KEY"]"#,
    ),
    (
        r"{ 'curve' 'prime256v1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"prime256v1","Q":"0426efcebd0ee9e34a669187e18b3a9122b2f733945b649cc9f9f921e9f9dad81290238bde9cc7bb330d150c67704dd25ae7055205744b6f31bf4070745872d0e6"}]"#,
    ),
    (
        r"{ 'curve' 'prime256v1' 'd' '12345' } ECPRIVATE ECPUBLIC SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'prime256v1' 'Q' '0426efcebd0ee9e34a669187e18b3a9122b2f733945b649cc9f9f921e9f9dad81290238bde9cc7bb330d150c67704dd25ae7055205744b6f31bf4070745872d0e6' } ECPUBLIC "]"#,
    ),
    (
        r"{ 'curve' 'P-256' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"P-256","Q":"0426efcebd0ee9e34a669187e18b3a9122b2f733945b649cc9f9f921e9f9dad81290238bde9cc7bb330d150c67704dd25ae7055205744b6f31bf4070745872d0e6"}]"#,
    ),
    (
        r"{ 'curve' 'P-256' 'd' '12345' } ECPRIVATE ECPUBLIC SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'P-256' 'Q' '0426efcebd0ee9e34a669187e18b3a9122b2f733945b649cc9f9f921e9f9dad81290238bde9cc7bb330d150c67704dd25ae7055205744b6f31bf4070745872d0e6' } ECPUBLIC "]"#,
    ),
    (
        r"{ 'curve' 'SECP256K1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"SECP256K1","Q":"04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295"}]"#,
    ),
    (
        r"{ 'curve' 'SECP256K1' 'd' '12345' } ECPRIVATE ECPUBLIC SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'SECP256K1' 'Q' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' } ECPUBLIC "]"#,
    ),
    (
        r"{ 'curve' 'p-384' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"p-384","Q":"04e86b995e350e4d312a95a6b7d5d8cdcdb4682f588800a1cd6c972e1dcb2a5ac38d90046951b03ccb3b179a89fda88b1b485b9f2c1f44b0dd41ce6e3c3a690bbfaabec1e1ff4aefb2461fdda6629becbd43cf74c13d8c1a1625612243f0046148"}]"#,
    ),
    (
        r"{ 'curve' 'p-384' 'd' '12345' } ECPRIVATE ECPUBLIC SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'p-384' 'Q' '04e86b995e350e4d312a95a6b7d5d8cdcdb4682f588800a1cd6c972e1dcb2a5ac38d90046951b03ccb3b179a89fda88b1b485b9f2c1f44b0dd41ce6e3c3a690bbfaabec1e1ff4aefb2461fdda6629becbd43cf74c13d8c1a1625612243f0046148' } ECPUBLIC "]"#,
    ),
    (
        r"{ 'curve' 'P-521' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"P-521","Q":"0400076ca1fa9613a70e8b2cecd8f64877968e32b91b16cd65166ca44b1439883a919e263c09cff2f7f39e770ebda9057d2f1198804a1c160f0bb99eb9ae6fab28f78b00b3121fe54f8e83d9bdea643e8f7a01fb6e0791d4d459ac5ba1ea434b87d2d6b9d6476a5f521ee0f6b8de3ce4f3474660235f88240ca7d0ddba59a07d493692bd6b"}]"#,
    ),
    (
        r"{ 'curve' 'P-521' 'd' '12345' } ECPRIVATE ECPUBLIC SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'P-521' 'Q' '0400076ca1fa9613a70e8b2cecd8f64877968e32b91b16cd65166ca44b1439883a919e263c09cff2f7f39e770ebda9057d2f1198804a1c160f0bb99eb9ae6fab28f78b00b3121fe54f8e83d9bdea643e8f7a01fb6e0791d4d459ac5ba1ea434b87d2d6b9d6476a5f521ee0f6b8de3ce4f3474660235f88240ca7d0ddba59a07d493692bd6b' } ECPUBLIC "]"#,
    ),
    (
        r"{ 'curve' 'Secp256R1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"Secp256R1","Q":"0426efcebd0ee9e34a669187e18b3a9122b2f733945b649cc9f9f921e9f9dad81290238bde9cc7bb330d150c67704dd25ae7055205744b6f31bf4070745872d0e6"}]"#,
    ),
    (
        r"{ 'curve' 'Secp256R1' 'd' '12345' } ECPRIVATE ECPUBLIC SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'Secp256R1' 'Q' '0426efcebd0ee9e34a669187e18b3a9122b2f733945b649cc9f9f921e9f9dad81290238bde9cc7bb330d150c67704dd25ae7055205744b6f31bf4070745872d0e6' } ECPUBLIC "]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' 15 } ECPRIVATE ECPRIVATE STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","d":"15"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' 15 } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"04d7924d4f7d43ea965a465ae3095ff41131e5946f3c85f79e44adbcf8e27e080e581e2872a86c72a683842ec228cc6defea40af2bd896d3a5c504dc9ff6a26b58"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '0x10' } ECPRIVATE ECPRIVATE STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","d":"16"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '0x10' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"04e60fce93b59e9ec53011aabc21c23e97b2a31369b87a5ae9c44ee89e2a6dec0af7e3507399e595929db99f34f57937101296891e44d23f0be1f32cce69616821"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '-0x10' } ECPRIVATE ECPRIVATE STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","d":"-16"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '-0x10' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"04e60fce93b59e9ec53011aabc21c23e97b2a31369b87a5ae9c44ee89e2a6dec0a081caf8c661a6a6d624660cb0a86c8efed6976e1bb2dc0f41e0cd330969e940e"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '0b101' } ECPRIVATE ECPRIVATE STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","d":"5"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '0b101' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"042f8bde4d1a07209355b4a7250a5c5128e88b84bddc619ab7cba8d569b240efe4d8ac222636e5e3d6d4dba9dda6c9c426f788271bab0d6840dca87d3aa6ac62d6"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '-0b101' } ECPRIVATE ECPRIVATE STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","d":"-5"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '-0b101' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"042f8bde4d1a07209355b4a7250a5c5128e88b84bddc619ab7cba8d569b240efe42753ddd9c91a1c292b24562259363bd90877d8e454f297bf235782c459539959"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '-1' } ECPRIVATE ECPRIVATE STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","d":"-1"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '-1' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798b7c52588d95c3b9aa25b0403f1eef75702e84bb7597aabe663b82f6f04ef2777"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '0' } ECPRIVATE ECPRIVATE STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","d":"0"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '0' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"00"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '12345' ->BD } ECPRIVATE ECPRIVATE STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","d":"12345"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '12345' ->BD } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789' } ECPRIVATE ECPRIVATE STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","d":"123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"04d26279bd46ed1b05c6c0652fd66f6da7e0c7c136e2fe0d6d1f7039d96787f505c8dced87013a24353c42e729f06d8ff741043a85792d47cf1fec1d6224f4c845"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141' } ECPRIVATE ECPRIVATE STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","d":"115792089237316195423570985008687907852837564279074904382605163141518161494337"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"00"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364142' } ECPRIVATE ECPRIVATE STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","d":"115792089237316195423570985008687907852837564279074904382605163141518161494338"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364142' } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' -5 } ECPRIVATE ECPRIVATE STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","d":"-5"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' -5 } ECPRIVATE ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"042f8bde4d1a07209355b4a7250a5c5128e88b84bddc619ab7cba8d569b240efe42753ddd9c91a1c292b24562259363bd90877d8e454f297bf235782c459539959"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8' } ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8' } ECPUBLIC SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'secp256k1' 'Q' '0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8' } ECPUBLIC "]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798' } ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798' } ECPUBLIC SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'secp256k1' 'Q' '0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8' } ECPUBLIC "]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '0679be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8' } ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"0679be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '0679be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8' } ECPUBLIC SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'secp256k1' 'Q' '0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8' } ECPUBLIC "]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '00' } ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"00"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '00' } ECPUBLIC SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'secp256k1' 'Q' '00' } ECPUBLIC "]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '0479BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8' } ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '0479BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8' } ECPUBLIC SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'secp256k1' 'Q' '0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8' } ECPUBLIC "]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '0279BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798' } ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"}]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '0279BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798' } ECPUBLIC SNAPSHOT STACKTOLIST ->JSON",
        r#"["{ 'curve' 'secp256k1' 'Q' '0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8' } ECPUBLIC "]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '0479be667e f9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8' } ECPUBLIC ECPUBLIC STACKTOLIST ->JSON",
        r#"[{"curve":"secp256k1","Q":"0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8"}]"#,
    ),
    (r"<% 1 2 + %> EVAL SNAPSHOT", r"3 "),
    (r"<% 1 'a' 'b' 'c' %> EVAL SNAPSHOT", r"1 'a' 'b' 'c' "),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' %> EVAL SNAPSHOT",
        r"3 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' ",
    ),
];

const KEYS_ERRORS: &[(&str, &str)] = &[
    (r"{ } ECPRIVATE", r"ECPRIVATE curve name not in"),
    (
        r"{ 'curve' 'nope' } ECPRIVATE",
        r"ECPRIVATE curve name not in",
    ),
    (r"{ 'curve' 1 } ECPRIVATE", r"ECPRIVATE curve name not in"),
    (
        r"{ 'curve' 'secp256k1' } ECPRIVATE",
        r"ECPRIVATE missing private key under 'd'.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' 1.5 } ECPRIVATE",
        r"ECPRIVATE can only be applied to a BIGDECIMAL with no fractional part.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' NULL } ECPRIVATE",
        r"ECPRIVATE missing private key under 'd'.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' T } ECPRIVATE",
        r"ECPRIVATE can only be applied to a BIGDECIMAL, STRING, BYTES, LONG or DOUBLE argument.",
    ),
    (
        r"1 ECPRIVATE",
        r"ECPRIVATE expects a parameter map or an ECC private key instance.",
    ),
    (r"ECPRIVATE", r"Empty stack."),
    (
        r"{ 'curve' 'secp256k1' 'Q' '00' } ECPUBLIC ECPRIVATE",
        r"ECPRIVATE expects a parameter map or an ECC private key instance.",
    ),
    (r"{ } ECPUBLIC", r"ECPUBLIC curve name 'null' not in"),
    (
        r"{ 'curve' 'nope' } ECPUBLIC",
        r"ECPUBLIC curve name 'nope' not in",
    ),
    (
        r"{ 'curve' 'secp256k1' } ECPUBLIC",
        r"ECPUBLIC missing or non-String parameter 'Q'.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' 1 } ECPUBLIC",
        r"ECPUBLIC missing or non-String parameter 'Q'.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' 'zz' } ECPUBLIC",
        r"exception decoding Hex string: invalid characters encountered in Hex string (invalid characters encountered in Hex string)",
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' 'abc' } ECPUBLIC",
        r"exception decoding Hex string: String index out of range: 3 (String index out of range: 3)",
    ),
    (r"{ 'curve' 'secp256k1' 'Q' '' } ECPUBLIC", r"0"),
    (
        r"{ 'curve' 'secp256k1' 'Q' '05' } ECPUBLIC",
        r"Invalid point encoding 0x5",
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '0000' } ECPUBLIC",
        r"Incorrect length for infinity encoding",
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '02' } ECPUBLIC",
        r"Incorrect length for compressed encoding",
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '04aa' } ECPUBLIC",
        r"Incorrect length for uncompressed encoding",
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d499' } ECPUBLIC",
        r"Invalid point coordinates",
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '0779be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8' } ECPUBLIC",
        r"Inconsistent Y coordinate in hybrid encoding",
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '0679be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4' } ECPUBLIC",
        r"Incorrect length for hybrid encoding",
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '02ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff' } ECPUBLIC",
        r"x value invalid for SecP256K1FieldElement",
    ),
    (
        r"{ 'curve' 'secp256r1' 'Q' '04ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff' } ECPUBLIC",
        r"x value invalid for SecP256R1FieldElement",
    ),
    (
        r"{ 'curve' 'secp256k1' 'Q' '0200000000000000000000000000000000000000000000000000000000000000' } ECPUBLIC",
        r"Incorrect length for compressed encoding",
    ),
    (
        r"1 ECPUBLIC",
        r"ECPUBLIC expects a parameter map, or a public or private key.",
    ),
    (r"ECPUBLIC", r"Empty stack."),
    (r"'a' ECGEN", r"ECGEN only supports the following curves:"),
    (r"1 ECGEN", r"ECGEN expects a curve name."),
    (r"ECGEN", r"Empty stack."),
];

const AGREEMENT_VALUES: &[(&str, &str)] = &[
    (
        r"{ 'curve' 'secp256k1' 'd' '123456789012345678901234567890' } ECPRIVATE { 'curve' 'secp256k1' 'd' '987654321098765432109876543210' } ECPRIVATE ECPUBLIC ECDH STACKTOLIST ->JSON",
        r#"["62ba2e7769e6f9d1eab9a5bce81b45e2a71018ea1fa00a7f57ae882be154499f"]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '987654321098765432109876543210' } ECPRIVATE { 'curve' 'secp256k1' 'd' '123456789012345678901234567890' } ECPRIVATE ECPUBLIC ECDH STACKTOLIST ->JSON",
        r#"["62ba2e7769e6f9d1eab9a5bce81b45e2a71018ea1fa00a7f57ae882be154499f"]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '123456789012345678901234567890' } ECPRIVATE { 'curve' 'secp256k1' 'd' '123456789012345678901234567890' } ECPRIVATE ECPUBLIC ECDH STACKTOLIST ->JSON",
        r#"["28621791a3893399fa6d805df2c6717e5a4ca8b446d09862247676fca2f12aff"]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '1' } ECPRIVATE { 'curve' 'secp256k1' 'd' '1' } ECPRIVATE ECPUBLIC ECDH STACKTOLIST ->JSON",
        r#"["79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'd' '123456789012345678901234567890' } ECPRIVATE { 'curve' 'secp256r1' 'd' '987654321098765432109876543210' } ECPRIVATE ECPUBLIC ECDH STACKTOLIST ->JSON",
        r#"["b3ce002fa7b7bed224867b93c362f16226964e73d2419262d70058e606768798"]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'd' '987654321098765432109876543210' } ECPRIVATE { 'curve' 'secp256r1' 'd' '123456789012345678901234567890' } ECPRIVATE ECPUBLIC ECDH STACKTOLIST ->JSON",
        r#"["b3ce002fa7b7bed224867b93c362f16226964e73d2419262d70058e606768798"]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'd' '123456789012345678901234567890' } ECPRIVATE { 'curve' 'secp256r1' 'd' '123456789012345678901234567890' } ECPRIVATE ECPUBLIC ECDH STACKTOLIST ->JSON",
        r#"["f143b150b6e97173da1e5c767fd91a8d73b8f6a2c0b1e9d75a7a1a2cea1b7496"]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'd' '1' } ECPRIVATE { 'curve' 'secp256r1' 'd' '1' } ECPRIVATE ECPUBLIC ECDH STACKTOLIST ->JSON",
        r#"["6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'd' '123456789012345678901234567890' } ECPRIVATE { 'curve' 'secp384r1' 'd' '987654321098765432109876543210' } ECPRIVATE ECPUBLIC ECDH STACKTOLIST ->JSON",
        r#"["2aa0df998ecace4f5d77ea87c2cb9f1ab01a352811a3ce11e43bd5dbb353adeff38e488489262b9f7947298d2a12592b"]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'd' '987654321098765432109876543210' } ECPRIVATE { 'curve' 'secp384r1' 'd' '123456789012345678901234567890' } ECPRIVATE ECPUBLIC ECDH STACKTOLIST ->JSON",
        r#"["2aa0df998ecace4f5d77ea87c2cb9f1ab01a352811a3ce11e43bd5dbb353adeff38e488489262b9f7947298d2a12592b"]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'd' '123456789012345678901234567890' } ECPRIVATE { 'curve' 'secp384r1' 'd' '123456789012345678901234567890' } ECPRIVATE ECPUBLIC ECDH STACKTOLIST ->JSON",
        r#"["a9c11f640f194b9de1638e7b0410dab9d7be42bf5a2b3033eac850ac4c4a61bbe787e3e8f4a1417a7f82cf88b0c51152"]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'd' '1' } ECPRIVATE { 'curve' 'secp384r1' 'd' '1' } ECPRIVATE ECPUBLIC ECDH STACKTOLIST ->JSON",
        r#"["aa87ca22be8b05378eb1c71ef320ad746e1d3b628ba79b9859f741e082542a385502f25dbf55296c3a545e3872760ab7"]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'd' '123456789012345678901234567890' } ECPRIVATE { 'curve' 'secp521r1' 'd' '987654321098765432109876543210' } ECPRIVATE ECPUBLIC ECDH STACKTOLIST ->JSON",
        r#"["54c9b52ba9ecd5fa88bbf33e0529adc0dfa0595c87c9405d64cfc30a1e85bf699df5b9968fd748128c481f11ebf9e25191acef682456b17ffd4b04732fd5e25084"]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'd' '987654321098765432109876543210' } ECPRIVATE { 'curve' 'secp521r1' 'd' '123456789012345678901234567890' } ECPRIVATE ECPUBLIC ECDH STACKTOLIST ->JSON",
        r#"["54c9b52ba9ecd5fa88bbf33e0529adc0dfa0595c87c9405d64cfc30a1e85bf699df5b9968fd748128c481f11ebf9e25191acef682456b17ffd4b04732fd5e25084"]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'd' '123456789012345678901234567890' } ECPRIVATE { 'curve' 'secp521r1' 'd' '123456789012345678901234567890' } ECPRIVATE ECPUBLIC ECDH STACKTOLIST ->JSON",
        r#"["8c6221bbc0a3bf33ac124ce08d0a6a24124b3b0a25d18025228ec69437366bfdd2525d528b26639edb5bf443688022c10806924bd2f7399bfc019837f92269aa5d"]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'd' '1' } ECPRIVATE { 'curve' 'secp521r1' 'd' '1' } ECPRIVATE ECPUBLIC ECDH STACKTOLIST ->JSON",
        r#"["c6858e06b70404e9cd9e3ecb662395b4429c648139053fb521f828af606b4d3dbaa14b5e77efe75928fe1dc127a2ffa8de3348b3c1856a429bf97e7e31c2e5bd66"]"#,
    ),
    // A secret of an odd number of digits is written with a leading zero.
    (
        r"{ 'curve' 'secp256k1' 'd' '45' } ECPRIVATE { 'curve' 'secp256k1' 'd' '1' } ECPRIVATE ECPUBLIC ECDH STACKTOLIST ->JSON",
        r#"["049370a4b5f43412ea25f514e8ecdad05266115e4a7ecb1387231808f8b45963"]"#,
    ),
];

const AGREEMENT_ERRORS: &[(&str, &str)] = &[
    (r"ECDH", r"Empty stack."),
    (r"1 ECDH", r"ECDH expects an ECC public key."),
    (
        r"{ 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECDH",
        r"ECDH expects an ECC public key.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECDH",
        r"ECDH expects an ECC public key.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECDH",
        r"Point must be on the same curve",
    ),
    (
        r"1 { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECDH",
        r"ECDH expects an ECC private key.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECDH",
        r"ECDH expects an ECC public key.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '0' } ECPRIVATE { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECDH",
        r"Scalar is not in the interval [1, n - 1]",
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141' } ECPRIVATE { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECDH",
        r"Scalar is not in the interval [1, n - 1]",
    ),
    (
        r"{ 'curve' 'secp256k1' 'd' '-1' } ECPRIVATE { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECDH",
        r"Scalar is not in the interval [1, n - 1]",
    ),
];

const SIGNATURES_VALUES: &[(&str, &str)] = &[
    (
        r"'hello' 'UTF-8' ->BYTES '3046022100a892643aa7dc78c5d0ad7a791bef48a57b52715f2f99730a46d4d9b27809f48a022100d50f4fcd7fbaea587dc4d4d2bb382d671e2ce88d8068e499f186df2b77410130' HEX-> 'ECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3046022100a892643aa7dc78c5d0ad7a791bef48a57b52715f2f99730a46d4d9b27809f48a022100d50f4fcd7fbaea587dc4d4d2bb382d671e2ce88d8068e499f186df2b77410130' HEX-> 'ECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3044022054d66772e2565c3896eb76519885f8479f9b672305658042e44d621cbf751dc7022016ad2b88e4ef46971cb2f2712c88b3367b176a3298b20f7b64cca7ad7df2e452' HEX-> 'SHA1WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3044022054d66772e2565c3896eb76519885f8479f9b672305658042e44d621cbf751dc7022016ad2b88e4ef46971cb2f2712c88b3367b176a3298b20f7b64cca7ad7df2e452' HEX-> 'SHA1WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '30450221009b2c09f1a7abf5a441a82b2103238fe52f4a95ef32d575ce3ff341e1a191ea53022008382fcc17dca944f87f627ac3a8e79a84b2f264646cb6f0a11fc7ef2cc28775' HEX-> 'SHA224WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '30450221009b2c09f1a7abf5a441a82b2103238fe52f4a95ef32d575ce3ff341e1a191ea53022008382fcc17dca944f87f627ac3a8e79a84b2f264646cb6f0a11fc7ef2cc28775' HEX-> 'SHA224WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3045022100f5c103cd3d9c43e521ca0098cbba7a33ed503b90767ac84ff18da2fbd017248f022005679f2b479fff8300cf5e4a49bac7a16355fec8a2bfed93b3046c1ec4941dd3' HEX-> 'SHA256WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3045022100f5c103cd3d9c43e521ca0098cbba7a33ed503b90767ac84ff18da2fbd017248f022005679f2b479fff8300cf5e4a49bac7a16355fec8a2bfed93b3046c1ec4941dd3' HEX-> 'SHA256WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3045022100c5cdfb0913d5e1877550d02242e12f9fe21d6cd8eb667a485d6069480e23e1a302206385ab635444c5749a854d5e99151a272aa217a563f9aad21799e706f9dd3da0' HEX-> 'SHA384WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3045022100c5cdfb0913d5e1877550d02242e12f9fe21d6cd8eb667a485d6069480e23e1a302206385ab635444c5749a854d5e99151a272aa217a563f9aad21799e706f9dd3da0' HEX-> 'SHA384WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '304402204b6ca761ed296a888f405d43b44b3dba554da1e3a119541525f4aff3c982de5e0220383aa52fe629a0c3e11f797bd8b0c4e93b5c8696679d793f476304c4ce7b5e4f' HEX-> 'SHA512WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '304402204b6ca761ed296a888f405d43b44b3dba554da1e3a119541525f4aff3c982de5e0220383aa52fe629a0c3e11f797bd8b0c4e93b5c8696679d793f476304c4ce7b5e4f' HEX-> 'SHA512WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3046022100de78c136316446ccf2802ba2a888351cc1472172b2aba024b9da211832254b9202210090471bd6e698f2bc100c5084428d55b98aee2685c1dc44742d15b6066554b57b' HEX-> 'SHA3-224WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3046022100de78c136316446ccf2802ba2a888351cc1472172b2aba024b9da211832254b9202210090471bd6e698f2bc100c5084428d55b98aee2685c1dc44742d15b6066554b57b' HEX-> 'SHA3-224WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3045022100a9185a1f9c12a970d06f5724e1f276d935d4380767f0bbfeb8f2bc43f8552e7802205ad8cd96eff9f27ae31996fb57392a2339ecc00ce9e29f116996d7f2c48e3b51' HEX-> 'SHA3-256WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3045022100a9185a1f9c12a970d06f5724e1f276d935d4380767f0bbfeb8f2bc43f8552e7802205ad8cd96eff9f27ae31996fb57392a2339ecc00ce9e29f116996d7f2c48e3b51' HEX-> 'SHA3-256WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '304502200e3c8a2a5cbb7560021eb570d2d3a87b9e6353d16ad6ea0ac6fade373f439f41022100c9ac6a726468711fc1cc3427ba2030d0bd08308921ae56a2ada95a4f604b95c3' HEX-> 'SHA3-384WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '304502200e3c8a2a5cbb7560021eb570d2d3a87b9e6353d16ad6ea0ac6fade373f439f41022100c9ac6a726468711fc1cc3427ba2030d0bd08308921ae56a2ada95a4f604b95c3' HEX-> 'SHA3-384WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3045022045d69fd0a55a2248dcdd002e35a33208d19226e370badc8df0d50e5d45f40551022100e3021dce8b6b231ce4f310273f66920016a16cb93064f7feed078f1ad2f8ec16' HEX-> 'SHA3-512WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3045022045d69fd0a55a2248dcdd002e35a33208d19226e370badc8df0d50e5d45f40551022100e3021dce8b6b231ce4f310273f66920016a16cb93064f7feed078f1ad2f8ec16' HEX-> 'SHA3-512WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '304502200e268119b186b4e796d3c1d7ac6cef4dd0a0c8c180edcdf19e452299bfde94a00221009a615f8321f42fdb83c568ad6b4f51c4409bc05eb7fac29e2953e0b81dc5dd4c' HEX-> 'RIPEMD160WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '304502200e268119b186b4e796d3c1d7ac6cef4dd0a0c8c180edcdf19e452299bfde94a00221009a615f8321f42fdb83c568ad6b4f51c4409bc05eb7fac29e2953e0b81dc5dd4c' HEX-> 'RIPEMD160WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3045022100878a3a2b03652177185d3c82feb28f9221ce872d6f7ad93499207ccf9c6760b702203e380c0ee75d17efabeef01955e0f4d125e6971c614e73c90b620730a32465e8' HEX-> 'sha256withecdsa' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3045022100878a3a2b03652177185d3c82feb28f9221ce872d6f7ad93499207ccf9c6760b702203e380c0ee75d17efabeef01955e0f4d125e6971c614e73c90b620730a32465e8' HEX-> 'sha256withecdsa' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3045022059c95722307f981ee3c1e00fd078d305ecd205d5cebe3878496cb3540cf56d3a022100897478aac16c98812b627d6050e8806f88120e1f1e263da328521870dc4fc00c' HEX-> 'SHA256withECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3045022059c95722307f981ee3c1e00fd078d305ecd205d5cebe3878496cb3540cf56d3a022100897478aac16c98812b627d6050e8806f88120e1f1e263da328521870dc4fc00c' HEX-> 'SHA256withECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'abababababababababababababababababababab' HEX-> '3045022100e1d1aa71a310d848b6ec28e9adf512277c0fdbc1ee57579f636f7aa5f2e784ab022078306c794f884673df54c08e5e0d0bae358491faef3a23f1ac585830ee68e51a' HEX-> 'NONEWITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'abababababababababababababababababababababababababababababababab' HEX-> '3045022100f044195ce11a1c553cc59ef9702c087c6c14c827555057ea4768541db5c6c62b022015b85fbc60ff79ccf65de2d3d266ec0b3d6f0e934b5b1787b221fe93df165c3d' HEX-> 'NONEWITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'abababababababababababababababababababababababababababababababababababababababababababababababab' HEX-> '304402206916c972429da3d20e13d69f4259dd69f16e7d0eff7c1c9b48f464bfdb14cacc02207759245d53443880d7e32b6e4114de6b4d3b8d238411ca311f310336411b0067' HEX-> 'NONEWITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'abababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababab' HEX-> '3045022041c8e9f229f285a3a69461088afe75937946014d90904a3eaff7f8b555a98df802210083767d3b2c0b61aacdd6835c57b80443b46753355f97057050edc91483f4423e' HEX-> 'NONEWITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'abababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababab' HEX-> '3044022034dec99413fe7e0ad12dbaaea4545575430fcd824291cf678c1117b592214a8602204f6c92332b290378b9553ccff612044e048ac626fed07417d20514b7a7bfded3' HEX-> 'NONEWITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3045022100ab464d5426cbc42e27505231a0a33f621ab974884bb5aa42e6711c5efb87bd1e0220140c79d0702267b1be0bd2d036533816c6b84cc7eaeae6f6d9314f9a35369e2d' HEX-> 'ECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3045022100ab464d5426cbc42e27505231a0a33f621ab974884bb5aa42e6711c5efb87bd1e0220140c79d0702267b1be0bd2d036533816c6b84cc7eaeae6f6d9314f9a35369e2d' HEX-> 'ECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3044022071e0e7afb262df3ed81cac2d5f0dd8fa03b805a0e4113aa2365918e04d68c69f022056a9d9a59ca6ea5bd0b3f382e21989a4dbdb7502eb01176bc1a41abce6d5b1ec' HEX-> 'SHA1WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3044022071e0e7afb262df3ed81cac2d5f0dd8fa03b805a0e4113aa2365918e04d68c69f022056a9d9a59ca6ea5bd0b3f382e21989a4dbdb7502eb01176bc1a41abce6d5b1ec' HEX-> 'SHA1WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '304502203839e1424afba862f2e2f84d9152ae52104abf06b63aa0d3033318113dc52560022100ae6f04427b8b7dccd3e84d13b4d0b6d30e4145f2bda05f727ad821648c47a4db' HEX-> 'SHA224WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '304502203839e1424afba862f2e2f84d9152ae52104abf06b63aa0d3033318113dc52560022100ae6f04427b8b7dccd3e84d13b4d0b6d30e4145f2bda05f727ad821648c47a4db' HEX-> 'SHA224WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '30460221008fb518a7f9fe960b8644d5af5dbb7a0a3d96ce074029c4119c254ae9c8bb9d25022100c5aed042f09246a0f4058a5ed2c646dda03dd18d0b20804e206b2c3ef513a410' HEX-> 'SHA256WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '30460221008fb518a7f9fe960b8644d5af5dbb7a0a3d96ce074029c4119c254ae9c8bb9d25022100c5aed042f09246a0f4058a5ed2c646dda03dd18d0b20804e206b2c3ef513a410' HEX-> 'SHA256WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '30460221009aaef0b41e1cb7081d73c061c489d2ee9ae0c98988db0e4de4bb163b59471921022100b1c7ca67cb59dc314b1d80ab7619125a1f1ffa832a05e46e7124ec0fb0eeeb62' HEX-> 'SHA384WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '30460221009aaef0b41e1cb7081d73c061c489d2ee9ae0c98988db0e4de4bb163b59471921022100b1c7ca67cb59dc314b1d80ab7619125a1f1ffa832a05e46e7124ec0fb0eeeb62' HEX-> 'SHA384WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3046022100b008ec1829f187ccf05641f44e2de657a7c5c8016392e9342bbfe37b000b7344022100ebfe87528ea3c94110232d007404194fd22a875f4d0ce89ccd95d38be727acdc' HEX-> 'SHA512WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3046022100b008ec1829f187ccf05641f44e2de657a7c5c8016392e9342bbfe37b000b7344022100ebfe87528ea3c94110232d007404194fd22a875f4d0ce89ccd95d38be727acdc' HEX-> 'SHA512WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '30450220602ca005831cee0ac2196abefab02fbd8ab8c45d909740e93dbd7a3fb6bcb3c002210092da65d92e1313ef334368e4f29657c71bb671026eb01901fae72c766c17ba17' HEX-> 'SHA3-224WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '30450220602ca005831cee0ac2196abefab02fbd8ab8c45d909740e93dbd7a3fb6bcb3c002210092da65d92e1313ef334368e4f29657c71bb671026eb01901fae72c766c17ba17' HEX-> 'SHA3-224WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3046022100e31f2c3f1bbb80c6e5d47eddf5bd1d60f8163fbf956d505a06b8a0dbf9c83675022100d2924145a3070f9d42a55276dfdc2439ef7ec29d9da9b5286eee75f5da9931bf' HEX-> 'SHA3-256WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3046022100e31f2c3f1bbb80c6e5d47eddf5bd1d60f8163fbf956d505a06b8a0dbf9c83675022100d2924145a3070f9d42a55276dfdc2439ef7ec29d9da9b5286eee75f5da9931bf' HEX-> 'SHA3-256WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3046022100f929197aac133b67b440a9b2aa89900d8193c66160f10a2c79182d6f8214a62a02210082873c50c4c1f46f5631fe45f14f31febe87dad8ca6aa06e64279829fa870168' HEX-> 'SHA3-384WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3046022100f929197aac133b67b440a9b2aa89900d8193c66160f10a2c79182d6f8214a62a02210082873c50c4c1f46f5631fe45f14f31febe87dad8ca6aa06e64279829fa870168' HEX-> 'SHA3-384WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3044022013d56704c18be6b58ece4e9c66e0a6aa443b2e06ccbc9a2afbd7dc0837bc8d0e02207bd5953e8fe5e077317e64fa75d385fc9213d7fe7e465a32462a00e0488b7f95' HEX-> 'SHA3-512WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3044022013d56704c18be6b58ece4e9c66e0a6aa443b2e06ccbc9a2afbd7dc0837bc8d0e02207bd5953e8fe5e077317e64fa75d385fc9213d7fe7e465a32462a00e0488b7f95' HEX-> 'SHA3-512WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3046022100a6da1e51d1fd05d6913287b88ec822045ca78aa59f8f459c4a210bd9c2ff6e8d022100a51d54a7a9d390f61f7672ec7810f6aa47f0ffbe2c5ee338bf53c2c8b691cb71' HEX-> 'RIPEMD160WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3046022100a6da1e51d1fd05d6913287b88ec822045ca78aa59f8f459c4a210bd9c2ff6e8d022100a51d54a7a9d390f61f7672ec7810f6aa47f0ffbe2c5ee338bf53c2c8b691cb71' HEX-> 'RIPEMD160WITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3046022100c82153a08744349daf22ede1f7be7d3faa21b9aa638f364db0a4caf595b0e9f20221008adff2ef8dc84ef50ba8c56f60afa3f585d9088c2743adaaa2beddd88b1b3e49' HEX-> 'sha256withecdsa' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3046022100c82153a08744349daf22ede1f7be7d3faa21b9aa638f364db0a4caf595b0e9f20221008adff2ef8dc84ef50ba8c56f60afa3f585d9088c2743adaaa2beddd88b1b3e49' HEX-> 'sha256withecdsa' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3045022014167b633ef9b4620f4c5a2a429bb1326c93769f2e6b1bf1267666d0e6d0f3d30221009164b03725590382acf35ccd59f3309f911abd4f66132d5eeb38b5fae4d0e968' HEX-> 'SHA256withECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3045022014167b633ef9b4620f4c5a2a429bb1326c93769f2e6b1bf1267666d0e6d0f3d30221009164b03725590382acf35ccd59f3309f911abd4f66132d5eeb38b5fae4d0e968' HEX-> 'SHA256withECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'abababababababababababababababababababab' HEX-> '3046022100ac3bdcde95f7f11978435d8169f6a6d0124f8ee96d1fde64c9afd8166f304c59022100e529804c99ee9a69d998d8e836991229300e7b126c0fae36361d81dc0606e7f0' HEX-> 'NONEWITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'abababababababababababababababababababababababababababababababab' HEX-> '3045022100f461f3eeabc3e27679484d3c58bcde52cc7fad964ef9a1b427202969e0f2db86022005fdb72906e6dbb109e8168e1179e4a0d15679fd4b2391fbe92fc032ffacc659' HEX-> 'NONEWITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'abababababababababababababababababababababababababababababababababababababababababababababababab' HEX-> '3046022100df110744ed836946ee7320bc8aa36e45e9f67bac7deea31da03f1f7db1fdf024022100bc58ba280a9a9f1d3c5033779c03a47e636a2b8cc1f3d6d6c3b6240d989f7f5f' HEX-> 'NONEWITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'abababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababab' HEX-> '3045022058c14f6c202a585c9473c2b5c4daa0893bf8fa9eb30feeb1759367b687b135b2022100e7e32dd583e87a43f2b5b04a63f36bdb08c3952f49254567eb75140d1e662050' HEX-> 'NONEWITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'abababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababab' HEX-> '304402206bb57add36ef0dc740547f52fe07f3201877fe73637a3e3e1d1905ad1bd9c53302207eb4462db6f34455777f0faa9d7b715022961b68c0e49469ad9473f54f97b7f8' HEX-> 'NONEWITHECDSA' { 'curve' 'secp256r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '30640230055b4ef86f5a11a8f87c719c4bda42c9ff0fc1548f5c88812ecb329151a90993e8587dd56f820e324a8d90453b377ad2023028af9930393a5733c828cc655d42ccc95a6cf7d4a3faf5047f69d1ddc08ded3b07881aa7afe7958fecab57c9d8c451c8' HEX-> 'ECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '30640230055b4ef86f5a11a8f87c719c4bda42c9ff0fc1548f5c88812ecb329151a90993e8587dd56f820e324a8d90453b377ad2023028af9930393a5733c828cc655d42ccc95a6cf7d4a3faf5047f69d1ddc08ded3b07881aa7afe7958fecab57c9d8c451c8' HEX-> 'ECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3065023010f38840226b13d29394bb84f8984b15aa41719fae6fd206296964e4cadf82d84a9da513d456ad4fe2f4275f22168cf9023100af6ec7c12afa068efd6f75613f047856f0f4c5a7a39d016963eff19ee1e151403982d78789c6231e70833fd16530652d' HEX-> 'SHA1WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3065023010f38840226b13d29394bb84f8984b15aa41719fae6fd206296964e4cadf82d84a9da513d456ad4fe2f4275f22168cf9023100af6ec7c12afa068efd6f75613f047856f0f4c5a7a39d016963eff19ee1e151403982d78789c6231e70833fd16530652d' HEX-> 'SHA1WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3065023042609711e5718cb4944e5423341f5bfe9bdf79824cac09a248dd4f3efcf1ebcdcbce2ca26b40e3e66956f7b92818a584023100fd7d259febc6808139480960b4c48d434b185acd4bbe3ce4c8875c786c11ba7d8b22f5a89f74d7f9f293db1cb580690a' HEX-> 'SHA224WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3065023042609711e5718cb4944e5423341f5bfe9bdf79824cac09a248dd4f3efcf1ebcdcbce2ca26b40e3e66956f7b92818a584023100fd7d259febc6808139480960b4c48d434b185acd4bbe3ce4c8875c786c11ba7d8b22f5a89f74d7f9f293db1cb580690a' HEX-> 'SHA224WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3065023019c48d99e95db60f541f256a40c2cff1d8b00c6753848ef35e13f59d25080017a9a4ccc0f4b13f671afafbabb1f5d0d0023100c08080f7b7c65ff23485c83d60e7be537e851eddb11c554e2580d33ff3801cdd22796d906f128fe009396d9b661117c9' HEX-> 'SHA256WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3065023019c48d99e95db60f541f256a40c2cff1d8b00c6753848ef35e13f59d25080017a9a4ccc0f4b13f671afafbabb1f5d0d0023100c08080f7b7c65ff23485c83d60e7be537e851eddb11c554e2580d33ff3801cdd22796d906f128fe009396d9b661117c9' HEX-> 'SHA256WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3064023026ab70cd205fdc096384ca87c507da24b87b72e5bfefc452d1ddf331b6dbd3545ef2bfa57fc7c4001ff9baa3be4df3ac0230117c8ce55c49b1c8a14106bf8932171ba5cf71111342ecfdae2c686ad6a6bcbd0e55ae81f27361c3d6bd2cd821f7396c' HEX-> 'SHA384WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3064023026ab70cd205fdc096384ca87c507da24b87b72e5bfefc452d1ddf331b6dbd3545ef2bfa57fc7c4001ff9baa3be4df3ac0230117c8ce55c49b1c8a14106bf8932171ba5cf71111342ecfdae2c686ad6a6bcbd0e55ae81f27361c3d6bd2cd821f7396c' HEX-> 'SHA384WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3064023011f8cbe8fd13c33e090750201a4694e64359c5b9dca229e21b2db05705ab1357fc857d756491e0baa4b3b8155ca9dc9f0230345cc965906b3e8e35e5870dc8673f14f73a135a458c4ad40f8c404e8bc510db762089d3ca05c86186862cbe8ad02b63' HEX-> 'SHA512WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3064023011f8cbe8fd13c33e090750201a4694e64359c5b9dca229e21b2db05705ab1357fc857d756491e0baa4b3b8155ca9dc9f0230345cc965906b3e8e35e5870dc8673f14f73a135a458c4ad40f8c404e8bc510db762089d3ca05c86186862cbe8ad02b63' HEX-> 'SHA512WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '306402301db109a3d6bc56f55c217325a900c4862faa9f882ad179e4e61b37cf05d2f4258ff32e318b152e2821d1b39540a23ca702300b2fd25010997e01f74ac54eceffcbaba8e4688eaad6bb9b8aac54b46eaf45dc19a1287deb9b218ae4c30c2dd966978c' HEX-> 'SHA3-224WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '306402301db109a3d6bc56f55c217325a900c4862faa9f882ad179e4e61b37cf05d2f4258ff32e318b152e2821d1b39540a23ca702300b2fd25010997e01f74ac54eceffcbaba8e4688eaad6bb9b8aac54b46eaf45dc19a1287deb9b218ae4c30c2dd966978c' HEX-> 'SHA3-224WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3065023100903df341b2f0697d18571ca84178701b6fb5ab70d5dc0b16ab0706d3882fef326ffe8faa93541f5a2a636cf9006e0654023057988e0e1b28affa54b2240e3e3a48bd57ecd348484cabe8b8cb79feab084efb97e517f001efc7a412f773d6367aad51' HEX-> 'SHA3-256WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3065023100903df341b2f0697d18571ca84178701b6fb5ab70d5dc0b16ab0706d3882fef326ffe8faa93541f5a2a636cf9006e0654023057988e0e1b28affa54b2240e3e3a48bd57ecd348484cabe8b8cb79feab084efb97e517f001efc7a412f773d6367aad51' HEX-> 'SHA3-256WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '30640230052f2081cbe8c747da3ef8805696ff44edf3ec4f45dfb5ae197ac53d74300cfeca95db8e2473fa981b4a87042659ba4902300e85fdc177ca1e331d663c9479fdd8b8329a1d7e6fe73eff7729e19e2d1830ac1f2f52d5c11de3549918f7245699b918' HEX-> 'SHA3-384WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '30640230052f2081cbe8c747da3ef8805696ff44edf3ec4f45dfb5ae197ac53d74300cfeca95db8e2473fa981b4a87042659ba4902300e85fdc177ca1e331d663c9479fdd8b8329a1d7e6fe73eff7729e19e2d1830ac1f2f52d5c11de3549918f7245699b918' HEX-> 'SHA3-384WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '306502301d437c98a707407faec39cbde802821a09355d691b72028e1d6f7dc14cd1ae1dc2bc0bd324ac1a889af578875a8d0f57023100f5d12fbb7163e3c04c18d814467ca48493e1a1f9d00ec7680d60d2460a3e07576104e5a847726a9e3b49d42e2583a0c5' HEX-> 'SHA3-512WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '306502301d437c98a707407faec39cbde802821a09355d691b72028e1d6f7dc14cd1ae1dc2bc0bd324ac1a889af578875a8d0f57023100f5d12fbb7163e3c04c18d814467ca48493e1a1f9d00ec7680d60d2460a3e07576104e5a847726a9e3b49d42e2583a0c5' HEX-> 'SHA3-512WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3066023100ad8165dcf94098897750740f3de5c30ac9abb59d1fa0b3164e5c241a24429c42e6f20402cf8efffafca8a475ee8ef74a023100950319693f8d3ce1e80e4440f4b46cdb8c1ada00fb79682e82c1b6762d1c59139bd8e5ae4737b1db6c4e90fd28a07551' HEX-> 'RIPEMD160WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3066023100ad8165dcf94098897750740f3de5c30ac9abb59d1fa0b3164e5c241a24429c42e6f20402cf8efffafca8a475ee8ef74a023100950319693f8d3ce1e80e4440f4b46cdb8c1ada00fb79682e82c1b6762d1c59139bd8e5ae4737b1db6c4e90fd28a07551' HEX-> 'RIPEMD160WITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '30640230544b70a6df4241fbef8c79d19bfffc2e7f2347e15d3b1d87de8b3d0ce86f120d8e6b0a222ff47e8aa585342ff944d2400230377e1b62757ffb0b46b390f5a055b7b4d55bb07887bef35fb7d9ac5e2be3c550b75c86b0e461dbd70e6e375f08f50869' HEX-> 'sha256withecdsa' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '30640230544b70a6df4241fbef8c79d19bfffc2e7f2347e15d3b1d87de8b3d0ce86f120d8e6b0a222ff47e8aa585342ff944d2400230377e1b62757ffb0b46b390f5a055b7b4d55bb07887bef35fb7d9ac5e2be3c550b75c86b0e461dbd70e6e375f08f50869' HEX-> 'sha256withecdsa' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3064023073d4022e298db9644f002e849b097d89a4f4aa944f8832a079d3e65c28e38f80419912e1421cc3a71c71f862a94da30202301e99db35a67d8cede2e4168ef8cf0d6e1581c4ad6be44a2256b2d83e56958eedc07a97df739eb733a0777d8798b6bc5e' HEX-> 'SHA256withECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3064023073d4022e298db9644f002e849b097d89a4f4aa944f8832a079d3e65c28e38f80419912e1421cc3a71c71f862a94da30202301e99db35a67d8cede2e4168ef8cf0d6e1581c4ad6be44a2256b2d83e56958eedc07a97df739eb733a0777d8798b6bc5e' HEX-> 'SHA256withECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'abababababababababababababababababababab' HEX-> '3064023068acc804b10f723699a5b7a3cf71e622ae5dcdc1c8bc7021f0ec96b2399f357f18e9a27a8f0d3f2230589affae8bc64202303d58256f07a55b66f4811592b32b4b26631161a098d3367d1af2ba87cb380924571055368cc37952b1756b4b9baa8298' HEX-> 'NONEWITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'abababababababababababababababababababababababababababababababab' HEX-> '3066023100839d4b4b4d7b31d04ef826d4cee8a481d40a7930ebafda7945a686ecb8204f2c7cde5343acb26c9ca4f2eb7aa2a79e9d023100e433575ba91f47712de9b9088ca02bd699b7d70713f14c74454315685c6fada8d0e0c5767c24daf911e6ebb53727f387' HEX-> 'NONEWITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'abababababababababababababababababababababababababababababababababababababababababababababababab' HEX-> '3066023100cb27c5b3120f0d34728cb22d0accd8f16f2d324c28760bce1018d5e12fc94defcafa704bb4dd83873af160a58adca0ec023100dd501a64ec0038d8351fab0eb13dd36cf51b539c2e0e84664af2edaec9fcaea568ca47dfa6c7b2d20e09fe3ead517620' HEX-> 'NONEWITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'abababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababab' HEX-> '3065023035db7fc3b26e2d57c3f83febaeb4fe1981a015a6cd3760bd278c830e4bb5eb2ba489f4140fbab19e27bbf8c0dd0c67d1023100e72ec3f480c25c0ff9ee7069cf61c1a55d0d5e6d1116b0c71e0f5860fe13a03c3386ba90bf0e245ae3fe33c3537f71d9' HEX-> 'NONEWITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'abababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababab' HEX-> '30660231009057f35598aa092f884c59f790233330fcbb82cb56ff809b1ebbd8b2fb295d2e71d837c5df36fbc2323c7caab1e710de02310082f720facd9ca7a5d89177ab96e18b15b09d89a6bf36d5e8a9d4677c2b178b70385babc9b7dde122fc83d5ac2e3222b1' HEX-> 'NONEWITHECDSA' { 'curve' 'secp384r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '308188024201fa7caad8c7a333cfa4212a42cfe5b593d454b84f31ff22fc1232c852e4d178e049116f8a200f4fbce710891277d35106c4c36c7b210fc179fd1a2c057468a258d1024201ac8d72d0a360c0e9203a45649bebc7d90ea58690b1d9b72b3b02a9a7a215a1b2fcb7a22baa9aff64918dd9150899bc3568436f153a016b258bbbe4389532351951' HEX-> 'ECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '308188024201fa7caad8c7a333cfa4212a42cfe5b593d454b84f31ff22fc1232c852e4d178e049116f8a200f4fbce710891277d35106c4c36c7b210fc179fd1a2c057468a258d1024201ac8d72d0a360c0e9203a45649bebc7d90ea58690b1d9b72b3b02a9a7a215a1b2fcb7a22baa9aff64918dd9150899bc3568436f153a016b258bbbe4389532351951' HEX-> 'ECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '308188024200dcf3cec24de5140645a4f83f1902263b6ff47275d1ac553e201d0e2c06bc6e4d53d3e99f9a5e6a9fe0264bfe857e0fac9e843202ae78fa00d5d2f75d22ee7810a6024200b71230734cb88b484efefb96db836566d8de25bf2a7a9cb536ac6a9df6666af6b8ffc396c64093e6fe020765d96d281ca3a8c3970eb664327389f50721173e70c9' HEX-> 'SHA1WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '308188024200dcf3cec24de5140645a4f83f1902263b6ff47275d1ac553e201d0e2c06bc6e4d53d3e99f9a5e6a9fe0264bfe857e0fac9e843202ae78fa00d5d2f75d22ee7810a6024200b71230734cb88b484efefb96db836566d8de25bf2a7a9cb536ac6a9df6666af6b8ffc396c64093e6fe020765d96d281ca3a8c3970eb664327389f50721173e70c9' HEX-> 'SHA1WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '30818702420189c1ec67f5d445590cab6b836628394badecf7443809762d96ced8a928b5e7c00b25f44328231555fb48793a77e79d2bf3b3afaaba98f3153c961a666463fbe74c024115965940804880295a74da3a5dab914f95abc85f72080756dc5eddaf031aa0cc42a1eca1cacc31bb46a6506e42153ca106779dee353b21045519872320a6c19326' HEX-> 'SHA224WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '30818702420189c1ec67f5d445590cab6b836628394badecf7443809762d96ced8a928b5e7c00b25f44328231555fb48793a77e79d2bf3b3afaaba98f3153c961a666463fbe74c024115965940804880295a74da3a5dab914f95abc85f72080756dc5eddaf031aa0cc42a1eca1cacc31bb46a6506e42153ca106779dee353b21045519872320a6c19326' HEX-> 'SHA224WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '308187024200f096c18c6b78524397e2036db38dcaea98eb03d0062eee29941b5b99d737d2a693970b0319eb09fce80153baeaebd4358fbadd46550f1499302a20f1d34c55e3620241729bd8b0380cb606bdc135e1779c197acb6385d74917fcb53782942d77d6ff08bbdf89f0f66875c631b463b2c10e2ee421ded59e8a3359dec8ae3c7bd3f7c1969d' HEX-> 'SHA256WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '308187024200f096c18c6b78524397e2036db38dcaea98eb03d0062eee29941b5b99d737d2a693970b0319eb09fce80153baeaebd4358fbadd46550f1499302a20f1d34c55e3620241729bd8b0380cb606bdc135e1779c197acb6385d74917fcb53782942d77d6ff08bbdf89f0f66875c631b463b2c10e2ee421ded59e8a3359dec8ae3c7bd3f7c1969d' HEX-> 'SHA256WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3081870242018043f4e7b11919b096c98b531d2a89e134f630751fead73c37824c8f6225d9ff05e07034fc16145d9451f442eadb4c0711b76d9d40a857c7c73f02fc1d85f751f602411471ce0d43ef69d2a38edb5be7fa4deebb7e26445809bab35748a82526d9f11fe051eb96c6aa6007c26151c3d61b503e78f08d89191396aef1790a97ce4020b98f' HEX-> 'SHA384WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '3081870242018043f4e7b11919b096c98b531d2a89e134f630751fead73c37824c8f6225d9ff05e07034fc16145d9451f442eadb4c0711b76d9d40a857c7c73f02fc1d85f751f602411471ce0d43ef69d2a38edb5be7fa4deebb7e26445809bab35748a82526d9f11fe051eb96c6aa6007c26151c3d61b503e78f08d89191396aef1790a97ce4020b98f' HEX-> 'SHA384WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '308188024201e106fa26f2347c9a494afe4ccf3434d5dbf9b538962cb84f36dcb6da606b7c7b782819a9e0811a74c470679ef61a4c0c76a3fed7c04990996541b22856a479aa93024201eb4d0ab189e9baa907804b44c52ba249e83f244e958811725d22bce6a1861cd83f6d3426e89ec0871c6322b491b3e94117b69fb56d649695ebb46b27b86ee64fd0' HEX-> 'SHA512WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '308188024201e106fa26f2347c9a494afe4ccf3434d5dbf9b538962cb84f36dcb6da606b7c7b782819a9e0811a74c470679ef61a4c0c76a3fed7c04990996541b22856a479aa93024201eb4d0ab189e9baa907804b44c52ba249e83f244e958811725d22bce6a1861cd83f6d3426e89ec0871c6322b491b3e94117b69fb56d649695ebb46b27b86ee64fd0' HEX-> 'SHA512WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '308186024100d691ccb260141fb854b278fd09b0a691074ac31e7af4dfa8f659ffd58921896165f06aa408fb31fc49d26855b1896ab3aab49c0e19766481a7619787f6d40d0102417d7c90540bef8c07a9135eb559c601310d542dce288a5daf3942ccfefbcf5f8cd5295b2e89637cc4c4741ea91870ed9e8700d5ad7248b753e0e0a90b5de82803d5' HEX-> 'SHA3-224WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '308186024100d691ccb260141fb854b278fd09b0a691074ac31e7af4dfa8f659ffd58921896165f06aa408fb31fc49d26855b1896ab3aab49c0e19766481a7619787f6d40d0102417d7c90540bef8c07a9135eb559c601310d542dce288a5daf3942ccfefbcf5f8cd5295b2e89637cc4c4741ea91870ed9e8700d5ad7248b753e0e0a90b5de82803d5' HEX-> 'SHA3-224WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '308186024177d8dd1b22eed67d2332d327f839bc01f7abaac92b7b397033e78d9d5257123adc69bc931f5ee13d90e74d6833e447b4f1c33bc6a97c5872bae928e698306a3f6402415af92d16993ae49f122f3486e6c5f22ab1c799a860648baa0eff7cadc5753e45fe86a945da19fea8087a2a09dd87fb9db9c4db261e486c72ef38a98d40e322c2b7' HEX-> 'SHA3-256WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '308186024177d8dd1b22eed67d2332d327f839bc01f7abaac92b7b397033e78d9d5257123adc69bc931f5ee13d90e74d6833e447b4f1c33bc6a97c5872bae928e698306a3f6402415af92d16993ae49f122f3486e6c5f22ab1c799a860648baa0eff7cadc5753e45fe86a945da19fea8087a2a09dd87fb9db9c4db261e486c72ef38a98d40e322c2b7' HEX-> 'SHA3-256WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '308187024200d16a9857e2a48398c649f3a6b3acd12d709889f5b6fd7302bbde638c52b491cd4b7c4042a5be6df9daf60f66e4ee45a2b64281b891742c59563b5175d3b7ea3b3002414511be1ee500e1d1514c6cc07899976825a97f6341878a4c887084978157c30adf4c68cdbe16fa079290790eb94702d8704d33248fcf3f6b4dd404d254ccea4fac' HEX-> 'SHA3-384WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '308187024200d16a9857e2a48398c649f3a6b3acd12d709889f5b6fd7302bbde638c52b491cd4b7c4042a5be6df9daf60f66e4ee45a2b64281b891742c59563b5175d3b7ea3b3002414511be1ee500e1d1514c6cc07899976825a97f6341878a4c887084978157c30adf4c68cdbe16fa079290790eb94702d8704d33248fcf3f6b4dd404d254ccea4fac' HEX-> 'SHA3-384WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '308187024200da76127c7c97a4651c74f07c3ffafdf7ac1b281d7b8bada398cf8446cfb220c5071cb3374b31a296f6c9aa1953c6f1034a717c09d4c236d8ab3ede520af0da06e70241262ff82940b478e9fc3a2aee4e9b06eb68098b2dac4afcb282a985cb6b4e313da3b7041ac2128ff3343430ce97b0922e8fa53b51718058e71c513f0d87d7d1b24d' HEX-> 'SHA3-512WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '308187024200da76127c7c97a4651c74f07c3ffafdf7ac1b281d7b8bada398cf8446cfb220c5071cb3374b31a296f6c9aa1953c6f1034a717c09d4c236d8ab3ede520af0da06e70241262ff82940b478e9fc3a2aee4e9b06eb68098b2dac4afcb282a985cb6b4e313da3b7041ac2128ff3343430ce97b0922e8fa53b51718058e71c513f0d87d7d1b24d' HEX-> 'SHA3-512WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '30818602412592d02069fa32bb426b41c82d3416d33bc69fc244c93ec9cb70f4332f016eb7aef1e7e81fb5ba0b484d0b7ef2410dd44018789f9b15890c8b55cfb0cf9f602ede02413a2ef52e49204fd09d0abb09788b635f012db1f0ba0873f05c7c7e8f8d2abffc4051033ad017d3fb7ccc53523ef5b6cd4ed30a37725c329d59c3edf95479fa382f' HEX-> 'RIPEMD160WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '30818602412592d02069fa32bb426b41c82d3416d33bc69fc244c93ec9cb70f4332f016eb7aef1e7e81fb5ba0b484d0b7ef2410dd44018789f9b15890c8b55cfb0cf9f602ede02413a2ef52e49204fd09d0abb09788b635f012db1f0ba0873f05c7c7e8f8d2abffc4051033ad017d3fb7ccc53523ef5b6cd4ed30a37725c329d59c3edf95479fa382f' HEX-> 'RIPEMD160WITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '308187024200f8df620c8edf3d1c8784c806ec5d5ce8c23d08bfb8f1a980aa043019371017e07b7fea09b2e856b12a605ced626d22c9076db4acdbab029042085506aefbfa690902415b1a2ba316458dd65de975e838b3ad51faccd7753e94541e37abba657404b95fe7ff60d8c82cf8bd039a81fc2e8eeee183137690a07a6eaa2f59101ac74f0260cb' HEX-> 'sha256withecdsa' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '308187024200f8df620c8edf3d1c8784c806ec5d5ce8c23d08bfb8f1a980aa043019371017e07b7fea09b2e856b12a605ced626d22c9076db4acdbab029042085506aefbfa690902415b1a2ba316458dd65de975e838b3ad51faccd7753e94541e37abba657404b95fe7ff60d8c82cf8bd039a81fc2e8eeee183137690a07a6eaa2f59101ac74f0260cb' HEX-> 'sha256withecdsa' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '308187024201d71d350e210a55154f1fbbfbcdd20cac757057dffe284d287d57a102ef6d83f756c0ee27cd1a5e13a40680e7f178ad68bfd6034fdf656b9e9f3a59297f008dc86702415d09bebc3e65114ec55f04a47fb8f39363bcccb1def401ef5a913dd8e183e929fa7dac299138d2f69b49814b816009308e768aac2ef2ed6066cbcfb3e172532619' HEX-> 'SHA256withECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hellp' 'UTF-8' ->BYTES '308187024201d71d350e210a55154f1fbbfbcdd20cac757057dffe284d287d57a102ef6d83f756c0ee27cd1a5e13a40680e7f178ad68bfd6034fdf656b9e9f3a59297f008dc86702415d09bebc3e65114ec55f04a47fb8f39363bcccb1def401ef5a913dd8e183e929fa7dac299138d2f69b49814b816009308e768aac2ef2ed6066cbcfb3e172532619' HEX-> 'SHA256withECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'abababababababababababababababababababab' HEX-> '308186024179422b5e154b878b9027102eb4189cbb7b29dfb9f6c34e737167005277a2b75939c6b0ad8049876262241808364d50bed77ad0e3379702a972870f4d7b1f955bd102415519e338af8ef9eaeca6853e4ff9a155e536070e3f66a407c1650474b126f81a824c0868aebe0577ab0eeec283373565a23c5a84c6352129c9c7d1618b34f664c6' HEX-> 'NONEWITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'abababababababababababababababababababababababababababababababab' HEX-> '308188024201344a67919a16705a2952b42dd980069e0705bc32f49654c0b1d8a8d93cd10ade93ef423a51323f5b8f8f7d95b9e9d5626ab1fab536867d05d478dfd1396235a29b024201718d2469528b0831d78c90fb05eb54221c90a99e09a5d8b587787a58e4e2dd828d9351890436a13f2298d61ee05a93b2d702c8ecb11fa32cf22977f6a9dd62083e' HEX-> 'NONEWITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'abababababababababababababababababababababababababababababababababababababababababababababababab' HEX-> '3081870242014f3e6409c5ac62b599316a21f8bd390cc5df6256cdbedec3147efcf4e735ddf994e0f13ed6e716cba36aec845e1f425923a3e155780732818f5e0f7f22d5c011d4024176dc4fe8777307c4c3283bb9152bcfa8d820a7b83cd026a7f985e5824f54fefdfd0404b235297df8b680f23d371abb042f180c423bad677421cb1b751e24ad9d99' HEX-> 'NONEWITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'abababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababab' HEX-> '308187024200f4d88d61097e42cab9eaf91aa9fd882fef0af910973f8216faf83aa75a64d0a3f9d15201ba3e172300f5cfbf63edb7122c2f5e5b8243eb454a7fd05931ad8733fc024130e43a1f8487878465050e45cd49d6092994173903b5967f37502cdaf7444cc6d8c49efd7646fa50bb7c24796a9be856f7342b8821268eca50a9c9e5b3eaff4774' HEX-> 'NONEWITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'abababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababab' HEX-> '308188024200c24fdac889b381fb22412f0dd6f476bc47efd9ab01be7da415ce536e5ef06d45c8168febb8f66f0dd426142f08f5d6e05324825360f65485714aed366b74def0cf0242009b7b826b1a7d0fc9e7ecdad86d6d1bcd3a67b18d96b4fd3cc58aae3595d49653132c3817083cc278334186a21d45d3b31085279194ee7fa1f8f0d0eda8438c73ab' HEX-> 'NONEWITHECDSA' { 'curve' 'secp521r1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[true]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3044022073e3e1d72275ec898da8eff1b78453dd42ea22b669283fc6fde7f95e136a6379022020bf54ddcc58136c872ab0295ec19f314235b10cfc33c98ef60629aaa83faeaf' HEX-> 'SHA256WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3006020101020101' HEX-> 'SHA256WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3006020100020101' HEX-> 'SHA256WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3006020101020100' HEX-> 'SHA256WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '300602010102010a' HEX-> 'SHA256WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY STACKTOLIST ->JSON",
        r"[false]",
    ),
];

const SIGNATURES_ERRORS: &[(&str, &str)] = &[
    (r"ECSIGN", r"Empty stack."),
    (r"1 ECSIGN", r"ECSIGN expects an ECC private key."),
    (
        r"{ 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECSIGN",
        r"ECSIGN expects an ECC private key.",
    ),
    (
        r"1 { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECSIGN",
        r"ECSIGN expects an algorithm name.",
    ),
    (
        r"'a' 1 { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECSIGN",
        r"ECSIGN expects an algorithm name.",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES 'XX' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECSIGN",
        r"ECSIGN error signing content. (no such algorithm: XX for provider BC)",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECSIGN",
        r"ECSIGN error signing content. (no such algorithm:  for provider BC)",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES 'SHA-1WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECSIGN",
        r"ECSIGN error signing content. (no such algorithm: SHA-1WITHECDSA for provider BC)",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES 'ECDSAWITHSHA256' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECSIGN",
        r"ECSIGN error signing content. (no such algorithm: ECDSAWITHSHA256 for provider BC)",
    ),
    (r"ECVERIFY", r"Empty stack."),
    (r"1 ECVERIFY", r"ECVERIFY expects an ECC public key."),
    (
        r"{ 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECVERIFY",
        r"ECVERIFY expects an ECC public key.",
    ),
    (
        r"1 'SHA256WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY",
        r"ECVERIFY expects a signature.",
    ),
    (
        r"'a' 1 'SHA256WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY",
        r"ECVERIFY expects a signature.",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES 'zz' 'UTF-8' ->BYTES 'SHA256WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY",
        r"ECVERIFY error while verifying signature. (error decoding signature bytes.)",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '' HEX-> 'SHA256WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY",
        r"ECVERIFY error while verifying signature. (error decoding signature bytes.)",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3006020101020101' HEX-> 'XX' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY",
        r"ECVERIFY error while verifying signature. (no such algorithm: XX for provider BC)",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '30' HEX-> 'SHA256WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY",
        r"ECVERIFY error while verifying signature. (error decoding signature bytes.)",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '3080' HEX-> 'SHA256WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY",
        r"ECVERIFY error while verifying signature. (error decoding signature bytes.)",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '300602010002010' HEX-> 'SHA256WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY",
        r"ECVERIFY error while verifying signature. (error decoding signature bytes.)",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES '30260220ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff020400000001' HEX-> 'SHA256WITHECDSA' { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC ECVERIFY",
        r"ECVERIFY error while verifying signature. (error decoding signature bytes.)",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES 'SHA256WITHECDSA' { 'curve' 'secp256k1' 'd' '0' } ECPRIVATE ECSIGN",
        r"Scalar is not in the interval [1, n - 1]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES 'SHA256WITHECDSA' { 'curve' 'secp256k1' 'd' '-1' } ECPRIVATE ECSIGN",
        r"Scalar is not in the interval [1, n - 1]",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES 'XX' { 'curve' 'secp256k1' 'd' '0' } ECPRIVATE ECSIGN",
        r"ECSIGN error signing content. (no such algorithm: XX for provider BC)",
    ),
    (
        r"'hello' 'UTF-8' ->BYTES 'SHA256WITHECDSA' { 'curve' 'secp256k1' 'd' '0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141' } ECPRIVATE ECSIGN",
        r"Scalar is not in the interval [1, n - 1]",
    ),
];

const RECOVER_VALUES: &[(&str, &str)] = &[
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX->  } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '98637885874477801065143305433999014093738369893398987228671557692459671234841' 's' '61299406769190695069821587140298973897101039469537835453780354414048726019914'  } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX-> 'even' T } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '98637885874477801065143305433999014093738369893398987228671557692459671234841' 's' '61299406769190695069821587140298973897101039469537835453780354414048726019914' 'even' T } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX-> 'even' F } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629","04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '98637885874477801065143305433999014093738369893398987228671557692459671234841' 's' '61299406769190695069821587140298973897101039469537835453780354414048726019914' 'even' F } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629","04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX-> 'even' 'x' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '98637885874477801065143305433999014093738369893398987228671557692459671234841' 's' '61299406769190695069821587140298973897101039469537835453780354414048726019914' 'even' 'x' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX-> 'i' 0 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '98637885874477801065143305433999014093738369893398987228671557692459671234841' 's' '61299406769190695069821587140298973897101039469537835453780354414048726019914' 'i' 0 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX-> 'i' 1 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '98637885874477801065143305433999014093738369893398987228671557692459671234841' 's' '61299406769190695069821587140298973897101039469537835453780354414048726019914' 'i' 1 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX-> 'i' -1 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '98637885874477801065143305433999014093738369893398987228671557692459671234841' 's' '61299406769190695069821587140298973897101039469537835453780354414048726019914' 'i' -1 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX-> 'i' 5 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '98637885874477801065143305433999014093738369893398987228671557692459671234841' 's' '61299406769190695069821587140298973897101039469537835453780354414048726019914' 'i' 5 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX-> 'i' 1.5 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '98637885874477801065143305433999014093738369893398987228671557692459671234841' 's' '61299406769190695069821587140298973897101039469537835453780354414048726019914' 'i' 1.5 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX-> 'coords' T } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0xda1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a2919","0x7621782fc44e6ba6b8b0dd2a3f2ace42daa3925912e1cdc212002c361e72009c","04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0"],["0xda1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a2919","0x89de87d03bb19459474f22d5c0d531bd255c6da6ed1e323dedffd3c8e18dfb93","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '98637885874477801065143305433999014093738369893398987228671557692459671234841' 's' '61299406769190695069821587140298973897101039469537835453780354414048726019914' 'coords' T } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0xda1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a2919","0x7621782fc44e6ba6b8b0dd2a3f2ace42daa3925912e1cdc212002c361e72009c","04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0"],["0xda1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a2919","0x89de87d03bb19459474f22d5c0d531bd255c6da6ed1e323dedffd3c8e18dfb93","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX-> 'coords' F } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '98637885874477801065143305433999014093738369893398987228671557692459671234841' 's' '61299406769190695069821587140298973897101039469537835453780354414048726019914' 'coords' F } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX-> 'coords' T 'even' T } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0xda1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a2919","0x7621782fc44e6ba6b8b0dd2a3f2ace42daa3925912e1cdc212002c361e72009c","04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0"],["0xda1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a2919","0x89de87d03bb19459474f22d5c0d531bd255c6da6ed1e323dedffd3c8e18dfb93","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '98637885874477801065143305433999014093738369893398987228671557692459671234841' 's' '61299406769190695069821587140298973897101039469537835453780354414048726019914' 'coords' T 'even' T } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0xda1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a2919","0x7621782fc44e6ba6b8b0dd2a3f2ace42daa3925912e1cdc212002c361e72009c","04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0"],["0xda1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a2919","0x89de87d03bb19459474f22d5c0d531bd255c6da6ed1e323dedffd3c8e18dfb93","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX-> 'coords' T 'i' 0 } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0xda1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a2919","0x7621782fc44e6ba6b8b0dd2a3f2ace42daa3925912e1cdc212002c361e72009c","04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0"],["0xda1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a2919","0x89de87d03bb19459474f22d5c0d531bd255c6da6ed1e323dedffd3c8e18dfb93","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '98637885874477801065143305433999014093738369893398987228671557692459671234841' 's' '61299406769190695069821587140298973897101039469537835453780354414048726019914' 'coords' T 'i' 0 } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0xda1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a2919","0x7621782fc44e6ba6b8b0dd2a3f2ace42daa3925912e1cdc212002c361e72009c","04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0"],["0xda1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a2919","0x89de87d03bb19459474f22d5c0d531bd255c6da6ed1e323dedffd3c8e18dfb93","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX-> 'coords' T 'i' 1 } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '98637885874477801065143305433999014093738369893398987228671557692459671234841' 's' '61299406769190695069821587140298973897101039469537835453780354414048726019914' 'coords' T 'i' 1 } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '0xda1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a2919' 's' '0X87863C1637BCAE69EE0F63332332A6AAB4A7F0E648DFC09D192C1F8C1688EF4A' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0","04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '1' 's' '1' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04b3697a2ca6d79a06e27286363d72ee6647238fbe6c76355a443281036ea156c4b566c679063bde191061520d80d2ebaf7635ca139264527aea963c61855e6b7c","0491e820dcbcee39aa85b3d9e7380f2b219ec33e5c5c7cc4ff5f09c58572afc78e480b19038c75d8f65886a75c97a217bffd94f8aa0ae89b7145e8e457776e5391"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '5' 's' '7' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '-5' 's' '1' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04dc8854e956b17031c8f5c2f1a67717354fabaecf7b79f5ccb3e51a217664df8f4113358263a590973e21ac22256cf64c07a4c5c0a4c18e5ade236adf1acf758b","04fe5932b1011d5ca63d8ae8af5cc321ddb8e0736fc4d5f7d29af5a15b0c5026d6591bd098da170498687a393cb86cbd54500d4bbca8e25a9b12c10676fb225af9"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '2037035976334486086268445688409378161051468393665936250636140449354381299763336706183397376' 's' '1' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '98637885874477801065143305433999014093738369893398987228671557692459671234841' 's' '0' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["044ecf7bc7ffc560bc99ea073e9e715bb07eb3c283d31173d37237f5e696b620f597a285531717801cf7d93814a6b01dd2a417b8ebbf8611bf06bf3fcc9fd91bf9"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '98637885874477801065143305433999014093738369893398987228671557692459671234841' 's' '-61299406769190695069821587140298973897101039469537835453780354414048726019914' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04643be9237a1a9878c9d863496a531279f1b7bf31cdd2c9c7311962cdb4d6135d8c097fc1e20068fca4d50ba87d6371f1f101c6842f62143c17a3897fc29c3629","04ff991d61bcf0134414d5e51f5b8c75bf2b2bd684c022c93e8309f35d306d797f1e2fe33e78917766ee98214fe181d67b46b609d4a5a2f69a237108b2fc512ae0"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '1951311313671536927842279065573005539746058124543648011859992901819069745473817' 's' '61299406769190695069821587140298973897101039469537835453780354414048726019914' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '98637885874477801065143305433999014093738369893398987228671557692459671234841' 's' '77706229784598973038711726251815602892517991174659526467949757172295303021067198306853398791454498607857664' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04c4f8e95ff8ec295b1d513460ac743cd11003fb3e4386ccdea6da603bbd5087f98ed100d613f737f1cfda56cd48b634e2bbdb1b1f70bca1d017e564b54572abc5","045b1cac2d6d5b7dc09ed4ceef4ee77a7ad5bd0cac8a2cfbe214b8a695a792a65fe720658e8e8552c2d988b05bb3fc4ca8041ea4867401f92b4ea55031c47fe865"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' '' 'UTF-8' ->BYTES 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04b4fe52805958a71865f7b35ed3ab446940da3e9304832e726e64b157e99386d71bb89a1573aa6130b35dee456a5a5ef9f5cd4fcc4e866ad28d0631ec4898344c","04b4fe52805958a71865f7b35ed3ab446940da3e9304832e726e64b157e99386d7e44765ea8c559ecf4ca211ba95a5a1060a32b033b179952d72f9ce12b767c7e3"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'ab' HEX-> 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04bacfe5343a3a893f1032a3cb20c9db8c197e13f3e056ed23a48aede782cee96d586f7bd0d6b9064fb6ab7c86e357e35c86ac0872a8c81933483f8d092e86b4c7","04458c817ba97fd3321ce850ab16b1734ca25b19761b36b945aa2f4d5061291543d97cb4a828c54e83d518ce97570b293bd79183e34f4417e54b62c81b0ed5bb7f"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'abababababababababababababababababababab' HEX-> 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04eece6a17253c22263b84748146331ae7f322dab2ef5f384b08942c31c2b1b1b8939d9c246d5c44548a7eadcd2e542588f0efe2fb0d1614c349e4b7c8304d320c","045584063975d8393867c988c896b55d10eb8028968f9f5cf92fd901c94da4aba3c8189bffe04ccbe4d2bf366b6257541dbf1105ba35cddd5a8cf9bf738b9ab965"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'abababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababab' HEX-> 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04fd63cee28b0b89dc6e38f79a32a9e14a6287de54c6e4e170e73365cfa21f51adb745f46d81867391649926e3e86691a7540c823e2072345303c66de24691069f","043b87509f5c9f02ee4a53be4f76f39a430e9b5f18057deb276e159177287c0c5b2ebb10b6a3498eb54cb1b7eb6e6c20fe5010bee194b9dbb718e772c0a4740f83"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff' HEX-> 'sig' '3046022100da1310a1c3a0a1c9223645a58dc2a9bb8b5dafdc72f5e549e45b627c093a291902210087863c1637bcae69ee0f63332332a6aab4a7f0e648dfc09d192c1f8c1688ef4a' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04813e18cd508ca6e8c3452886767a34b3141b309a9acb2beef7074ca06c64c57fa79697bcb4b4262f246e2bdead0c748b0c9378809915f8f7800700dac3ff9f98","04513860bcc60d03b32600d5b211bb865afec6b88b814670ced133a4c161db9dd8651050662f8e721ab60e36e817cb541b50524431aafb6c5a9389056702146dbf"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX->  } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '89410797969234023712849649731521084652633902817435377766220408987540273537578' 's' '101579768931686330037347947805828773240956106877916435365400490023841964199453'  } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX-> 'even' T } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '89410797969234023712849649731521084652633902817435377766220408987540273537578' 's' '101579768931686330037347947805828773240956106877916435365400490023841964199453' 'even' T } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX-> 'even' F } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d","04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '89410797969234023712849649731521084652633902817435377766220408987540273537578' 's' '101579768931686330037347947805828773240956106877916435365400490023841964199453' 'even' F } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d","04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX-> 'even' 'x' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '89410797969234023712849649731521084652633902817435377766220408987540273537578' 's' '101579768931686330037347947805828773240956106877916435365400490023841964199453' 'even' 'x' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX-> 'i' 0 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '89410797969234023712849649731521084652633902817435377766220408987540273537578' 's' '101579768931686330037347947805828773240956106877916435365400490023841964199453' 'i' 0 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX-> 'i' 1 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '89410797969234023712849649731521084652633902817435377766220408987540273537578' 's' '101579768931686330037347947805828773240956106877916435365400490023841964199453' 'i' 1 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX-> 'i' -1 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '89410797969234023712849649731521084652633902817435377766220408987540273537578' 's' '101579768931686330037347947805828773240956106877916435365400490023841964199453' 'i' -1 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX-> 'i' 5 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '89410797969234023712849649731521084652633902817435377766220408987540273537578' 's' '101579768931686330037347947805828773240956106877916435365400490023841964199453' 'i' 5 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX-> 'i' 1.5 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '89410797969234023712849649731521084652633902817435377766220408987540273537578' 's' '101579768931686330037347947805828773240956106877916435365400490023841964199453' 'i' 1.5 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX-> 'coords' T } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0xc5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a","0x86b3def8fdea8cebb58e0b2c823156555a78bfdd3fee0978946f732baf8a549c","04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec"],["0xc5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a","0x794c2106021573154a71f4d37dcea9aaa5874023c011f6876b908cd45075ab63","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '89410797969234023712849649731521084652633902817435377766220408987540273537578' 's' '101579768931686330037347947805828773240956106877916435365400490023841964199453' 'coords' T } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0xc5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a","0x86b3def8fdea8cebb58e0b2c823156555a78bfdd3fee0978946f732baf8a549c","04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec"],["0xc5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a","0x794c2106021573154a71f4d37dcea9aaa5874023c011f6876b908cd45075ab63","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX-> 'coords' F } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '89410797969234023712849649731521084652633902817435377766220408987540273537578' 's' '101579768931686330037347947805828773240956106877916435365400490023841964199453' 'coords' F } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX-> 'coords' T 'even' T } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0xc5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a","0x86b3def8fdea8cebb58e0b2c823156555a78bfdd3fee0978946f732baf8a549c","04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec"],["0xc5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a","0x794c2106021573154a71f4d37dcea9aaa5874023c011f6876b908cd45075ab63","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '89410797969234023712849649731521084652633902817435377766220408987540273537578' 's' '101579768931686330037347947805828773240956106877916435365400490023841964199453' 'coords' T 'even' T } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0xc5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a","0x86b3def8fdea8cebb58e0b2c823156555a78bfdd3fee0978946f732baf8a549c","04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec"],["0xc5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a","0x794c2106021573154a71f4d37dcea9aaa5874023c011f6876b908cd45075ab63","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX-> 'coords' T 'i' 0 } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0xc5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a","0x86b3def8fdea8cebb58e0b2c823156555a78bfdd3fee0978946f732baf8a549c","04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec"],["0xc5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a","0x794c2106021573154a71f4d37dcea9aaa5874023c011f6876b908cd45075ab63","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '89410797969234023712849649731521084652633902817435377766220408987540273537578' 's' '101579768931686330037347947805828773240956106877916435365400490023841964199453' 'coords' T 'i' 0 } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0xc5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a","0x86b3def8fdea8cebb58e0b2c823156555a78bfdd3fee0978946f732baf8a549c","04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec"],["0xc5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a","0x794c2106021573154a71f4d37dcea9aaa5874023c011f6876b908cd45075ab63","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX-> 'coords' T 'i' 1 } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '89410797969234023712849649731521084652633902817435377766220408987540273537578' 's' '101579768931686330037347947805828773240956106877916435365400490023841964199453' 'coords' T 'i' 1 } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '0xc5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a' 's' '0XE0941C9D117439AEAF398DD02F9ED876411CF07820A74009DD3A0102711F8E1D' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec","04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '1' 's' '1' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '5' 's' '7' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04596878e373262cd9a123fe5468ef89aaf32760b5b002f2084ead09d9188a118c9549e0e1cdb4a6a26eb1dc4cb7c2719b5ef7d439a9dca1cbfbc286baca97cd2f","040d031a24f4a5d85a62e6699e4839c38192d6cb984ea22e70e8c9f7d731ec4535edffeb76e0179b375a29a230ae91400bbff1efd17a929938d2f546af0953bbe6"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '-5' 's' '1' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '2037035976334486086268445688409378161051468393665936250636140449354381299763336706183397376' 's' '1' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '89410797969234023712849649731521084652633902817435377766220408987540273537578' 's' '0' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04a5e0a7b8c62232d99b2cdd75b7b2b727cf486494f857a6cbf3704996238aa395508d0ff2389add2ae49796f21d72c4681891c7176bd16d1273fcca932a9b37fc"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '89410797969234023712849649731521084652633902817435377766220408987540273537578' 's' '-101579768931686330037347947805828773240956106877916435365400490023841964199453' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04e5e83eec61219ae073a3fbd4ef9908f5e8f591648f5bb2e3868b09f031e6959d94d74ac556dfbcc08f4807b4f3e830834d42ff704576a8e6e15fcf06c2a03f9d","04a5832a7b2fa6f4cac80f9ba7f6b9b00edd48b441fe966bfc9bcb50168c725751d12fe62d011cde4c89d2f1ade5fc2bb577b4c48593d1e4b8f66a2c2ea73ccdec"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '1942084225766293150489985409870527610304953657467684402397541753114150347776554' 's' '101579768931686330037347947805828773240956106877916435365400490023841964199453' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '89410797969234023712849649731521084652633902817435377766220408987540273537578' 's' '128767655057297025151094326298861776955018074152173834705842752879700983540129391530508016997981142885859328' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["041023b8ff889573b37ef5f5946e60d192d434496dbb753a9a6b2949ef75787a6348b6b20e101f68c603613019278b27ea3409fc29cbfd187d0d83f21432820de8","0491c23764c227bce62210e9a61e0a8caddda2386b046c98483cb548a087db646972d8939859b787f7a1845ea5dbc8e87d1ea969d2234aeb64dd89de8aa927c2b8"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' '' 'UTF-8' ->BYTES 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04e9d81a36974f1c505ba7c68f9f34d99eef825e09cc8a7c71af671c258874529019129251084d619e4bba5309b72e62ee7c9bd1341a0e136fb266d9e16e77fc60","04e9d81a36974f1c505ba7c68f9f34d99eef825e09cc8a7c71af671c2588745290e6ed6dadf7b29e62b445acf648d19d1183642ecce5f1ec904d99261e9188039f"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'ab' HEX-> 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["045d90167230fef268f74a495e03f6fd2cd50a48b52395c77a5f5dff39d981afe1a98b9635a2afbfb8677ca3203ade592198ec641c44702c80acf63098af086867","047e37f334c344bf093e94bf49c97c45c41ee39c6ef9dd7b689d1b6feb9bee167c31752df36493b4736a7196e27120d84489c50a2bc9a225cf7e2ab0a2d4aab44f"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'abababababababababababababababababababab' HEX-> 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0430ad84b7f4561e1f0c59612980a620a99bbb1d7f89c3b90bba81336fe0021247e8cbe3f6d8d021092e1415d0bffb2beb7164a626ebd9ad0c735040ef9ee717f0","04bff4b317fb6c9981e378274b36b1a522b05e21a71229befe22cccd2758ece0312ffabbddb731bf7e424bfb6a9bcb55049efc58148741115bf6e89050f7c62dc8"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'abababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababab' HEX-> 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["041b3fee01554751b3552234ad88cb0d3ea5a18cc508b661571223b6f1056ab158716d3a0731be6274f29b4f8ae2dfac81770c2f366d5483c3355b223fbfe20284","0464ba19e9c04b0a9c65775be2354d5fbf868c56046e3f7d0f74f1ad23ea63534345c0626781e4e33309b72bbefdfd52e5b0980ae17f733d4fcbccf2f6a46fc4a7"]]"#,
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff' HEX-> 'sig' '3046022100c5acb7e1a74de7467de165f39208ddd559fa6bef77fb065c2295c846cd40a22a022100e0941c9d117439aeaf398dd02f9ed876411cf07820a74009dd3a0102711f8e1d' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04ad020ab42f11757481f82e40a4fdf65ebee05904333a07bbc512672efd1c6e101369fa875d0590df234cdfd6e41da85c2def32312f01ce1f8093f365358dff33","0411818fb6b9efc673edccab75d8a660768f5e045c76951048bd09af882307bddeaf9041c38454fd3b6a274a218eecf954b05c2274be3f7a90385531ea9a355aad"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX->  } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '24943945063431737963271902495720930825690413674484222865252688309889454790385853584849321237450927109001484454509027' 's' '5503465509634739097318685948602647949405869330028998683893235348598163135541138022263494619352700621926997535089014'  } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX-> 'even' T } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '24943945063431737963271902495720930825690413674484222865252688309889454790385853584849321237450927109001484454509027' 's' '5503465509634739097318685948602647949405869330028998683893235348598163135541138022263494619352700621926997535089014' 'even' T } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX-> 'even' F } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf","04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '24943945063431737963271902495720930825690413674484222865252688309889454790385853584849321237450927109001484454509027' 's' '5503465509634739097318685948602647949405869330028998683893235348598163135541138022263494619352700621926997535089014' 'even' F } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf","04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX-> 'even' 'x' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '24943945063431737963271902495720930825690413674484222865252688309889454790385853584849321237450927109001484454509027' 's' '5503465509634739097318685948602647949405869330028998683893235348598163135541138022263494619352700621926997535089014' 'even' 'x' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX-> 'i' 0 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '24943945063431737963271902495720930825690413674484222865252688309889454790385853584849321237450927109001484454509027' 's' '5503465509634739097318685948602647949405869330028998683893235348598163135541138022263494619352700621926997535089014' 'i' 0 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX-> 'i' 1 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '24943945063431737963271902495720930825690413674484222865252688309889454790385853584849321237450927109001484454509027' 's' '5503465509634739097318685948602647949405869330028998683893235348598163135541138022263494619352700621926997535089014' 'i' 1 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX-> 'i' -1 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '24943945063431737963271902495720930825690413674484222865252688309889454790385853584849321237450927109001484454509027' 's' '5503465509634739097318685948602647949405869330028998683893235348598163135541138022263494619352700621926997535089014' 'i' -1 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX-> 'i' 5 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '24943945063431737963271902495720930825690413674484222865252688309889454790385853584849321237450927109001484454509027' 's' '5503465509634739097318685948602647949405869330028998683893235348598163135541138022263494619352700621926997535089014' 'i' 5 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX-> 'i' 1.5 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '24943945063431737963271902495720930825690413674484222865252688309889454790385853584849321237450927109001484454509027' 's' '5503465509634739097318685948602647949405869330028998683893235348598163135541138022263494619352700621926997535089014' 'i' 1.5 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX-> 'coords' T } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0xa21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3","0xc7c3e51e36f728f32713b8252fc6d511bf34b7450dc48adb255724ba83c91b92d025f84616b7c5e26a06690cd9cfbabc","04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a"],["0xa21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3","0x383c1ae1c908d70cd8ec47dad0392aee40cb48baf23b7524daa8db457c36e46c2fda07b8e9483a1d95f996f426304543","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '24943945063431737963271902495720930825690413674484222865252688309889454790385853584849321237450927109001484454509027' 's' '5503465509634739097318685948602647949405869330028998683893235348598163135541138022263494619352700621926997535089014' 'coords' T } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0xa21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3","0xc7c3e51e36f728f32713b8252fc6d511bf34b7450dc48adb255724ba83c91b92d025f84616b7c5e26a06690cd9cfbabc","04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a"],["0xa21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3","0x383c1ae1c908d70cd8ec47dad0392aee40cb48baf23b7524daa8db457c36e46c2fda07b8e9483a1d95f996f426304543","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX-> 'coords' F } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '24943945063431737963271902495720930825690413674484222865252688309889454790385853584849321237450927109001484454509027' 's' '5503465509634739097318685948602647949405869330028998683893235348598163135541138022263494619352700621926997535089014' 'coords' F } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX-> 'coords' T 'even' T } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0xa21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3","0xc7c3e51e36f728f32713b8252fc6d511bf34b7450dc48adb255724ba83c91b92d025f84616b7c5e26a06690cd9cfbabc","04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a"],["0xa21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3","0x383c1ae1c908d70cd8ec47dad0392aee40cb48baf23b7524daa8db457c36e46c2fda07b8e9483a1d95f996f426304543","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '24943945063431737963271902495720930825690413674484222865252688309889454790385853584849321237450927109001484454509027' 's' '5503465509634739097318685948602647949405869330028998683893235348598163135541138022263494619352700621926997535089014' 'coords' T 'even' T } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0xa21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3","0xc7c3e51e36f728f32713b8252fc6d511bf34b7450dc48adb255724ba83c91b92d025f84616b7c5e26a06690cd9cfbabc","04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a"],["0xa21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3","0x383c1ae1c908d70cd8ec47dad0392aee40cb48baf23b7524daa8db457c36e46c2fda07b8e9483a1d95f996f426304543","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX-> 'coords' T 'i' 0 } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0xa21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3","0xc7c3e51e36f728f32713b8252fc6d511bf34b7450dc48adb255724ba83c91b92d025f84616b7c5e26a06690cd9cfbabc","04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a"],["0xa21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3","0x383c1ae1c908d70cd8ec47dad0392aee40cb48baf23b7524daa8db457c36e46c2fda07b8e9483a1d95f996f426304543","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '24943945063431737963271902495720930825690413674484222865252688309889454790385853584849321237450927109001484454509027' 's' '5503465509634739097318685948602647949405869330028998683893235348598163135541138022263494619352700621926997535089014' 'coords' T 'i' 0 } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0xa21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3","0xc7c3e51e36f728f32713b8252fc6d511bf34b7450dc48adb255724ba83c91b92d025f84616b7c5e26a06690cd9cfbabc","04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a"],["0xa21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3","0x383c1ae1c908d70cd8ec47dad0392aee40cb48baf23b7524daa8db457c36e46c2fda07b8e9483a1d95f996f426304543","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX-> 'coords' T 'i' 1 } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '24943945063431737963271902495720930825690413674484222865252688309889454790385853584849321237450927109001484454509027' 's' '5503465509634739097318685948602647949405869330028998683893235348598163135541138022263494619352700621926997535089014' 'coords' T 'i' 1 } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '0xa21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3' 's' '0X23C1B983184BAA4AD20E1B3D1FC7E9A1FFD6639E88BF141D5F59B158717FABDDD988CE50694214B6AF57E85EB0BDE176' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a","04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '1' 's' '1' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '5' 's' '7' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04981f89785bfed2e5e3eafc668983e8bd767ec0681569dadf414b211186e085c3a2f7a234fb5070e8b602a90405ee0096f4f5077e944b1e07066694255ab077dd2e1b54dca9faa39ca83de56dae08a4729ed7593fd93370dabeb86bc3b5c97879","04be76eda99cfb6f79ef7a4d4c4f6bd3624f113a6ade4cb26ebabafff12b8fa3e18dd35c4f78fd0306516151f39f2f63c32c6e3a062f62cacbcabb33b8885f7e81dc1712bb8a581fa23ed3a13da4372efbe5f0fe62351c132895c680816d2060c5"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '-5' 's' '1' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04be1b53939f76508ea9838c2c5cb26d0be076a4fae529e0d203d4c77f01d9db030de93ea04ef99c7e747131afb0e09d381daf5a452ab5cda57cc0c53b11502e6f2c3a1d3c839b9db56359c2d0a5c1acfe87171130c2d513a5502eef6c9ca5af97","041a5eaf7edd7c62783ec31ed49d1e66f106d108021fd0168552fe0f84cf0ed6e698b0192cf4105b629d963505ce40eb4b031011776edecd98722ac4c6fd54dbaddb334261fcb117174aa76fee71c238008c794d0d71d33aab0d7065f50d7ce9a5"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '2037035976334486086268445688409378161051468393665936250636140449354381299763336706183397376' 's' '1' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04df2e6b223ccddfecab6a4edd8e27d05b2dbad04b3917cda409d5c45f1fb1031afe73f95eb252574db80309ffb80dde6872b7c1814344345d87148011c2e86806193857a4fe3c0690443a3dc9e7c7e3d6c0bf22e5c04fc98284068597b4b5ea52","044beaae91b57ed6d833e3573e51514c69840579d6c967567109885a554e5ac5ea2f705ed26e180ac5b6f941c6a5658022c00a5945d4852286f09fcf6af2e12be2531565792aa021335871e4124e6eb4988abcf308675ccc11385bec06b466a5f7"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '24943945063431737963271902495720930825690413674484222865252688309889454790385853584849321237450927109001484454509027' 's' '0' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["040c8035264e3b98319859f0afe156764d87a8ba6fa8dad471ab61c2a26a41c13fe1df46c9adbb5003b4f1083fd991909092deedb4886cae02ca82cda667ae11babfe77ca56f298851fcce5b4950bead165e457f044198308bd29491b8330215ec"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '24943945063431737963271902495720930825690413674484222865252688309889454790385853584849321237450927109001484454509027' 's' '-5503465509634739097318685948602647949405869330028998683893235348598163135541138022263494619352700621926997535089014' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04a59f376ea9314e658b086d8c3f2c78450d6b6ddbc23ecd0abdaa710a8c7ddc73b5f7f4aaf7354953a628e002d25bfcb1a5f1bbc5e979ff4165ddff4415511031b5962c4f98e91839674af381b0c3e7e3c6902cd666f92330adf683bfca07b4bf","04d89dc414e9f14f8ae00cd135381914ab61edd6d0e0afd98c6ac50fbb686f36a484d9d3c251fa7e8f9881541774d7f66d03fce38b1116a8b6757ae0d15dc3fac05d88ec0b4f61193e2e6e9c7de5661c02762b9bd3ee60ddff4f8fbb160d0e686a"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '24943945063431737963271902495720930827543087102281281992029824070028461316038173339499570262082248453128094528748003' 's' '5503465509634739097318685948602647949405869330028998683893235348598163135541138022263494619352700621926997535089014' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["046c1a06027ce1e8655a08c259621172818d5131fc546397e1d667a59617f1c67b517a0cd10c3ac0b4228e30363988ef56c6c99cd24169e7b064b047ea393c02075c5753e0c9eb7cc75d5bebcf90254d1cbd604f2fe69215bbf20ff7a5a92f6509","047e7706618e115b76500b5afa36de7f185e7744c623503eb07e678e105b302901f50c6027e8df9e1914bc3b894a6a8026d1777914076c3772d32075a8cf3a8c136a346c340fd3cd2262d47e7aa5e5e76b7a6b9d4d01d406fa3637cdce19d70735"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '24943945063431737963271902495720930825690413674484222865252688309889454790385853584849321237450927109001484454509027' 's' '6976471356623835436981175826241876555409479145890864486744428578859625366550645281341824431607786600116828480532651164863490532450978287283339264' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04f32c985294d3dc142c905972ba41007a88789f3b676ba4b7daa05939c0f77c93556855015506c5341fff33b031dc84a71e12a6b03eef59c547843df4020f4b356e169da977c1ed97e0e12db37b2f33af31a2d32d421fdf48b3479f759c246249","042350d743f7d3f2e6304e218a1a2102144e0fcc6779c608129e52425526ebf80aaf11042e14daf111af9b4856b54558bb208e998b6b414151b5c50fc4270047be4d657d3d3877a3be54d1a52476ef0bf038700772a0994286a734c0149f4a8ca1"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' '' 'UTF-8' ->BYTES 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["041828e3cb081dd25451491249b15e8fb0e5eafeda62f3b014c4e9b90e52caa9f9cfb58f047764a5ba7021b7b6f5aab3589888750cc6f11bb6c4e183623bad8a9a5853a6851193077a356e9b39ed363e87c3acc705ac9270f67dcab63c2d25cce1","041828e3cb081dd25451491249b15e8fb0e5eafeda62f3b014c4e9b90e52caa9f9cfb58f047764a5ba7021b7b6f5aab35867778af3390ee4493b1e7c9dc4527565a7ac597aee6cf885ca9164c612c9c1773c5338f9536d8f09823549c4d2da331e"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'ab' HEX-> 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04b06cd854021d808526c31dea4c2de3f0c533a6323dae1c51e7810150c4fb3277f1adedff1685201937dcae07dea875bd884639d6adf5aa5a6a226953e3db4ec56e76c65e7707bc7286babdfb7d2bea2941b59b37131d3099a7893e31e5217802","0435a1d51eb1854178455595d7758053b1eb67058af9f10f8b9f5a41c503661bfb9db331e1a7cb6fcb11701bc684361b1c8e138ab8847d6fcb8dac12c7172a11e49f2d3c18c30f32333dca7655e49fe2b252921ca4efdac6431dd3541c157f9cff"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'abababababababababababababababababababab' HEX-> 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04b736b3ceb01fbc589ecc953c422d2a1f2c5bd767c19f175bce1a71d8ab4bc5247ff65e6a548043514043d825c6b448302ac15bf027ee520cf239b385df138ebd90d77529659429e45fd82c85ff7c01925537134b7853843e11aec0dc84b1c9d0","04544630ec5287d13ea2669adf19f830790acf0883233b2439ac1670a0b27508fed8821a32017746b3ed89409fc4b61d637f08a5b9b8555cd6d2bc1f4d013b5f827579ddaddb06988706a1bcd959400c1e36de033ec59a5c21d86ce1f429f56e67"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'abababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababab' HEX-> 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04080ab4f7fbd886338c4e9fbd9b7a6eb4a45a279fb8d6af9b40d38b84407026528d2f8a89c4480aa3f1b65f1e9c857cad083cc2d040c65b22b4c0308c31acc774ba7d14e6fabd0152b46afadd8a9514a1d98448bd1955670c08bddbc5e62415b6","04f36fcdd56c36d2966a993f063a33cfda02d110aac30b36daa017bb6a133a2b2630afa3879150c8f77334c11a77b81039b3306645fdf6c3e7b059abe57884bdf106bcfce9b16d89aa50b8b6a37d91c4ff9ff7d5803f5b06b257c0f31b84fbde48"]]"#,
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff' HEX-> 'sig' '3065023100a21067a215de65542289176bd4d2d97b1ee5773d5ba4d14f0b1a08b41b4fde8ec5446497c9945aaba66ff27d7642b9e3023023c1b983184baa4ad20e1b3d1fc7e9a1ffd6639e88bf141d5f59b158717fabddd988ce50694214b6af57e85eb0bde176' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["042a685121282c50efdf6d147153b34ce2e63d9fea90cc7c61bf3d4d8aeda1533b7acfeff45dd9e6ee27ccc993e362354c0d1c6ada67acaa93e464122d5c1598f415dea5ea8cb0541867b7b3d1cf5b7eb0e91d0a57db461ea11033313e76d59a45","047fa0d255226f57af491c9be5777ed3bc1dfb43a10952510117f54555ceffc2ed333a911f54656e20c707566c9ebf0ddc134a6bd49b613a1ed5a850ae38dc0b9376b4c3880adeb60eba84a061fa7ec00f5759a9bee74166a0cd639de9997be503"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX->  } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '4721687394094666562851153492887562346148801736642415325708728937897166900281154211088842198666800856627342055142888214745681402420555250447847036485028732453' 's' '4471579213419948715366615490592599709779377436327641348373205501917537459706767026797759244345687694438006853427988198873256947113073393755924726938500800353'  } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX-> 'even' T } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '4721687394094666562851153492887562346148801736642415325708728937897166900281154211088842198666800856627342055142888214745681402420555250447847036485028732453' 's' '4471579213419948715366615490592599709779377436327641348373205501917537459706767026797759244345687694438006853427988198873256947113073393755924726938500800353' 'even' T } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX-> 'even' F } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d","0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '4721687394094666562851153492887562346148801736642415325708728937897166900281154211088842198666800856627342055142888214745681402420555250447847036485028732453' 's' '4471579213419948715366615490592599709779377436327641348373205501917537459706767026797759244345687694438006853427988198873256947113073393755924726938500800353' 'even' F } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d","0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX-> 'even' 'x' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '4721687394094666562851153492887562346148801736642415325708728937897166900281154211088842198666800856627342055142888214745681402420555250447847036485028732453' 's' '4471579213419948715366615490592599709779377436327641348373205501917537459706767026797759244345687694438006853427988198873256947113073393755924726938500800353' 'even' 'x' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX-> 'i' 0 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '4721687394094666562851153492887562346148801736642415325708728937897166900281154211088842198666800856627342055142888214745681402420555250447847036485028732453' 's' '4471579213419948715366615490592599709779377436327641348373205501917537459706767026797759244345687694438006853427988198873256947113073393755924726938500800353' 'i' 0 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX-> 'i' 1 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '4721687394094666562851153492887562346148801736642415325708728937897166900281154211088842198666800856627342055142888214745681402420555250447847036485028732453' 's' '4471579213419948715366615490592599709779377436327641348373205501917537459706767026797759244345687694438006853427988198873256947113073393755924726938500800353' 'i' 1 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX-> 'i' -1 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '4721687394094666562851153492887562346148801736642415325708728937897166900281154211088842198666800856627342055142888214745681402420555250447847036485028732453' 's' '4471579213419948715366615490592599709779377436327641348373205501917537459706767026797759244345687694438006853427988198873256947113073393755924726938500800353' 'i' -1 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX-> 'i' 5 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '4721687394094666562851153492887562346148801736642415325708728937897166900281154211088842198666800856627342055142888214745681402420555250447847036485028732453' 's' '4471579213419948715366615490592599709779377436327641348373205501917537459706767026797759244345687694438006853427988198873256947113073393755924726938500800353' 'i' 5 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX-> 'i' 1.5 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '4721687394094666562851153492887562346148801736642415325708728937897166900281154211088842198666800856627342055142888214745681402420555250447847036485028732453' 's' '4471579213419948715366615490592599709779377436327641348373205501917537459706767026797759244345687694438006853427988198873256947113073393755924726938500800353' 'i' 1.5 } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX-> 'coords' T } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0x16028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade25","0x188abbb8a49543f5223a5810457b4c22dcbc6a7407361ce5d65c07611b720aa087fd0fb3a591dddfddf146e41f93e0625c13cedf333759f31ac99550e0dcea03b0c","0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f"],["0x16028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade25","0x77544475b6abc0addc5a7efba84b3dd2343958bf8c9e31a29a3f89ee48df55f7802f04c5a6e2220220eb91be06c1f9da3ec3120ccc8a60ce5366aaf1f2315fc4f3","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '4721687394094666562851153492887562346148801736642415325708728937897166900281154211088842198666800856627342055142888214745681402420555250447847036485028732453' 's' '4471579213419948715366615490592599709779377436327641348373205501917537459706767026797759244345687694438006853427988198873256947113073393755924726938500800353' 'coords' T } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0x16028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade25","0x188abbb8a49543f5223a5810457b4c22dcbc6a7407361ce5d65c07611b720aa087fd0fb3a591dddfddf146e41f93e0625c13cedf333759f31ac99550e0dcea03b0c","0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f"],["0x16028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade25","0x77544475b6abc0addc5a7efba84b3dd2343958bf8c9e31a29a3f89ee48df55f7802f04c5a6e2220220eb91be06c1f9da3ec3120ccc8a60ce5366aaf1f2315fc4f3","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX-> 'coords' F } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '4721687394094666562851153492887562346148801736642415325708728937897166900281154211088842198666800856627342055142888214745681402420555250447847036485028732453' 's' '4471579213419948715366615490592599709779377436327641348373205501917537459706767026797759244345687694438006853427988198873256947113073393755924726938500800353' 'coords' F } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX-> 'coords' T 'even' T } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0x16028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade25","0x188abbb8a49543f5223a5810457b4c22dcbc6a7407361ce5d65c07611b720aa087fd0fb3a591dddfddf146e41f93e0625c13cedf333759f31ac99550e0dcea03b0c","0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f"],["0x16028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade25","0x77544475b6abc0addc5a7efba84b3dd2343958bf8c9e31a29a3f89ee48df55f7802f04c5a6e2220220eb91be06c1f9da3ec3120ccc8a60ce5366aaf1f2315fc4f3","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '4721687394094666562851153492887562346148801736642415325708728937897166900281154211088842198666800856627342055142888214745681402420555250447847036485028732453' 's' '4471579213419948715366615490592599709779377436327641348373205501917537459706767026797759244345687694438006853427988198873256947113073393755924726938500800353' 'coords' T 'even' T } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0x16028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade25","0x188abbb8a49543f5223a5810457b4c22dcbc6a7407361ce5d65c07611b720aa087fd0fb3a591dddfddf146e41f93e0625c13cedf333759f31ac99550e0dcea03b0c","0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f"],["0x16028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade25","0x77544475b6abc0addc5a7efba84b3dd2343958bf8c9e31a29a3f89ee48df55f7802f04c5a6e2220220eb91be06c1f9da3ec3120ccc8a60ce5366aaf1f2315fc4f3","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX-> 'coords' T 'i' 0 } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0x16028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade25","0x188abbb8a49543f5223a5810457b4c22dcbc6a7407361ce5d65c07611b720aa087fd0fb3a591dddfddf146e41f93e0625c13cedf333759f31ac99550e0dcea03b0c","0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f"],["0x16028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade25","0x77544475b6abc0addc5a7efba84b3dd2343958bf8c9e31a29a3f89ee48df55f7802f04c5a6e2220220eb91be06c1f9da3ec3120ccc8a60ce5366aaf1f2315fc4f3","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '4721687394094666562851153492887562346148801736642415325708728937897166900281154211088842198666800856627342055142888214745681402420555250447847036485028732453' 's' '4471579213419948715366615490592599709779377436327641348373205501917537459706767026797759244345687694438006853427988198873256947113073393755924726938500800353' 'coords' T 'i' 0 } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r#"[[["0x16028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade25","0x188abbb8a49543f5223a5810457b4c22dcbc6a7407361ce5d65c07611b720aa087fd0fb3a591dddfddf146e41f93e0625c13cedf333759f31ac99550e0dcea03b0c","0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f"],["0x16028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade25","0x77544475b6abc0addc5a7efba84b3dd2343958bf8c9e31a29a3f89ee48df55f7802f04c5a6e2220220eb91be06c1f9da3ec3120ccc8a60ce5366aaf1f2315fc4f3","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX-> 'coords' T 'i' 1 } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '4721687394094666562851153492887562346148801736642415325708728937897166900281154211088842198666800856627342055142888214745681402420555250447847036485028732453' 's' '4471579213419948715366615490592599709779377436327641348373205501917537459706767026797759244345687694438006853427988198873256947113073393755924726938500800353' 'coords' T 'i' 1 } ECRECOVER <% DROP DUP 'R.x' GET SWAP DUP 'R.y' GET SWAP 'pubkey' GET ECPUBLIC 'Q' GET 3 ->LIST %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '0x16028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade25' 's' '0X14D816FE7F9A2E33570886348578FE51B51E2242156C177969848F3D96F4CC732F19EFC5CCB5A9E20AEC9D96D9D8DD184528AE87D04586D68A55B883599CC1F1F61' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f","0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '1' 's' '1' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0401e58ae4578871525083204c4ff8da71c0d74c359f01f442d548e91a3d352dd37ad0cf1f58da2bc4403d48e94805d5bc9903488357d9207d0549ed28f8c1d39034a10049095e4c2fe1958bc7d5581b7d193a0235cd6fdc1b09c24dc0f6dcf650932007a639e5266fbaeb6b4875e2447e053185a470d3719ff82ad505d046e6ed5337455b","04015498f6911c22c35ba6865a86fd88400174760cf7a1c5312ae5d6a82949fe38f14b2ee5bbc20fbe1494856c88128698d553d5045813647716709aa342410e03b9b400979bf11419fe127a81006c69e02a14810e1a12915e49a4807c6bbcdf0af403602338ddceae51b759c20b574547772bd5c28a87ce91e2fbc17995bf3ce6e41af4dd","04012e810d230b24a04459fe8a27f928f8b12ef81cbe90709d92084000ac6674823f4f7b81360c240c4a85c4d7b89f62c010d085b4fea2dd5a7190f703033450fafdd100f889c3d53de3e274f29cb7ce34ab938b81bb103f262407ce9e4ae88307d858cf657021dd63247d9f5bc557f6df21558831228ad40aa715ece781085cc3bf880a7e","04011e4a8fc6bd3ee7329e8a1521ca81d64e2c808fbf8da724d06e7ed1e99797e93a199cfba1f8a0b5d91d8acc1ef5cf2054c5f628c578255c5b70728b4378a8a3a690019ab372ed9baef62b484504cc9944bc8eb0aae8ca6eed5f852a957e56cab03aa65d3f773a5bd58832097c13bb6ffc5c396e9ca8ec982212de26f291c49f2c41aaea"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '5' 's' '7' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04018d637293e4f464f5ff56a3075d4279503e1ac074c8591655a1f72a7a8bf49039432050c4b0546fd15d62500616f459fd0f40f8e882c3042b58b8517e4c03ce295701871e868919f721ea67d92173d3d0beb744ce6c311285910120ecf24bbb53e9ca06d9376ccfd469867f054dff5891fb3dd8222d6dfb07c9f3bce0f8d3ce01cdb256","04012f8b106f5bb4ac8a9bd7c34ed286805efe327171a24b149d9c5668bd81e669cb24b9eef0ec7b0a10fc54fd50a50444b23724081d419b3b7739b2aa5b9475797fa901ee027fa5d15fbd3299c92172473c44529ecf949d6c7db32ae55e43d50d0592b74536a5e16e414c77ada0513a675a56283519fcaa9396d150e52aa5804c090fc5ef"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '-5' 's' '1' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["040095d9cb130f9543f1d19fb81d928941f1bcb4e05b268fe84b310969bb8cf42a29a529b579921b292adb7120b9196f5452a0615f07904b0f30d4cd1cc602e7e99a7400b3591e2df68f5a1c636ad3f7d955167ac30a2ab4712b9f0e58f5810923a9546f0eacf80d35e437dd95a710d9ae10bd11559da219755e2efa609f2e5f7a4cabdcb9","04016afbf2aa9cdda8f70f762b542126ddd79d9a6812c1107acf1062a7455fa2aa232be7af04a6bbb5ca26ffbb061e503d828f349f3db0f7c563e04af2f30e64385d6e003911f2c79d17a18ce7b362c77c1691bb6f53faa8faddfc15e07e8ab5c2c593dde79b9f93b72417f469ea4f23d28cfc13d232c483a883e84deedfe825473ab05bfc"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '2037035976334486086268445688409378161051468393665936250636140449354381299763336706183397376' 's' '1' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r"[[]]",
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '4721687394094666562851153492887562346148801736642415325708728937897166900281154211088842198666800856627342055142888214745681402420555250447847036485028732453' 's' '0' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0401e66ece951a62fad123128e7b367d39039505099db5b2b6ae65aeb34909ca3f0ab86fec039ca8d6a8585acd97adec3e2a063e4fd69b8ad7960dfac40b2547fc857e010660690b87272c8153ec9b5ee228f28268d031c1abbc23e0780a84262a17ced1c1bce2c0ea6a38eec71f277a549c8385ee1f878e9523662080d4c4781ad757804c"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '4721687394094666562851153492887562346148801736642415325708728937897166900281154211088842198666800856627342055142888214745681402420555250447847036485028732453' 's' '-4471579213419948715366615490592599709779377436327641348373205501917537459706767026797759244345687694438006853427988198873256947113073393755924726938500800353' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0400da5e2a8221fbb3057c778e9281bb819b6af9c699aa430f4f9da156e01417f2778b771fe08feb247c96e2af0719d0884aa6349a139e306cd716b221681c0b0e0c94018ea4dee25c4b725abedff1e1181899b1767cb758b705c4fc5bffe8210933a4f27c0b34db94361de6ca3bbd5ce9b52ec3c73462bed7a9fe492a4edb6bce1da70b5d","0401580e67a48df5fdf5ebd71925af1907f5e35845c2f9705b19cd5c950ac0ada355f39cbf87924a56902adb49ad8a2262dfb4d85fd18dd269a4de3c106a404e04bde201eb69fdd5fc3e270c7ecb0fd8031529cd67b3ad00066f06b188332b672aa7a85250240a640cfd65207d3e6550c28d1570b359e7d0caf2784eb989b154161166739f"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '4721687394094666562851153492887562346148801736642415325708728937897166900281156063762269995725927633763102194149413867065436052669579881769191163095102971429' 's' '4471579213419948715366615490592599709779377436327641348373205501917537459706767026797759244345687694438006853427988198873256947113073393755924726938500800353' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04003d5f51fa860a21db6a755b71d6785b3e00dbfa2b6c6e1ce7f2d4545c96a9024d5177422c87805ed0d81f044bb20012d2402b234ddb27d638e44c3aeb872b0134fc00ee4c42726de6e06856ec80ef8946f0892d31bdd57815c756eac949b5d5dfde8b617e9f3ab3db2977c71cec1201909be1079892a350364813394c7029f9cecdb956","0400716c799b33670e86bee3b7d8363a6a7eda88257d8ba26af3194c9ab02dc37d5496cfe40f21aebf4742705f8b8eb28a8675884fba35435fb41625d114cdaea3a1a701808fe9fb11c0e30815b236a827790288405b7b52ba098c07b6f44d3235c226c71cea23dcca5aacbb3a1fddd3d73d9f03a99a970de7d48179f1f9d5c0002313487b"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '4721687394094666562851153492887562346148801736642415325708728937897166900281154211088842198666800856627342055142888214745681402420555250447847036485028732453' 's' '5668400073859871888627653131776226848581055222740756555888380309516785331358988827318732429587864780994697295547597314516765287244305615368887330780302207491533658963916661312276532297728' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04004f5118540a96c312e01f746ea5ee992f81ad6b8bb764d18853cd3cbd3a2685c8de0fdb38eacbab7bf663418a41acda2e3cc0fb6f369d998619ae756be5f767f98800c57cfe39271bab8a69ee9e15caec0d59b7ff083eb0f82b6d45709450d48fb1f86edb2879f45c8adb249dc4c56737fb202d7a964acfdbe625e3c0a156b029c93b74","0401fc24fd7a7e3f4572d514d9abde086eaae8c34f45cf60c72c2b0efc8e2f2c340010a3f4087c0e840088d60687bf59e0270d6ec868c8fae0ebe04157b209b15a8ab100f78a0f4f20d6648e381a64c60e3c05e0e00ed93747a1f9516061ed5de43c0948acc434c04c08118eea7448a4ae5d7fb5642b5e0cc280fc0fe1a33bb7254e6298bf"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' '' 'UTF-8' ->BYTES 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["040139c8e0b7165277ceaea59e2c5c0de0b0e89ba970b5aeec1b619365533aeeb49feea5e96e01c196beeaa7f231015215919c71da8ee3dadcd56c4df42f0d99a2ec87005d59c2d57358a5a821546d9cfc04f1c705ce61de647752c6632e5feffc185de994a9ea3f2c7fdd1ada172e6fa8a02c673ae97b281f50ea9cd7accc70671e636283","040139c8e0b7165277ceaea59e2c5c0de0b0e89ba970b5aeec1b619365533aeeb49feea5e96e01c196beeaa7f231015215919c71da8ee3dadcd56c4df42f0d99a2ec8701a2a63d2a8ca75a57deab926303fb0e38fa319e219b88ad399cd1a01003e7a2166b5615c0d38022e525e8d190575fd398c51684d7e0af15632853338f98e19c9d7c"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'ab' HEX-> 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04005c3c1484e1b21f46f337ac8caa588cb51b5ee05ec560727e4fbef9dcceb91280d013f0702a0bd2fef61bd7e414d41a8e4fdffc63493caca2f84c5d63f464c0d9720044b6638703e0dc7f7dec1489623019ecbf45b92fc75ecb8b150172671311b72a258585651883b5dd3e7abeb1da5643ce2d5fdd84f22e65f85c61321d0978a5f84c","040117b57066f1b59b7d666d4d0a9e3b99ebb551f81763c7224fe544b4b4a72602fa6b3db644635bfd8688816b4290b71edd86d692a50a0befa58e677fae31f41c0e37016c53fd902d625aaf77f35f38fa10eceb693de8573cc0c00b79c4eab3ec8e3a9a6a1ba71da956c4b044e9f42455f32b3886952dec563cacc9f00cb0de374ddd45e5"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'abababababababababababababababababababab' HEX-> 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["0400dcb00e0a08cc0640cd51551b13b590dfacdeed03ce64d691a02b5d5a14ca3975fad2e44548572ae7b7a58525756ff24fb695b28f43fe44ee532612cf407eb5bd2800c00983f46dd9e410c31c583b284e3c8861bc6729b4c314477c99679f609d6f557a3567c74af8217dc2a85c8f95362c60920fdf7f79bc8599ddd2f8a2924e6e0d87","0400b29493affb2e259675a1386b0780eb5ce0d0b9c451c43e0c8b97bc63a07b4f9a653dbf24daaadb5aa275f8d7e95ae7514ed217024a9b63b62c87ec0f1b13025b45010661a98e60c308d669e78a5592b04e7f3344e8dc024d89312deadf77bb02b76fc88e3fe90ca4f262a6a80d31d3e31d9f10997bbfa9817842031ebb19cb22942e84"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'abababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababababab' HEX-> 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["04004db9d2a7046115efb2957ee1b81effb13b7d390b3f46e5d79d5c555518816d40fa5c96d0c270eaad6abb5f08f7482ece4f77ccec3a80d371cb7d80f81d80dcf4380083565b32b08ff723504297896c4876f3ad6163b0adc9a5ac64924d06b3294e3e9319c237af8a27534068ca472583aa6e1baed79e449612b5e67b0e49af5ea73814","0400860521bb4e94f5f00ca888fdf82c1274f15f4da2bb98d4d2bd9cd851e3e5f2ed4ca23986a427c396bc76888024d481a3d06bef36fe565f03a03f5558937e60ccbe00c0d0767778e2de2da0314761b86b06c70b4bfce1a3da422d064e51d262cccd81291f56fe13ed10d6cb1d05f84bd00ed32e96322674d49eb74aa51139bd42ef4ddf"]]"#,
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff' HEX-> 'sig' '3081880242016028d73a48f90a477fc4323fb93eeefe4e5cd9d187b8b50c585b8ec4034a3547cdefb9ea6900a47c646000f379e0eaf48dd9bff8ce6ce2528a33e84539dccade250242014d816fe7f9a2e33570886348578fe51b51e2242156c177969848f3d96f4cc732f19efc5ccb5a9e20aec9d96d9d8dd184528ae87d04586d68a55b883599cc1f1f61' HEX-> } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP STACKTOLIST ->JSON",
        r#"[["040045c8ead0d4abc194d608790d6d6a9f3b00b0b657aa9e874dc8fa7b183204fa5fcc9c5d01a2f3f5da4603b7361e7b703eecff4f6e7a17518d047f4ff168b16cc3050110573820ec86f7b791be2b367d8a9c5716d058b15990bb1ae86fc30adc07c8edcca94f8abd53f47aea623cb8d17d1ff8f013c83f284e2f8a6c1aaaa4fc2ff40490","0400908715bd967570b07639f63d4e1b16179c2897cf99800e76bbcd0f790d85f5723f745c1727dc20c2dba0b2b35203fb77baee3675b9e63f79969a0ba919bcd1fdd200d00bad4e4a8682be4d0d09d922baef6e4a72db0774010da64b009886282e85c138c72f70164ca7b7a2277978b1207c027687a37842fd7e955f4eef5b91e310ed17"]]"#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3006020101020101' HEX-> } ECRECOVER SIZE STACKTOLIST ->JSON",
        r"[2]",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '300602010102010a' HEX-> } ECRECOVER SIZE STACKTOLIST ->JSON",
        r"[2]",
    ),
];

const RECOVER_ERRORS: &[(&str, &str)] = &[
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '0' 's' '1' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP",
        r"BigInteger not invertible.",
    ),
    (
        r"{ 'curve' 'secp256r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '0' 's' '1' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP",
        r"BigInteger not invertible.",
    ),
    (
        r"{ 'curve' 'secp384r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '0' 's' '1' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP",
        r"BigInteger not invertible.",
    ),
    (
        r"{ 'curve' 'secp521r1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'r' '0' 's' '1' } ECRECOVER <% DROP ECPUBLIC 'Q' GET %> LMAP",
        r"BigInteger not invertible.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3006020100020101' HEX-> } ECRECOVER",
        r"ECRECOVER invalid value for r, should be in [1, 0x00fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141] but was 0x0.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3006020101020100' HEX-> } ECRECOVER",
        r"ECRECOVER invalid value for s, should be in [1, 0x00fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141] but was 0x0.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3006020101' HEX-> } ECRECOVER",
        r"7 > 5",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '30' HEX-> } ECRECOVER",
        r"1",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '' HEX-> } ECRECOVER",
        r"1",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '01' HEX-> } ECRECOVER",
        r"1",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3081' HEX-> } ECRECOVER",
        r"2",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '308102' HEX-> } ECRECOVER",
        r"4",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '30820102' HEX-> } ECRECOVER",
        r"5",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '3044022004f4221b738eedd9ddea1d1b8eda9d7bfb12de26cf9fef59a6d39244cd90618402205a68c12b8e5945ccf3209e71aff44720e41bd31d0668ce095800296327e55eb3ffff' HEX-> } ECRECOVER",
        r"ECRECOVER invalid value for s, should be in [1, 0x00fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141] but was 0x5a68c12b8e5945ccf3209e71aff44720e41bd31d0668ce095800296327e55eb3ffff.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '300402010102' HEX-> } ECRECOVER",
        r"7 > 6",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'hello' 'UTF-8' ->BYTES SHA256 'sig' '30060280010102' HEX-> } ECRECOVER",
        r"4 > -124",
    ),
    (r"ECRECOVER", r"Empty stack."),
    (r"1 ECRECOVER", r"ECRECOVER expects a MAP."),
    (
        r"{ } ECRECOVER",
        r"ECRECOVER missing ECC curve name under 'curve'.",
    ),
    (
        r"{ 'curve' 1 } ECRECOVER",
        r"ECRECOVER missing ECC curve name under 'curve'.",
    ),
    (
        r"{ 'curve' 'secp256k1' } ECRECOVER",
        r"ECRECOVER invalid 'hash', expected BYTES.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'a' } ECRECOVER",
        r"ECRECOVER invalid 'hash', expected BYTES.",
    ),
    (
        r"{ 'curve' 'nope' 'hash' 'a' 'UTF-8' ->BYTES } ECRECOVER",
        r"NullPointerException",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'a' 'UTF-8' ->BYTES } ECRECOVER",
        r"ECRECOVER expects 'sig' or 'r' and 's' to be provided.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'a' 'UTF-8' ->BYTES 'sig' 'a' } ECRECOVER",
        r"ECRECOVER invalid 'sig', expected BYTES.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'a' 'UTF-8' ->BYTES 'r' 1 's' 1 } ECRECOVER",
        r"ECRECOVER invalid 'r', expected STRING.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'a' 'UTF-8' ->BYTES 'r' '1' 's' 1 } ECRECOVER",
        r"ECRECOVER invalid 's', expected STRING.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'a' 'UTF-8' ->BYTES 'r' '1' 's' 'x' } ECRECOVER",
        r#"For input string: "x""#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'a' 'UTF-8' ->BYTES 'r' 'x' 's' '1' } ECRECOVER",
        r#"For input string: "x""#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'a' 'UTF-8' ->BYTES 'r' '' 's' '1' } ECRECOVER",
        r"Zero length BigInteger",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'a' 'UTF-8' ->BYTES 'r' '0xzz' 's' '1' } ECRECOVER",
        r#"For input string: "zz""#,
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'a' 'UTF-8' ->BYTES 'r' '1' } ECRECOVER",
        r"ECRECOVER expects 'sig' or 'r' and 's' to be provided.",
    ),
    (
        r"{ 'curve' 'secp256k1' 'hash' 'a' 'UTF-8' ->BYTES 'r' '1-2' 's' '1' } ECRECOVER",
        r"Illegal embedded sign character",
    ),
];

const MACROS_VALUES: &[(&str, &str)] = &[
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MSIGCOUNT SNAPSHOT",
        r"1 ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MCHECKSIG SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> true ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MVERIFY SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MDETACH SNAPSHOT",
        r"<% 1 2 + %> <% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MSIG SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> <% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> EVAL SNAPSHOT",
        r"3 ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MSIGINFO DUP TYPEOF <% 'MAP' == %> <% DUP 'key' GET ECPUBLIC 'Q' GET SWAP 'sig' GET ->HEX 2 ->LIST %> <% %> IFTE SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> [] '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' +! '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' +! ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '30440220574644975131a422872342e35476b6f48d8d26d14e9a5840fa91c88f46c8f3ff0220739bb59479bd3e6a72b86fc12bb6f7e85e6f57b5a68ee2c4135410d8d26e282a' MSIG 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '304402207d4bd94d3e8a9576be16844d07a711fb5275cea6f052293b37be1a40deb2bf5c02207897b44f92741d67519f282225a2a306c05b64e78e1c2ca94187cfd1ad1e94fa' MSIG %> MSIGCOUNT SNAPSHOT",
        r"2 ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '30440220574644975131a422872342e35476b6f48d8d26d14e9a5840fa91c88f46c8f3ff0220739bb59479bd3e6a72b86fc12bb6f7e85e6f57b5a68ee2c4135410d8d26e282a' MSIG 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '304402207d4bd94d3e8a9576be16844d07a711fb5275cea6f052293b37be1a40deb2bf5c02207897b44f92741d67519f282225a2a306c05b64e78e1c2ca94187cfd1ad1e94fa' MSIG %> MCHECKSIG SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '30440220574644975131a422872342e35476b6f48d8d26d14e9a5840fa91c88f46c8f3ff0220739bb59479bd3e6a72b86fc12bb6f7e85e6f57b5a68ee2c4135410d8d26e282a' MSIG 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '304402207d4bd94d3e8a9576be16844d07a711fb5275cea6f052293b37be1a40deb2bf5c02207897b44f92741d67519f282225a2a306c05b64e78e1c2ca94187cfd1ad1e94fa' MSIG %> true ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '30440220574644975131a422872342e35476b6f48d8d26d14e9a5840fa91c88f46c8f3ff0220739bb59479bd3e6a72b86fc12bb6f7e85e6f57b5a68ee2c4135410d8d26e282a' MSIG 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '304402207d4bd94d3e8a9576be16844d07a711fb5275cea6f052293b37be1a40deb2bf5c02207897b44f92741d67519f282225a2a306c05b64e78e1c2ca94187cfd1ad1e94fa' MSIG %> MVERIFY SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '30440220574644975131a422872342e35476b6f48d8d26d14e9a5840fa91c88f46c8f3ff0220739bb59479bd3e6a72b86fc12bb6f7e85e6f57b5a68ee2c4135410d8d26e282a' MSIG 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '304402207d4bd94d3e8a9576be16844d07a711fb5275cea6f052293b37be1a40deb2bf5c02207897b44f92741d67519f282225a2a306c05b64e78e1c2ca94187cfd1ad1e94fa' MSIG %> ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '30440220574644975131a422872342e35476b6f48d8d26d14e9a5840fa91c88f46c8f3ff0220739bb59479bd3e6a72b86fc12bb6f7e85e6f57b5a68ee2c4135410d8d26e282a' MSIG 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '304402207d4bd94d3e8a9576be16844d07a711fb5275cea6f052293b37be1a40deb2bf5c02207897b44f92741d67519f282225a2a306c05b64e78e1c2ca94187cfd1ad1e94fa' MSIG %> MDETACH SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '30440220574644975131a422872342e35476b6f48d8d26d14e9a5840fa91c88f46c8f3ff0220739bb59479bd3e6a72b86fc12bb6f7e85e6f57b5a68ee2c4135410d8d26e282a' MSIG %> <% 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '304402207d4bd94d3e8a9576be16844d07a711fb5275cea6f052293b37be1a40deb2bf5c02207897b44f92741d67519f282225a2a306c05b64e78e1c2ca94187cfd1ad1e94fa' MSIG %> ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '30440220574644975131a422872342e35476b6f48d8d26d14e9a5840fa91c88f46c8f3ff0220739bb59479bd3e6a72b86fc12bb6f7e85e6f57b5a68ee2c4135410d8d26e282a' MSIG 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '304402207d4bd94d3e8a9576be16844d07a711fb5275cea6f052293b37be1a40deb2bf5c02207897b44f92741d67519f282225a2a306c05b64e78e1c2ca94187cfd1ad1e94fa' MSIG %> MSIG SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '30440220574644975131a422872342e35476b6f48d8d26d14e9a5840fa91c88f46c8f3ff0220739bb59479bd3e6a72b86fc12bb6f7e85e6f57b5a68ee2c4135410d8d26e282a' MSIG 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '304402207d4bd94d3e8a9576be16844d07a711fb5275cea6f052293b37be1a40deb2bf5c02207897b44f92741d67519f282225a2a306c05b64e78e1c2ca94187cfd1ad1e94fa' MSIG %> <% 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '304402207d4bd94d3e8a9576be16844d07a711fb5275cea6f052293b37be1a40deb2bf5c02207897b44f92741d67519f282225a2a306c05b64e78e1c2ca94187cfd1ad1e94fa' MSIG %> ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '30440220574644975131a422872342e35476b6f48d8d26d14e9a5840fa91c88f46c8f3ff0220739bb59479bd3e6a72b86fc12bb6f7e85e6f57b5a68ee2c4135410d8d26e282a' MSIG 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '304402207d4bd94d3e8a9576be16844d07a711fb5275cea6f052293b37be1a40deb2bf5c02207897b44f92741d67519f282225a2a306c05b64e78e1c2ca94187cfd1ad1e94fa' MSIG %> EVAL SNAPSHOT",
        r"3 ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '30440220574644975131a422872342e35476b6f48d8d26d14e9a5840fa91c88f46c8f3ff0220739bb59479bd3e6a72b86fc12bb6f7e85e6f57b5a68ee2c4135410d8d26e282a' MSIG 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '304402207d4bd94d3e8a9576be16844d07a711fb5275cea6f052293b37be1a40deb2bf5c02207897b44f92741d67519f282225a2a306c05b64e78e1c2ca94187cfd1ad1e94fa' MSIG %> MSIGINFO DUP TYPEOF <% 'MAP' == %> <% DUP 'key' GET ECPUBLIC 'Q' GET SWAP 'sig' GET ->HEX 2 ->LIST %> <% %> IFTE SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '30440220574644975131a422872342e35476b6f48d8d26d14e9a5840fa91c88f46c8f3ff0220739bb59479bd3e6a72b86fc12bb6f7e85e6f57b5a68ee2c4135410d8d26e282a' MSIG 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '304402207d4bd94d3e8a9576be16844d07a711fb5275cea6f052293b37be1a40deb2bf5c02207897b44f92741d67519f282225a2a306c05b64e78e1c2ca94187cfd1ad1e94fa' MSIG %> [] '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' +! '304402207d4bd94d3e8a9576be16844d07a711fb5275cea6f052293b37be1a40deb2bf5c02207897b44f92741d67519f282225a2a306c05b64e78e1c2ca94187cfd1ad1e94fa' +! ",
    ),
    (
        r"<% 'a' 'b' + 'secp521r1' '04001699e9fbb70f17f7e3c8175e2bd4c1f5cbd2d30b694eef3bde2f2ed68b919d792257dd54a04b846fe60745e36dd94a39bf25d2e3d7e3b166d73b401ecfb7e9649c0182887b66e3a257d6634109dae5a4806ff94fa398bf56a56bf48be8a4bf86c89e03a83916574830dc839c1349f0649068f7b944e1d3884d16ca2620b2971a254c85' '30818702413f50c2033a93c952719dedb5d834c4ac3f924e9847ba4504291c03639fcfa9739aebc2d6fcfc38c05868587fda5ea521b4983bce1bf5b098988c22a571cb638619024201835e24b1db612c33d073c005fa0a8f9f0c5768554363ba510bd012b31f3a42fca4476452d344502e6471a526481cdfe50e316db65719628af4a7bd861a45c3060a' MSIG 'secp384r1' '0463110061e15a51190cef8c3d91f0d705e8a3015d93ba0d39a29cf53272ef075aa258c6a1784f7f42f377dd785ac4ce3771b53105a5e6f27b81772370315927b9fbd4d7bbc8f09daa492a946184e76ba7c5cd12cf48d70da86ef6d7a1cdcde8fd' '306402302ea988d7dfacfd6476b84f5400d965a979c90b2bc08882c2433a3dd06b52ba1b89e74dd1eea1a8df61353a6f1194af1a023041d1be206ae83dbd1082cb4bf138b0dfd40bfc9d61a6888bc0931b675a7e52fd0da80d818e9aaa488868df69cf9cdc7f' MSIG 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402203a0b9574eba2979de9952e0ab57b06664f275179c8854e73c3ec1cdfa2db0ed20220732b4e917e91af623d09b6d62bac017bfd95f3458f0c5dbc52d32604cc43cbf4' MSIG %> MSIGCOUNT SNAPSHOT",
        r"3 ",
    ),
    (
        r"<% 'a' 'b' + 'secp521r1' '04001699e9fbb70f17f7e3c8175e2bd4c1f5cbd2d30b694eef3bde2f2ed68b919d792257dd54a04b846fe60745e36dd94a39bf25d2e3d7e3b166d73b401ecfb7e9649c0182887b66e3a257d6634109dae5a4806ff94fa398bf56a56bf48be8a4bf86c89e03a83916574830dc839c1349f0649068f7b944e1d3884d16ca2620b2971a254c85' '30818702413f50c2033a93c952719dedb5d834c4ac3f924e9847ba4504291c03639fcfa9739aebc2d6fcfc38c05868587fda5ea521b4983bce1bf5b098988c22a571cb638619024201835e24b1db612c33d073c005fa0a8f9f0c5768554363ba510bd012b31f3a42fca4476452d344502e6471a526481cdfe50e316db65719628af4a7bd861a45c3060a' MSIG 'secp384r1' '0463110061e15a51190cef8c3d91f0d705e8a3015d93ba0d39a29cf53272ef075aa258c6a1784f7f42f377dd785ac4ce3771b53105a5e6f27b81772370315927b9fbd4d7bbc8f09daa492a946184e76ba7c5cd12cf48d70da86ef6d7a1cdcde8fd' '306402302ea988d7dfacfd6476b84f5400d965a979c90b2bc08882c2433a3dd06b52ba1b89e74dd1eea1a8df61353a6f1194af1a023041d1be206ae83dbd1082cb4bf138b0dfd40bfc9d61a6888bc0931b675a7e52fd0da80d818e9aaa488868df69cf9cdc7f' MSIG 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402203a0b9574eba2979de9952e0ab57b06664f275179c8854e73c3ec1cdfa2db0ed20220732b4e917e91af623d09b6d62bac017bfd95f3458f0c5dbc52d32604cc43cbf4' MSIG %> MCHECKSIG SNAPSHOT",
        r"<% 'a' 'b' + 'secp521r1' '04001699e9fbb70f17f7e3c8175e2bd4c1f5cbd2d30b694eef3bde2f2ed68b919d792257dd54a04b846fe60745e36dd94a39bf25d2e3d7e3b166d73b401ecfb7e9649c0182887b66e3a257d6634109dae5a4806ff94fa398bf56a56bf48be8a4bf86c89e03a83916574830dc839c1349f0649068f7b944e1d3884d16ca2620b2971a254c85' '30818702413f50c2033a93c952719dedb5d834c4ac3f924e9847ba4504291c03639fcfa9739aebc2d6fcfc38c05868587fda5ea521b4983bce1bf5b098988c22a571cb638619024201835e24b1db612c33d073c005fa0a8f9f0c5768554363ba510bd012b31f3a42fca4476452d344502e6471a526481cdfe50e316db65719628af4a7bd861a45c3060a' MSIG 'secp384r1' '0463110061e15a51190cef8c3d91f0d705e8a3015d93ba0d39a29cf53272ef075aa258c6a1784f7f42f377dd785ac4ce3771b53105a5e6f27b81772370315927b9fbd4d7bbc8f09daa492a946184e76ba7c5cd12cf48d70da86ef6d7a1cdcde8fd' '306402302ea988d7dfacfd6476b84f5400d965a979c90b2bc08882c2433a3dd06b52ba1b89e74dd1eea1a8df61353a6f1194af1a023041d1be206ae83dbd1082cb4bf138b0dfd40bfc9d61a6888bc0931b675a7e52fd0da80d818e9aaa488868df69cf9cdc7f' MSIG 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402203a0b9574eba2979de9952e0ab57b06664f275179c8854e73c3ec1cdfa2db0ed20220732b4e917e91af623d09b6d62bac017bfd95f3458f0c5dbc52d32604cc43cbf4' MSIG %> true ",
    ),
    (
        r"<% 'a' 'b' + 'secp521r1' '04001699e9fbb70f17f7e3c8175e2bd4c1f5cbd2d30b694eef3bde2f2ed68b919d792257dd54a04b846fe60745e36dd94a39bf25d2e3d7e3b166d73b401ecfb7e9649c0182887b66e3a257d6634109dae5a4806ff94fa398bf56a56bf48be8a4bf86c89e03a83916574830dc839c1349f0649068f7b944e1d3884d16ca2620b2971a254c85' '30818702413f50c2033a93c952719dedb5d834c4ac3f924e9847ba4504291c03639fcfa9739aebc2d6fcfc38c05868587fda5ea521b4983bce1bf5b098988c22a571cb638619024201835e24b1db612c33d073c005fa0a8f9f0c5768554363ba510bd012b31f3a42fca4476452d344502e6471a526481cdfe50e316db65719628af4a7bd861a45c3060a' MSIG 'secp384r1' '0463110061e15a51190cef8c3d91f0d705e8a3015d93ba0d39a29cf53272ef075aa258c6a1784f7f42f377dd785ac4ce3771b53105a5e6f27b81772370315927b9fbd4d7bbc8f09daa492a946184e76ba7c5cd12cf48d70da86ef6d7a1cdcde8fd' '306402302ea988d7dfacfd6476b84f5400d965a979c90b2bc08882c2433a3dd06b52ba1b89e74dd1eea1a8df61353a6f1194af1a023041d1be206ae83dbd1082cb4bf138b0dfd40bfc9d61a6888bc0931b675a7e52fd0da80d818e9aaa488868df69cf9cdc7f' MSIG 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402203a0b9574eba2979de9952e0ab57b06664f275179c8854e73c3ec1cdfa2db0ed20220732b4e917e91af623d09b6d62bac017bfd95f3458f0c5dbc52d32604cc43cbf4' MSIG %> MVERIFY SNAPSHOT",
        r"<% 'a' 'b' + 'secp521r1' '04001699e9fbb70f17f7e3c8175e2bd4c1f5cbd2d30b694eef3bde2f2ed68b919d792257dd54a04b846fe60745e36dd94a39bf25d2e3d7e3b166d73b401ecfb7e9649c0182887b66e3a257d6634109dae5a4806ff94fa398bf56a56bf48be8a4bf86c89e03a83916574830dc839c1349f0649068f7b944e1d3884d16ca2620b2971a254c85' '30818702413f50c2033a93c952719dedb5d834c4ac3f924e9847ba4504291c03639fcfa9739aebc2d6fcfc38c05868587fda5ea521b4983bce1bf5b098988c22a571cb638619024201835e24b1db612c33d073c005fa0a8f9f0c5768554363ba510bd012b31f3a42fca4476452d344502e6471a526481cdfe50e316db65719628af4a7bd861a45c3060a' MSIG 'secp384r1' '0463110061e15a51190cef8c3d91f0d705e8a3015d93ba0d39a29cf53272ef075aa258c6a1784f7f42f377dd785ac4ce3771b53105a5e6f27b81772370315927b9fbd4d7bbc8f09daa492a946184e76ba7c5cd12cf48d70da86ef6d7a1cdcde8fd' '306402302ea988d7dfacfd6476b84f5400d965a979c90b2bc08882c2433a3dd06b52ba1b89e74dd1eea1a8df61353a6f1194af1a023041d1be206ae83dbd1082cb4bf138b0dfd40bfc9d61a6888bc0931b675a7e52fd0da80d818e9aaa488868df69cf9cdc7f' MSIG 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402203a0b9574eba2979de9952e0ab57b06664f275179c8854e73c3ec1cdfa2db0ed20220732b4e917e91af623d09b6d62bac017bfd95f3458f0c5dbc52d32604cc43cbf4' MSIG %> ",
    ),
    (
        r"<% 'a' 'b' + 'secp521r1' '04001699e9fbb70f17f7e3c8175e2bd4c1f5cbd2d30b694eef3bde2f2ed68b919d792257dd54a04b846fe60745e36dd94a39bf25d2e3d7e3b166d73b401ecfb7e9649c0182887b66e3a257d6634109dae5a4806ff94fa398bf56a56bf48be8a4bf86c89e03a83916574830dc839c1349f0649068f7b944e1d3884d16ca2620b2971a254c85' '30818702413f50c2033a93c952719dedb5d834c4ac3f924e9847ba4504291c03639fcfa9739aebc2d6fcfc38c05868587fda5ea521b4983bce1bf5b098988c22a571cb638619024201835e24b1db612c33d073c005fa0a8f9f0c5768554363ba510bd012b31f3a42fca4476452d344502e6471a526481cdfe50e316db65719628af4a7bd861a45c3060a' MSIG 'secp384r1' '0463110061e15a51190cef8c3d91f0d705e8a3015d93ba0d39a29cf53272ef075aa258c6a1784f7f42f377dd785ac4ce3771b53105a5e6f27b81772370315927b9fbd4d7bbc8f09daa492a946184e76ba7c5cd12cf48d70da86ef6d7a1cdcde8fd' '306402302ea988d7dfacfd6476b84f5400d965a979c90b2bc08882c2433a3dd06b52ba1b89e74dd1eea1a8df61353a6f1194af1a023041d1be206ae83dbd1082cb4bf138b0dfd40bfc9d61a6888bc0931b675a7e52fd0da80d818e9aaa488868df69cf9cdc7f' MSIG 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402203a0b9574eba2979de9952e0ab57b06664f275179c8854e73c3ec1cdfa2db0ed20220732b4e917e91af623d09b6d62bac017bfd95f3458f0c5dbc52d32604cc43cbf4' MSIG %> MDETACH SNAPSHOT",
        r"<% 'a' 'b' + 'secp521r1' '04001699e9fbb70f17f7e3c8175e2bd4c1f5cbd2d30b694eef3bde2f2ed68b919d792257dd54a04b846fe60745e36dd94a39bf25d2e3d7e3b166d73b401ecfb7e9649c0182887b66e3a257d6634109dae5a4806ff94fa398bf56a56bf48be8a4bf86c89e03a83916574830dc839c1349f0649068f7b944e1d3884d16ca2620b2971a254c85' '30818702413f50c2033a93c952719dedb5d834c4ac3f924e9847ba4504291c03639fcfa9739aebc2d6fcfc38c05868587fda5ea521b4983bce1bf5b098988c22a571cb638619024201835e24b1db612c33d073c005fa0a8f9f0c5768554363ba510bd012b31f3a42fca4476452d344502e6471a526481cdfe50e316db65719628af4a7bd861a45c3060a' MSIG 'secp384r1' '0463110061e15a51190cef8c3d91f0d705e8a3015d93ba0d39a29cf53272ef075aa258c6a1784f7f42f377dd785ac4ce3771b53105a5e6f27b81772370315927b9fbd4d7bbc8f09daa492a946184e76ba7c5cd12cf48d70da86ef6d7a1cdcde8fd' '306402302ea988d7dfacfd6476b84f5400d965a979c90b2bc08882c2433a3dd06b52ba1b89e74dd1eea1a8df61353a6f1194af1a023041d1be206ae83dbd1082cb4bf138b0dfd40bfc9d61a6888bc0931b675a7e52fd0da80d818e9aaa488868df69cf9cdc7f' MSIG %> <% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402203a0b9574eba2979de9952e0ab57b06664f275179c8854e73c3ec1cdfa2db0ed20220732b4e917e91af623d09b6d62bac017bfd95f3458f0c5dbc52d32604cc43cbf4' MSIG %> ",
    ),
    (
        r"<% 'a' 'b' + 'secp521r1' '04001699e9fbb70f17f7e3c8175e2bd4c1f5cbd2d30b694eef3bde2f2ed68b919d792257dd54a04b846fe60745e36dd94a39bf25d2e3d7e3b166d73b401ecfb7e9649c0182887b66e3a257d6634109dae5a4806ff94fa398bf56a56bf48be8a4bf86c89e03a83916574830dc839c1349f0649068f7b944e1d3884d16ca2620b2971a254c85' '30818702413f50c2033a93c952719dedb5d834c4ac3f924e9847ba4504291c03639fcfa9739aebc2d6fcfc38c05868587fda5ea521b4983bce1bf5b098988c22a571cb638619024201835e24b1db612c33d073c005fa0a8f9f0c5768554363ba510bd012b31f3a42fca4476452d344502e6471a526481cdfe50e316db65719628af4a7bd861a45c3060a' MSIG 'secp384r1' '0463110061e15a51190cef8c3d91f0d705e8a3015d93ba0d39a29cf53272ef075aa258c6a1784f7f42f377dd785ac4ce3771b53105a5e6f27b81772370315927b9fbd4d7bbc8f09daa492a946184e76ba7c5cd12cf48d70da86ef6d7a1cdcde8fd' '306402302ea988d7dfacfd6476b84f5400d965a979c90b2bc08882c2433a3dd06b52ba1b89e74dd1eea1a8df61353a6f1194af1a023041d1be206ae83dbd1082cb4bf138b0dfd40bfc9d61a6888bc0931b675a7e52fd0da80d818e9aaa488868df69cf9cdc7f' MSIG 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402203a0b9574eba2979de9952e0ab57b06664f275179c8854e73c3ec1cdfa2db0ed20220732b4e917e91af623d09b6d62bac017bfd95f3458f0c5dbc52d32604cc43cbf4' MSIG %> MSIG SNAPSHOT",
        r"<% 'a' 'b' + 'secp521r1' '04001699e9fbb70f17f7e3c8175e2bd4c1f5cbd2d30b694eef3bde2f2ed68b919d792257dd54a04b846fe60745e36dd94a39bf25d2e3d7e3b166d73b401ecfb7e9649c0182887b66e3a257d6634109dae5a4806ff94fa398bf56a56bf48be8a4bf86c89e03a83916574830dc839c1349f0649068f7b944e1d3884d16ca2620b2971a254c85' '30818702413f50c2033a93c952719dedb5d834c4ac3f924e9847ba4504291c03639fcfa9739aebc2d6fcfc38c05868587fda5ea521b4983bce1bf5b098988c22a571cb638619024201835e24b1db612c33d073c005fa0a8f9f0c5768554363ba510bd012b31f3a42fca4476452d344502e6471a526481cdfe50e316db65719628af4a7bd861a45c3060a' MSIG 'secp384r1' '0463110061e15a51190cef8c3d91f0d705e8a3015d93ba0d39a29cf53272ef075aa258c6a1784f7f42f377dd785ac4ce3771b53105a5e6f27b81772370315927b9fbd4d7bbc8f09daa492a946184e76ba7c5cd12cf48d70da86ef6d7a1cdcde8fd' '306402302ea988d7dfacfd6476b84f5400d965a979c90b2bc08882c2433a3dd06b52ba1b89e74dd1eea1a8df61353a6f1194af1a023041d1be206ae83dbd1082cb4bf138b0dfd40bfc9d61a6888bc0931b675a7e52fd0da80d818e9aaa488868df69cf9cdc7f' MSIG 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402203a0b9574eba2979de9952e0ab57b06664f275179c8854e73c3ec1cdfa2db0ed20220732b4e917e91af623d09b6d62bac017bfd95f3458f0c5dbc52d32604cc43cbf4' MSIG %> <% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402203a0b9574eba2979de9952e0ab57b06664f275179c8854e73c3ec1cdfa2db0ed20220732b4e917e91af623d09b6d62bac017bfd95f3458f0c5dbc52d32604cc43cbf4' MSIG %> ",
    ),
    (
        r"<% 'a' 'b' + 'secp521r1' '04001699e9fbb70f17f7e3c8175e2bd4c1f5cbd2d30b694eef3bde2f2ed68b919d792257dd54a04b846fe60745e36dd94a39bf25d2e3d7e3b166d73b401ecfb7e9649c0182887b66e3a257d6634109dae5a4806ff94fa398bf56a56bf48be8a4bf86c89e03a83916574830dc839c1349f0649068f7b944e1d3884d16ca2620b2971a254c85' '30818702413f50c2033a93c952719dedb5d834c4ac3f924e9847ba4504291c03639fcfa9739aebc2d6fcfc38c05868587fda5ea521b4983bce1bf5b098988c22a571cb638619024201835e24b1db612c33d073c005fa0a8f9f0c5768554363ba510bd012b31f3a42fca4476452d344502e6471a526481cdfe50e316db65719628af4a7bd861a45c3060a' MSIG 'secp384r1' '0463110061e15a51190cef8c3d91f0d705e8a3015d93ba0d39a29cf53272ef075aa258c6a1784f7f42f377dd785ac4ce3771b53105a5e6f27b81772370315927b9fbd4d7bbc8f09daa492a946184e76ba7c5cd12cf48d70da86ef6d7a1cdcde8fd' '306402302ea988d7dfacfd6476b84f5400d965a979c90b2bc08882c2433a3dd06b52ba1b89e74dd1eea1a8df61353a6f1194af1a023041d1be206ae83dbd1082cb4bf138b0dfd40bfc9d61a6888bc0931b675a7e52fd0da80d818e9aaa488868df69cf9cdc7f' MSIG 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402203a0b9574eba2979de9952e0ab57b06664f275179c8854e73c3ec1cdfa2db0ed20220732b4e917e91af623d09b6d62bac017bfd95f3458f0c5dbc52d32604cc43cbf4' MSIG %> EVAL SNAPSHOT",
        r"'ab' ",
    ),
    (
        r"<% 'a' 'b' + 'secp521r1' '04001699e9fbb70f17f7e3c8175e2bd4c1f5cbd2d30b694eef3bde2f2ed68b919d792257dd54a04b846fe60745e36dd94a39bf25d2e3d7e3b166d73b401ecfb7e9649c0182887b66e3a257d6634109dae5a4806ff94fa398bf56a56bf48be8a4bf86c89e03a83916574830dc839c1349f0649068f7b944e1d3884d16ca2620b2971a254c85' '30818702413f50c2033a93c952719dedb5d834c4ac3f924e9847ba4504291c03639fcfa9739aebc2d6fcfc38c05868587fda5ea521b4983bce1bf5b098988c22a571cb638619024201835e24b1db612c33d073c005fa0a8f9f0c5768554363ba510bd012b31f3a42fca4476452d344502e6471a526481cdfe50e316db65719628af4a7bd861a45c3060a' MSIG 'secp384r1' '0463110061e15a51190cef8c3d91f0d705e8a3015d93ba0d39a29cf53272ef075aa258c6a1784f7f42f377dd785ac4ce3771b53105a5e6f27b81772370315927b9fbd4d7bbc8f09daa492a946184e76ba7c5cd12cf48d70da86ef6d7a1cdcde8fd' '306402302ea988d7dfacfd6476b84f5400d965a979c90b2bc08882c2433a3dd06b52ba1b89e74dd1eea1a8df61353a6f1194af1a023041d1be206ae83dbd1082cb4bf138b0dfd40bfc9d61a6888bc0931b675a7e52fd0da80d818e9aaa488868df69cf9cdc7f' MSIG 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402203a0b9574eba2979de9952e0ab57b06664f275179c8854e73c3ec1cdfa2db0ed20220732b4e917e91af623d09b6d62bac017bfd95f3458f0c5dbc52d32604cc43cbf4' MSIG %> MSIGINFO DUP TYPEOF <% 'MAP' == %> <% DUP 'key' GET ECPUBLIC 'Q' GET SWAP 'sig' GET ->HEX 2 ->LIST %> <% %> IFTE SNAPSHOT",
        r"<% 'a' 'b' + 'secp521r1' '04001699e9fbb70f17f7e3c8175e2bd4c1f5cbd2d30b694eef3bde2f2ed68b919d792257dd54a04b846fe60745e36dd94a39bf25d2e3d7e3b166d73b401ecfb7e9649c0182887b66e3a257d6634109dae5a4806ff94fa398bf56a56bf48be8a4bf86c89e03a83916574830dc839c1349f0649068f7b944e1d3884d16ca2620b2971a254c85' '30818702413f50c2033a93c952719dedb5d834c4ac3f924e9847ba4504291c03639fcfa9739aebc2d6fcfc38c05868587fda5ea521b4983bce1bf5b098988c22a571cb638619024201835e24b1db612c33d073c005fa0a8f9f0c5768554363ba510bd012b31f3a42fca4476452d344502e6471a526481cdfe50e316db65719628af4a7bd861a45c3060a' MSIG 'secp384r1' '0463110061e15a51190cef8c3d91f0d705e8a3015d93ba0d39a29cf53272ef075aa258c6a1784f7f42f377dd785ac4ce3771b53105a5e6f27b81772370315927b9fbd4d7bbc8f09daa492a946184e76ba7c5cd12cf48d70da86ef6d7a1cdcde8fd' '306402302ea988d7dfacfd6476b84f5400d965a979c90b2bc08882c2433a3dd06b52ba1b89e74dd1eea1a8df61353a6f1194af1a023041d1be206ae83dbd1082cb4bf138b0dfd40bfc9d61a6888bc0931b675a7e52fd0da80d818e9aaa488868df69cf9cdc7f' MSIG 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402203a0b9574eba2979de9952e0ab57b06664f275179c8854e73c3ec1cdfa2db0ed20220732b4e917e91af623d09b6d62bac017bfd95f3458f0c5dbc52d32604cc43cbf4' MSIG %> [] '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' +! '304402203a0b9574eba2979de9952e0ab57b06664f275179c8854e73c3ec1cdfa2db0ed20220732b4e917e91af623d09b6d62bac017bfd95f3458f0c5dbc52d32604cc43cbf4' +! ",
    ),
    (
        r"<% 1 <% 2 %> EVAL + 'x%20y' 3.5 [ 1 2 ] { 'a' 'b' } 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '3046022100a598e4d0fa604bb1e0df0c6d4c4114a28a28404f0d482ba7a78af5379a51896b022100ea972315235d34e8df78cae59567ab18c78239e9847f26899742f68189bf3780' MSIG %> MSIGCOUNT SNAPSHOT",
        r"1 ",
    ),
    (
        r"<% 1 <% 2 %> EVAL + 'x%20y' 3.5 [ 1 2 ] { 'a' 'b' } 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '3046022100a598e4d0fa604bb1e0df0c6d4c4114a28a28404f0d482ba7a78af5379a51896b022100ea972315235d34e8df78cae59567ab18c78239e9847f26899742f68189bf3780' MSIG %> MCHECKSIG SNAPSHOT",
        r"<% 1 <% 2 %> EVAL + 'x%20y' 3.5 [ 1 2 ] { 'a' 'b' } 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '3046022100a598e4d0fa604bb1e0df0c6d4c4114a28a28404f0d482ba7a78af5379a51896b022100ea972315235d34e8df78cae59567ab18c78239e9847f26899742f68189bf3780' MSIG %> true ",
    ),
    (
        r"<% 1 <% 2 %> EVAL + 'x%20y' 3.5 [ 1 2 ] { 'a' 'b' } 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '3046022100a598e4d0fa604bb1e0df0c6d4c4114a28a28404f0d482ba7a78af5379a51896b022100ea972315235d34e8df78cae59567ab18c78239e9847f26899742f68189bf3780' MSIG %> MVERIFY SNAPSHOT",
        r"<% 1 <% 2 %> EVAL + 'x%20y' 3.5 [ 1 2 ] { 'a' 'b' } 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '3046022100a598e4d0fa604bb1e0df0c6d4c4114a28a28404f0d482ba7a78af5379a51896b022100ea972315235d34e8df78cae59567ab18c78239e9847f26899742f68189bf3780' MSIG %> ",
    ),
    (
        r"<% 1 <% 2 %> EVAL + 'x%20y' 3.5 [ 1 2 ] { 'a' 'b' } 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '3046022100a598e4d0fa604bb1e0df0c6d4c4114a28a28404f0d482ba7a78af5379a51896b022100ea972315235d34e8df78cae59567ab18c78239e9847f26899742f68189bf3780' MSIG %> MDETACH SNAPSHOT",
        r"<% 1 <% 2 %> EVAL + 'x%20y' 3.5 [ 1 2 ] { 'a' 'b' } %> <% 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '3046022100a598e4d0fa604bb1e0df0c6d4c4114a28a28404f0d482ba7a78af5379a51896b022100ea972315235d34e8df78cae59567ab18c78239e9847f26899742f68189bf3780' MSIG %> ",
    ),
    (
        r"<% 1 <% 2 %> EVAL + 'x%20y' 3.5 [ 1 2 ] { 'a' 'b' } 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '3046022100a598e4d0fa604bb1e0df0c6d4c4114a28a28404f0d482ba7a78af5379a51896b022100ea972315235d34e8df78cae59567ab18c78239e9847f26899742f68189bf3780' MSIG %> MSIG SNAPSHOT",
        r"<% 1 <% 2 %> EVAL + 'x%20y' 3.5 [ 1 2 ] { 'a' 'b' } 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '3046022100a598e4d0fa604bb1e0df0c6d4c4114a28a28404f0d482ba7a78af5379a51896b022100ea972315235d34e8df78cae59567ab18c78239e9847f26899742f68189bf3780' MSIG %> <% 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '3046022100a598e4d0fa604bb1e0df0c6d4c4114a28a28404f0d482ba7a78af5379a51896b022100ea972315235d34e8df78cae59567ab18c78239e9847f26899742f68189bf3780' MSIG %> ",
    ),
    (
        r"<% 1 <% 2 %> EVAL + 'x%20y' 3.5 [ 1 2 ] { 'a' 'b' } 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '3046022100a598e4d0fa604bb1e0df0c6d4c4114a28a28404f0d482ba7a78af5379a51896b022100ea972315235d34e8df78cae59567ab18c78239e9847f26899742f68189bf3780' MSIG %> EVAL SNAPSHOT",
        r"3 'x%20y' 3.5 [] 1 +! 2 +! {} 'b' 'a' PUT ",
    ),
    (
        r"<% 1 <% 2 %> EVAL + 'x%20y' 3.5 [ 1 2 ] { 'a' 'b' } 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '3046022100a598e4d0fa604bb1e0df0c6d4c4114a28a28404f0d482ba7a78af5379a51896b022100ea972315235d34e8df78cae59567ab18c78239e9847f26899742f68189bf3780' MSIG %> MSIGINFO DUP TYPEOF <% 'MAP' == %> <% DUP 'key' GET ECPUBLIC 'Q' GET SWAP 'sig' GET ->HEX 2 ->LIST %> <% %> IFTE SNAPSHOT",
        r"<% 1 <% 2 %> EVAL + 'x%20y' 3.5 [ 1 2 ] { 'a' 'b' } 'secp256r1' '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' '3046022100a598e4d0fa604bb1e0df0c6d4c4114a28a28404f0d482ba7a78af5379a51896b022100ea972315235d34e8df78cae59567ab18c78239e9847f26899742f68189bf3780' MSIG %> [] '04100effdbc0e5a386048b0931a3a3bc9949d3875670819d1bf7c43440fa16ea8fe73a61ddc1077ebf523c816192976168ddf66fb5ac1c039b820950732690deba' +! '3046022100a598e4d0fa604bb1e0df0c6d4c4114a28a28404f0d482ba7a78af5379a51896b022100ea972315235d34e8df78cae59567ab18c78239e9847f26899742f68189bf3780' +! ",
    ),
    (
        r"<% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3046022100f0d2bca228c883a279a65eb4e46545e0fb271f329c8a39a752dbd9023eb1106b02210090824275556b1e018a394de9b8606c173906416e585383240c1fd3470ab80776' MSIG %> MSIGCOUNT SNAPSHOT",
        r"1 ",
    ),
    (
        r"<% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3046022100f0d2bca228c883a279a65eb4e46545e0fb271f329c8a39a752dbd9023eb1106b02210090824275556b1e018a394de9b8606c173906416e585383240c1fd3470ab80776' MSIG %> MCHECKSIG SNAPSHOT",
        r"<% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3046022100f0d2bca228c883a279a65eb4e46545e0fb271f329c8a39a752dbd9023eb1106b02210090824275556b1e018a394de9b8606c173906416e585383240c1fd3470ab80776' MSIG %> true ",
    ),
    (
        r"<% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3046022100f0d2bca228c883a279a65eb4e46545e0fb271f329c8a39a752dbd9023eb1106b02210090824275556b1e018a394de9b8606c173906416e585383240c1fd3470ab80776' MSIG %> MVERIFY SNAPSHOT",
        r"<% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3046022100f0d2bca228c883a279a65eb4e46545e0fb271f329c8a39a752dbd9023eb1106b02210090824275556b1e018a394de9b8606c173906416e585383240c1fd3470ab80776' MSIG %> ",
    ),
    (
        r"<% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3046022100f0d2bca228c883a279a65eb4e46545e0fb271f329c8a39a752dbd9023eb1106b02210090824275556b1e018a394de9b8606c173906416e585383240c1fd3470ab80776' MSIG %> MDETACH SNAPSHOT",
        r"<% %> <% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3046022100f0d2bca228c883a279a65eb4e46545e0fb271f329c8a39a752dbd9023eb1106b02210090824275556b1e018a394de9b8606c173906416e585383240c1fd3470ab80776' MSIG %> ",
    ),
    (
        r"<% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3046022100f0d2bca228c883a279a65eb4e46545e0fb271f329c8a39a752dbd9023eb1106b02210090824275556b1e018a394de9b8606c173906416e585383240c1fd3470ab80776' MSIG %> MSIG SNAPSHOT",
        r"<% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3046022100f0d2bca228c883a279a65eb4e46545e0fb271f329c8a39a752dbd9023eb1106b02210090824275556b1e018a394de9b8606c173906416e585383240c1fd3470ab80776' MSIG %> <% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3046022100f0d2bca228c883a279a65eb4e46545e0fb271f329c8a39a752dbd9023eb1106b02210090824275556b1e018a394de9b8606c173906416e585383240c1fd3470ab80776' MSIG %> ",
    ),
    (
        r"<% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3046022100f0d2bca228c883a279a65eb4e46545e0fb271f329c8a39a752dbd9023eb1106b02210090824275556b1e018a394de9b8606c173906416e585383240c1fd3470ab80776' MSIG %> EVAL SNAPSHOT",
        r"",
    ),
    (
        r"<% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3046022100f0d2bca228c883a279a65eb4e46545e0fb271f329c8a39a752dbd9023eb1106b02210090824275556b1e018a394de9b8606c173906416e585383240c1fd3470ab80776' MSIG %> MSIGINFO DUP TYPEOF <% 'MAP' == %> <% DUP 'key' GET ECPUBLIC 'Q' GET SWAP 'sig' GET ->HEX 2 ->LIST %> <% %> IFTE SNAPSHOT",
        r"<% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3046022100f0d2bca228c883a279a65eb4e46545e0fb271f329c8a39a752dbd9023eb1106b02210090824275556b1e018a394de9b8606c173906416e585383240c1fd3470ab80776' MSIG %> [] '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' +! '3046022100f0d2bca228c883a279a65eb4e46545e0fb271f329c8a39a752dbd9023eb1106b02210090824275556b1e018a394de9b8606c173906416e585383240c1fd3470ab80776' +! ",
    ),
    (
        r"<% 1 3 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MSIGCOUNT SNAPSHOT",
        r"1 ",
    ),
    (
        r"<% 1 3 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MCHECKSIG SNAPSHOT",
        r"<% 1 3 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> false ",
    ),
    (
        r"<% 1 3 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MDETACH SNAPSHOT",
        r"<% 1 3 + %> <% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> ",
    ),
    (
        r"<% 1 3 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MSIG SNAPSHOT",
        r"<% 1 3 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> <% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> ",
    ),
    (
        r"<% 1 3 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> EVAL SNAPSHOT",
        r"4 ",
    ),
    (
        r"<% 1 3 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MSIGINFO DUP TYPEOF <% 'MAP' == %> <% DUP 'key' GET ECPUBLIC 'Q' GET SWAP 'sig' GET ->HEX 2 ->LIST %> <% %> IFTE SNAPSHOT",
        r"<% 1 3 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> [] '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' +! '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' +! ",
    ),
    (r"<% 1 2 + %> MSIGCOUNT SNAPSHOT", r"0 "),
    (r"<% 1 2 + %> MCHECKSIG SNAPSHOT", r"<% 1 2 + %> false "),
    (r"<% 1 2 + %> MDETACH SNAPSHOT", r"<% 1 2 + %> <% %> "),
    (r"<% 1 2 + %> MSIG SNAPSHOT", r"<% 1 2 + %> <% %> "),
    (
        r"<% 1 2 + %> MSIGINFO DUP TYPEOF <% 'MAP' == %> <% DUP 'key' GET ECPUBLIC 'Q' GET SWAP 'sig' GET ->HEX 2 ->LIST %> <% %> IFTE SNAPSHOT",
        r"false ",
    ),
    (r"<% 1 'a' 'b' 'c' %> MSIGCOUNT SNAPSHOT", r"0 "),
    (
        r"<% 1 'a' 'b' 'c' %> MCHECKSIG SNAPSHOT",
        r"<% 1 'a' 'b' 'c' %> false ",
    ),
    (
        r"<% 1 'a' 'b' 'c' %> MDETACH SNAPSHOT",
        r"<% 1 'a' 'b' 'c' %> <% %> ",
    ),
    (
        r"<% 1 'a' 'b' 'c' %> MSIG SNAPSHOT",
        r"<% 1 'a' 'b' 'c' %> <% %> ",
    ),
    (
        r"<% 1 'a' 'b' 'c' %> MSIGINFO DUP TYPEOF <% 'MAP' == %> <% DUP 'key' GET ECPUBLIC 'Q' GET SWAP 'sig' GET ->HEX 2 ->LIST %> <% %> IFTE SNAPSHOT",
        r"false ",
    ),
    (r"<% 'a' 'b' 'c' MSIG %> MSIGCOUNT SNAPSHOT", r"1 "),
    (
        r"<% 'a' 'b' 'c' MSIG %> MDETACH SNAPSHOT",
        r"<% %> <% 'a' 'b' 'c' MSIG %> ",
    ),
    (
        r"<% 'a' 'b' 'c' MSIG %> MSIG SNAPSHOT",
        r"<% 'a' 'b' 'c' MSIG %> <% 'a' 'b' 'c' MSIG %> ",
    ),
    (r"<% 'a' 'b' 'c' MSIG %> EVAL SNAPSHOT", r""),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' %> MSIGCOUNT SNAPSHOT",
        r"0 ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' %> MCHECKSIG SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' %> false ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' %> MDETACH SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' %> <% %> ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' %> MSIG SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' %> <% %> ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' %> MSIGINFO DUP TYPEOF <% 'MAP' == %> <% DUP 'key' GET ECPUBLIC 'Q' GET SWAP 'sig' GET ->HEX 2 ->LIST %> <% %> IFTE SNAPSHOT",
        r"false ",
    ),
    (
        r"<% 1 2 + 'nope' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MSIGCOUNT SNAPSHOT",
        r"1 ",
    ),
    (
        r"<% 1 2 + 'nope' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MDETACH SNAPSHOT",
        r"<% 1 2 + %> <% 'nope' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> ",
    ),
    (
        r"<% 1 2 + 'nope' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MSIG SNAPSHOT",
        r"<% 1 2 + 'nope' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> <% 'nope' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> ",
    ),
    (
        r"<% 1 2 + 'nope' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> EVAL SNAPSHOT",
        r"3 ",
    ),
    (
        r"<% 1 2 + 'secp256k1' 'zz' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MSIGCOUNT SNAPSHOT",
        r"1 ",
    ),
    (
        r"<% 1 2 + 'secp256k1' 'zz' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MDETACH SNAPSHOT",
        r"<% 1 2 + %> <% 'secp256k1' 'zz' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> ",
    ),
    (
        r"<% 1 2 + 'secp256k1' 'zz' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MSIG SNAPSHOT",
        r"<% 1 2 + 'secp256k1' 'zz' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> <% 'secp256k1' 'zz' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> ",
    ),
    (
        r"<% 1 2 + 'secp256k1' 'zz' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> EVAL SNAPSHOT",
        r"3 ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '0401' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MSIGCOUNT SNAPSHOT",
        r"1 ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '0401' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MDETACH SNAPSHOT",
        r"<% 1 2 + %> <% 'secp256k1' '0401' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '0401' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MSIG SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '0401' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> <% 'secp256k1' '0401' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '0401' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> EVAL SNAPSHOT",
        r"3 ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3006020101020101' MSIG %> MSIGCOUNT SNAPSHOT",
        r"1 ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3006020101020101' MSIG %> MCHECKSIG SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3006020101020101' MSIG %> false ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3006020101020101' MSIG %> MDETACH SNAPSHOT",
        r"<% 1 2 + %> <% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3006020101020101' MSIG %> ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3006020101020101' MSIG %> MSIG SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3006020101020101' MSIG %> <% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3006020101020101' MSIG %> ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3006020101020101' MSIG %> EVAL SNAPSHOT",
        r"3 ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3006020101020101' MSIG %> MSIGINFO DUP TYPEOF <% 'MAP' == %> <% DUP 'key' GET ECPUBLIC 'Q' GET SWAP 'sig' GET ->HEX 2 ->LIST %> <% %> IFTE SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3006020101020101' MSIG %> [] '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' +! '3006020101020101' +! ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'ab' MSIG %> MSIGCOUNT SNAPSHOT",
        r"1 ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'ab' MSIG %> MDETACH SNAPSHOT",
        r"<% 1 2 + %> <% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'ab' MSIG %> ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'ab' MSIG %> MSIG SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'ab' MSIG %> <% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'ab' MSIG %> ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'ab' MSIG %> EVAL SNAPSHOT",
        r"3 ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'ab' MSIG %> MSIGINFO DUP TYPEOF <% 'MAP' == %> <% DUP 'key' GET ECPUBLIC 'Q' GET SWAP 'sig' GET ->HEX 2 ->LIST %> <% %> IFTE SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'ab' MSIG %> [] '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' +! 'ab' +! ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'abc' MSIG %> MSIGCOUNT SNAPSHOT",
        r"1 ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'abc' MSIG %> MDETACH SNAPSHOT",
        r"<% 1 2 + %> <% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'abc' MSIG %> ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'abc' MSIG %> MSIG SNAPSHOT",
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'abc' MSIG %> <% 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'abc' MSIG %> ",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'abc' MSIG %> EVAL SNAPSHOT",
        r"3 ",
    ),
];

const MACROS_ERRORS: &[(&str, &str)] = &[
    (
        r"<% 1 3 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MVERIFY",
        r"MVERIFY unable to verify macro.",
    ),
    (r"<% 1 2 + %> MVERIFY", r"MVERIFY unable to verify macro."),
    (
        r"<% 1 'a' 'b' 'c' %> MVERIFY",
        r"MVERIFY unable to verify macro.",
    ),
    (r"<% 'a' 'b' 'c' MSIG %> MCHECKSIG", r"NullPointerException"),
    (r"<% 'a' 'b' 'c' MSIG %> MVERIFY", r"NullPointerException"),
    (r"<% 'a' 'b' 'c' MSIG %> MSIGINFO", r"NullPointerException"),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' %> MVERIFY",
        r"MVERIFY unable to verify macro.",
    ),
    (
        r"<% 1 2 + 'nope' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MCHECKSIG",
        r"NullPointerException",
    ),
    (
        r"<% 1 2 + 'nope' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MVERIFY",
        r"NullPointerException",
    ),
    (
        r"<% 1 2 + 'nope' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MSIGINFO",
        r"NullPointerException",
    ),
    (
        r"<% 1 2 + 'secp256k1' 'zz' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MCHECKSIG",
        r"exception decoding Hex string: invalid characters encountered in Hex string (invalid characters encountered in Hex string)",
    ),
    (
        r"<% 1 2 + 'secp256k1' 'zz' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MVERIFY",
        r"exception decoding Hex string: invalid characters encountered in Hex string (invalid characters encountered in Hex string)",
    ),
    (
        r"<% 1 2 + 'secp256k1' 'zz' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MSIGINFO",
        r"exception decoding Hex string: invalid characters encountered in Hex string (invalid characters encountered in Hex string)",
    ),
    (
        r"<% 1 2 + 'secp256k1' '0401' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MCHECKSIG",
        r"Incorrect length for uncompressed encoding",
    ),
    (
        r"<% 1 2 + 'secp256k1' '0401' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MVERIFY",
        r"Incorrect length for uncompressed encoding",
    ),
    (
        r"<% 1 2 + 'secp256k1' '0401' '304402204e2866a7c7a601f445cb2ff49985b58fa1a597bd15ff16158b1f2fbd846f62ff0220320c0c8314b12c4a8e27e24d435e3f307da03dc1e042877f7f46f0637f67a9b3' MSIG %> MSIGINFO",
        r"Incorrect length for uncompressed encoding",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' '3006020101020101' MSIG %> MVERIFY",
        r"MVERIFY unable to verify macro.",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'ab' MSIG %> MCHECKSIG",
        r"ECVERIFY error while verifying signature. (error decoding signature bytes.)",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'ab' MSIG %> MVERIFY",
        r"ECVERIFY error while verifying signature. (error decoding signature bytes.)",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'abc' MSIG %> MCHECKSIG",
        r"exception decoding Hex string: String index out of range: 3 (String index out of range: 3)",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'abc' MSIG %> MVERIFY",
        r"exception decoding Hex string: String index out of range: 3 (String index out of range: 3)",
    ),
    (
        r"<% 1 2 + 'secp256k1' '04f01d6b9018ab421dd410404cb869072065522bf85734008f105cf385a023a80f0eba29d0f0c5408ed681984dc525982abefccd9f7ff01dd26da4999cf3f6a295' 'abc' MSIG %> MSIGINFO",
        r"exception decoding Hex string: String index out of range: 3 (String index out of range: 3)",
    ),
    (r"MSIGN", r"Empty stack."),
    (r"1 MSIGN", r"MSIGN expects an ECC private key."),
    (
        r"{ 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE MSIGN",
        r"Empty stack.",
    ),
    (
        r"1 { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE MSIGN",
        r"MSIGN operates on a macro.",
    ),
    (r"<% 1 %> 1 MSIGN", r"MSIGN expects an ECC private key."),
    (
        r"<% 1 %> { 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE ECPUBLIC MSIGN",
        r"MSIGN expects an ECC private key.",
    ),
    (
        r"<% 1 %> { 'curve' 'secp256k1' 'd' '0' } ECPRIVATE MSIGN",
        r"Scalar is not in the interval [1, n - 1]",
    ),
    (r"MSIG", r"Empty stack."),
    (r"1 MSIG", r"MSIG expects a hex encoded signature."),
    (r"'a' MSIG", r"Empty stack."),
    (r"'a' 'b' MSIG", r"Empty stack."),
    (
        r"'a' 1 'c' MSIG",
        r"MSIG expects a hex encoded ECC public key.",
    ),
    (r"1 'b' 'c' MSIG", r"MSIG expects an ECC curve name."),
    (r"'c' MSIG", r"Empty stack."),
    (r"MSIGCOUNT", r"Empty stack."),
    (r"1 MSIGCOUNT", r"MSIGCOUNT operates on a Macro."),
    (r"MSIGINFO", r"Empty stack."),
    (r"1 MSIGINFO", r"MSIGINFO expects a macro."),
    (r"MVERIFY", r"Empty stack."),
    (r"1 MVERIFY", r"MVERIFY operates on a macro."),
    (r"MCHECKSIG", r"Empty stack."),
    (r"1 MCHECKSIG", r"MCHECKSIG operates on a macro."),
    (r"MDETACH", r"Empty stack."),
    (r"1 MDETACH", r"MDETACH operates on a macro."),
];

/// The scripts of a table give what Warp 10 gave.
fn assert_values(table: &[(&str, &str)]) {
    for (script, expected) in table {
        assert_top(script, *expected);
    }
}

/// The scripts of a table fail with what Warp 10 said.
fn assert_all_fail(table: &[(&str, &str)]) {
    for (script, message) in table {
        assert_fails(script, message);
    }
}

#[test]
fn keys_are_those_of_warp_10() {
    assert_values(KEYS_VALUES);
    assert_all_fail(KEYS_ERRORS);
}

#[test]
fn agreement_are_those_of_warp_10() {
    assert_values(AGREEMENT_VALUES);
    assert_all_fail(AGREEMENT_ERRORS);
}

#[test]
fn signatures_are_those_of_warp_10() {
    assert_values(SIGNATURES_VALUES);
    assert_all_fail(SIGNATURES_ERRORS);
}

#[test]
fn recover_are_those_of_warp_10() {
    assert_values(RECOVER_VALUES);
    assert_all_fail(RECOVER_ERRORS);
}

#[test]
fn macros_are_those_of_warp_10() {
    assert_values(MACROS_VALUES);
    assert_all_fail(MACROS_ERRORS);
}

/// A source of random bytes that delivers `bytes`, then fails.
fn scripted(bytes: Vec<u8>) -> impl FnMut(&mut [u8]) -> Result<(), getrandom::Error> {
    let mut offset = 0;
    move |buffer: &mut [u8]| {
        let chunk = bytes
            .get(offset..offset + buffer.len())
            .ok_or(getrandom::Error::UNSUPPORTED)?;
        buffer.copy_from_slice(chunk);
        offset += buffer.len();
        Ok(())
    }
}

/// `size` bytes holding the number `n`.
fn number(n: u8, size: usize) -> Vec<u8> {
    let mut bytes = vec![0; size];
    bytes[size - 1] = n;
    bytes
}

const CURVES: [(&str, usize); 4] = [
    ("secp256k1", 32),
    ("secp256r1", 32),
    ("secp384r1", 48),
    ("secp521r1", 66),
];

/// The script that makes the public key of the private number `d` on a curve, in the
/// round trip through its map that Warp 10 needs to verify with it.
fn public_of(curve: &str, d: &str) -> String {
    format!("{{ 'curve' '{curve}' 'd' '{d}' }} ECPRIVATE ECPUBLIC ECPUBLIC ECPUBLIC")
}

#[test]
fn a_key_pair_is_generated_from_random_bytes() {
    for (curve, size) in CURVES {
        let mut stack = Stack::new();
        stack.push(Value::string(curve));
        // A number that is too small is drawn again; then the private number.
        let mut random = scripted([number(1, size), number(7, size)].concat());
        ecgen_with(&mut stack, &mut random).expect("a key pair");
        assert_eq!(stack.depth(), 2);
        let public = stack.pop().expect("the public key");
        let private = stack.pop().expect("the private key");
        let json = |value: Value| {
            let mut stack = Stack::new();
            stack.push(value);
            stack.exec("->JSON").expect("JSON");
            stack.pop().expect("a text").to_string()
        };
        assert_eq!(
            json(private),
            format!("{{\"curve\":\"{curve}\",\"d\":\"7\"}}")
        );
        let expected = top(&format!("{} ECPUBLIC 'Q' GET", {
            format!("{{ 'curve' '{curve}' 'd' '7' }} ECPRIVATE ECPUBLIC")
        }));
        assert_eq!(
            json(public),
            format!("{{\"curve\":\"{curve}\",\"Q\":{}}}", json(expected))
        );
    }
}

#[test]
fn a_key_pair_keeps_the_name_of_the_curve() {
    let mut stack = Stack::new();
    stack.push(Value::string("P-256"));
    ecgen_with(&mut stack, &mut scripted(number(9, 32))).expect("a key pair");
    stack.exec("'curve' GET SWAP 'curve' GET").expect("names");
    assert_eq!(stack.pop().expect("a name"), Value::string("P-256"));
    assert_eq!(stack.pop().expect("a name"), Value::string("P-256"));
}

#[test]
fn a_key_pair_cannot_be_generated_without_random_bytes() {
    let mut stack = Stack::new();
    stack.push(Value::string("secp256k1"));
    let err = ecgen_with(&mut stack, &mut scripted(Vec::new())).expect_err("no entropy");
    assert!(
        err.to_string().contains("Unable to obtain random data"),
        "{err}"
    );
}

#[test]
fn what_is_signed_here_verifies_here() {
    const ALGORITHMS: [&str; 14] = [
        "ECDSA",
        "SHA1WITHECDSA",
        "SHA224WITHECDSA",
        "SHA256WITHECDSA",
        "SHA384WITHECDSA",
        "SHA512WITHECDSA",
        "SHA3-224WITHECDSA",
        "SHA3-256WITHECDSA",
        "SHA3-384WITHECDSA",
        "SHA3-512WITHECDSA",
        "RIPEMD160WITHECDSA",
        "NONEWITHECDSA",
        "sha256withecdsa",
        "Sha256WithEcdsa",
    ];
    for (curve, _) in CURVES {
        let private = format!("{{ 'curve' '{curve}' 'd' '31337' }} ECPRIVATE");
        let public = public_of(curve, "31337");
        for algorithm in ALGORITHMS {
            let sign = format!("'hello' 'UTF-8' ->BYTES '{algorithm}' {private} ECSIGN");
            let verify = |data: &str| {
                top(&format!(
                    "{data} 'UTF-8' ->BYTES {sign} '{algorithm}' {public} ECVERIFY"
                ))
            };
            assert_eq!(
                verify("'hello'"),
                Value::Boolean(true),
                "{curve} {algorithm}"
            );
            assert_eq!(
                verify("'hellp'"),
                Value::Boolean(false),
                "{curve} {algorithm}"
            );
        }
    }
}

#[test]
fn a_signature_is_made_with_the_random_bytes_it_is_given() {
    for (curve, size) in CURVES {
        let mut stack = Stack::new();
        stack.push(Value::bytes(b"hello"));
        stack.push(Value::string("SHA256WITHECDSA"));
        stack
            .exec(&format!("{{ 'curve' '{curve}' 'd' '31337' }} ECPRIVATE"))
            .expect("a key");
        ecsign_with(&mut stack, &mut scripted(number(5, size))).expect("a signature");
        let first = stack.pop().expect("a signature");
        let mut stack = Stack::new();
        stack.push(Value::bytes(b"hello"));
        stack.push(Value::string("SHA256WITHECDSA"));
        stack
            .exec(&format!("{{ 'curve' '{curve}' 'd' '31337' }} ECPRIVATE"))
            .expect("a key");
        ecsign_with(&mut stack, &mut scripted(number(5, size))).expect("a signature");
        assert_eq!(
            stack.pop().expect("a signature"),
            first,
            "{curve}: the same nonce, the same signature"
        );
    }
}

#[test]
fn a_signature_cannot_be_made_without_random_bytes() {
    let mut stack = Stack::new();
    stack.push(Value::bytes(b"hello"));
    stack.push(Value::string("SHA256WITHECDSA"));
    stack
        .exec("{ 'curve' 'secp256k1' 'd' '31337' } ECPRIVATE")
        .expect("a key");
    let err = ecsign_with(&mut stack, &mut scripted(Vec::new())).expect_err("no entropy");
    assert!(
        err.to_string()
            .contains("ECSIGN error signing content. (Unable to obtain random data"),
        "{err}"
    );
}

#[test]
fn a_macro_signed_here_is_verified_here() {
    for (curve, size) in CURVES {
        let key = format!("{{ 'curve' '{curve}' 'd' '4242' }} ECPRIVATE");
        let mut stack = Stack::new();
        stack.exec("<% 1 2 + 'text' %>").expect("a macro");
        stack.exec(&key).expect("a key");
        msign_with(&mut stack, &mut scripted(number(3, size))).expect("a signature");
        stack.exec("+").expect("the signed macro");
        stack.exec("DUP MSIGCOUNT SWAP MCHECKSIG").expect("checks");
        assert_eq!(
            stack.pop().expect("verdict"),
            Value::Boolean(true),
            "{curve}"
        );
        // Signed again: the second signature covers the first.
        stack.exec(&key).expect("a key");
        msign_with(&mut stack, &mut scripted(number(4, size))).expect("a signature");
        stack
            .exec("+ DUP MSIGCOUNT SWAP MCHECKSIG")
            .expect("verified");
        assert_eq!(
            stack.pop().expect("verdict"),
            Value::Boolean(true),
            "{curve}"
        );
        stack.exec("DROP").expect("the macro dropped");
        assert_eq!(stack.pop().expect("count"), Value::Long(2), "{curve}");
        assert_eq!(stack.pop().expect("count"), Value::Long(1), "{curve}");
    }
}

#[test]
fn a_macro_cannot_be_signed_without_random_bytes() {
    let mut stack = Stack::new();
    stack.exec("<% 1 %>").expect("a macro");
    stack
        .exec("{ 'curve' 'secp256k1' 'd' '4242' } ECPRIVATE")
        .expect("a key");
    let err = msign_with(&mut stack, &mut scripted(Vec::new())).expect_err("no entropy");
    assert!(
        err.to_string()
            .contains("ECSIGN error signing content. (Unable to obtain random data"),
        "{err}"
    );
}

#[test]
fn a_macro_signed_with_the_system_randomness_is_verified() {
    for curve in ["secp256k1", "secp521r1"] {
        let script = format!(
            "<% 1 2 + %> {{ 'curve' '{curve}' 'd' '4242' }} ECPRIVATE MSIGN + MVERIFY EVAL"
        );
        assert_eq!(top(&script), Value::Long(3), "{curve}");
    }
}

#[test]
fn an_attached_signature_is_detached() {
    assert_eq!(
        top(
            "<% 1 2 + %> { 'curve' 'secp256k1' 'd' '4242' } ECPRIVATE MSIGN + MDETACH DROP SNAPSHOT"
        ),
        Value::string("<% 1 2 + %> ")
    );
    assert_eq!(
        top(
            "<% 1 2 + %> { 'curve' 'secp256k1' 'd' '4242' } ECPRIVATE MSIGN + MDETACH DROP MSIGCOUNT"
        ),
        Value::Long(0)
    );
}

#[test]
fn a_secure_macro_stays_secure_without_its_signature() {
    let macro_ = Macro::from_statements(vec![Statement::Push(Value::Long(1))]);
    macro_.secure.set(true);
    let mut stack = Stack::new();
    stack.push(Value::Macro(Rc::new(macro_)));
    stack
        .exec("{ 'curve' 'secp256k1' 'd' '4242' } ECPRIVATE MSIGN + MDETACH DROP")
        .expect("detached");
    let Value::Macro(macro_) = stack.pop().expect("a macro") else {
        panic!("a macro is expected");
    };
    assert!(macro_.is_secure());
}

#[test]
fn keys_are_objects() {
    let key = "{ 'curve' 'secp256k1' 'd' '5' } ECPRIVATE";
    assert_eq!(top(&format!("{key} DUP ==")), Value::Boolean(true));
    assert_eq!(top(&format!("{key} {key} ==")), Value::Boolean(false));
    assert_eq!(top(&format!("{key} ->JSON")), Value::string("null"));
    assert_eq!(top(&format!("[ {key} ] ->JSON")), Value::string("[null]"));
    assert_eq!(
        top(key).to_string(),
        "io.warp10.script.functions.ECPRIVATE$1@0"
    );
    assert_eq!(top(&format!("{{ {key} 1 }} SIZE")), Value::Long(1));
    assert_fails(&format!("{key} SIZE"), "SIZE operates on");
    // A key does not give its secret away when it is pickled.
    let pickled = top(&format!("{key} ->PICKLE ->HEX"));
    let Value::String(hex) = pickled else {
        panic!("a text is expected");
    };
    assert!(!hex.contains("secp256k1"), "{hex}");
}

#[test]
fn a_private_key_is_made_of_its_parts() {
    let mut stack = exec("{ 'curve' 'secp256k1' 'd' '5' } ECPRIVATE");
    let Value::Key(key) = stack.pop().expect("a key") else {
        panic!("a key is expected");
    };
    assert_eq!(
        *key,
        Key::Private(PrivateKey {
            curve: Curve::Secp256k1,
            name: "secp256k1".to_owned(),
            d: BigInt::from(5),
        })
    );
}

#[test]
fn a_failure_is_what_the_error_of_the_stack_says() {
    let err: Error = crate::tests::fail("'secp256k1' ECGEN 'x' ECGEN");
    assert!(
        err.to_string()
            .contains("ECGEN only supports the following curves: secp256k1, secp256r1"),
        "{err}"
    );
}

#[test]
fn the_arguments_of_a_signature_are_checked_in_order() {
    let private = "{ 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE";
    let public = public_of("secp256k1", "12345");
    assert_fails(
        &format!("1 'SHA256WITHECDSA' {private} ECSIGN"),
        "ECSIGN operates on a byte array.",
    );
    assert_fails(
        &format!("'a' 'SHA256WITHECDSA' {private} ECSIGN"),
        "ECSIGN operates on a byte array.",
    );
    assert_fails(
        &format!("'00' HEX-> 1 {public} ECVERIFY"),
        "ECVERIFY expects an algorithm name.",
    );
    assert_fails(
        &format!("1 '3006020101020101' HEX-> 'SHA256WITHECDSA' {public} ECVERIFY"),
        "ECVERIFY operates on a byte array.",
    );
    assert_fails(
        &format!("'a' '3006020101020101' HEX-> 'SHA256WITHECDSA' {public} ECVERIFY"),
        "ECVERIFY operates on a byte array.",
    );
}

#[test]
fn a_signature_needs_all_its_arguments() {
    let private = "{ 'curve' 'secp256k1' 'd' '12345' } ECPRIVATE";
    let public = public_of("secp256k1", "12345");
    for script in [
        format!("{private} ECSIGN"),
        format!("'SHA256WITHECDSA' {private} ECSIGN"),
        format!("{public} ECVERIFY"),
        format!("'SHA256WITHECDSA' {public} ECVERIFY"),
        format!("'00' HEX-> 'SHA256WITHECDSA' {public} ECVERIFY"),
    ] {
        assert_fails(&script, "Empty stack.");
    }
}
