#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::encoding::{hex_decode, hex_encode, opb64_decode, opb64_encode};

#[test]
fn opb64_round_trips() {
    for data in [
        &b""[..],
        b"f",
        b"fo",
        b"foo",
        b"foob",
        b"fooba",
        b"foobar",
        &[0u8, 255, 128, 1, 2, 3, 4],
    ] {
        let encoded = opb64_encode(data);
        assert_eq!(opb64_decode(&encoded).unwrap(), data, "{encoded}");
    }
}

#[test]
fn opb64_uses_the_order_preserving_alphabet() {
    // Standard base64 of "foobar" is "Zm9vYmFy"; OPB64 maps each digit to the sorted alphabet.
    assert_eq!(opb64_encode(b"foobar"), "OaxjNa4m");
    assert_eq!(opb64_encode(b"f"), "OV");
    assert_eq!(opb64_encode(b"fo"), "Oaw");
    assert!(opb64_encode(&[0u8]) < opb64_encode(&[1u8]));
    assert!(opb64_encode(b"abc") < opb64_encode(b"abd"));
    assert_eq!(opb64_decode("_.").unwrap(), vec![0x94]);
    assert_eq!(opb64_encode(&[0x94]), "_.");
}

#[test]
fn opb64_rejects_invalid_characters() {
    let err = opb64_decode("ab*c").unwrap_err();
    assert_eq!(err.to_string(), "Invalid OPB64 character '*' at offset 2.");
}

#[test]
fn hex_round_trips() {
    assert_eq!(hex_encode(&[0x00, 0xAB, 0xff]), "00abff");
    assert_eq!(hex_decode("00abFF").unwrap(), vec![0x00, 0xAB, 0xFF]);
    assert_eq!(hex_decode("").unwrap(), Vec::<u8>::new());
    assert_eq!(
        hex_decode("abc").unwrap_err().to_string(),
        "Odd number of hexadecimal digits."
    );
    assert_eq!(
        hex_decode("zz").unwrap_err().to_string(),
        "Invalid hexadecimal digit."
    );
}
