// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Bouncy Castle OpenPGP (Copyright (c) 2000-2024 The Legion of the Bouncy Castle Inc., MIT licence), ported to Rust and modified.

//! Tests of the ASCII armor: the decoding of `PGPUtil.getDecoderStream` and the encoding of
//! `ArmoredOutputStream`. The expected texts are the ones BouncyCastle writes, computed on the
//! same payloads with an independent CRC-24 and base64.

use crate::error::pgp::ArmorError;
use crate::openpgp::armor::{decode, encode};

/// The payload of the armored blocks of these tests: a public key packet header and counting bytes.
const P1: [u8; 16] = [
    0x99, 0x01, 0x0d, 0x04, 0x00, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11,
];

const PUB_BLOCK: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----\nVersion: BCPG v@RELEASE_NAME@\n\nmQENBAABAgMEBQYHCAkKCw==\n=4xuH\n-----END PGP PUBLIC KEY BLOCK-----\n";
const CRLF_BLOCK: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----\r\nVersion: BCPG v@RELEASE_NAME@\r\n\r\nmQENBAABAgMEBQYHCAkKCw==\r\n=4xuH\r\n-----END PGP PUBLIC KEY BLOCK-----\r\n";
const CR_BLOCK: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----\rVersion: BCPG v@RELEASE_NAME@\r\rmQENBAABAgMEBQYHCAkKCw==\r=4xuH\r-----END PGP PUBLIC KEY BLOCK-----\r";
const NO_CRC_BLOCK: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----\nVersion: BCPG v@RELEASE_NAME@\n\nmQENBAABAgMEBQYHCAkKCw==\n-----END PGP PUBLIC KEY BLOCK-----\n";
const BAD_CRC_BLOCK: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----\nVersion: BCPG v@RELEASE_NAME@\n\nmQENBAABAgMEBQYHCAkKCw==\n=I5f1\n-----END PGP PUBLIC KEY BLOCK-----\n";
const AA_CRC_BLOCK: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----\nVersion: BCPG v@RELEASE_NAME@\n\nmQENBAABAgMEBQYHCAkKCw==\n=AA==\n-----END PGP PUBLIC KEY BLOCK-----\n";
const AAA_CRC_BLOCK: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----\nVersion: BCPG v@RELEASE_NAME@\n\nmQENBAABAgMEBQYHCAkKCw==\n=AAA=\n-----END PGP PUBLIC KEY BLOCK-----\n";
const NO_BLANK_BLOCK: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----\nVersion: BCPG v@RELEASE_NAME@\nmQENBAABAgMEBQYHCAkKCw==\n=4xuH\n-----END PGP PUBLIC KEY BLOCK-----\n";
const BAD_HEADER_BLOCK: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----\nVersion BCPG\n\nmQENBAABAgMEBQYHCAkKCw==\n=4xuH\n-----END PGP PUBLIC KEY BLOCK-----\n";

/// `P1` without its armor headers, a line of base64 with `=` padding; the window holds the `=`.
const BARE_P1: &str = "mQENBAABAgMEBQYHCAkKCw==\n";

/// The 100 bytes `0x99, 0, 1, ..., 98` as one armored block.
const P2_BLOCK: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----\nVersion: BCPG v@RELEASE_NAME@\n\nmQABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fICEiIyQlJicoKSorLC0u\nLzAxMjM0NTY3ODk6Ozw9Pj9AQUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVpbXF1e\nX2BhYg==\n=BiTK\n-----END PGP PUBLIC KEY BLOCK-----\n";
/// The same 100 bytes without armor headers: 64-character lines, then the padded tail.
const P2_BARE: &str = "mQABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fICEiIyQlJicoKSorLC0u\nLzAxMjM0NTY3ODk6Ozw9Pj9AQUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVpbXF1e\nX2BhYg==\n";

fn p2() -> Vec<u8> {
    std::iter::once(0x99u8).chain(0u8..99).collect()
}

fn decode_text(text: &str) -> Result<Vec<u8>, ArmorError> {
    decode(text.as_bytes())
}

fn armored_body(body: &[u8]) -> Vec<u8> {
    let mut text = b"-----BEGIN PGP MESSAGE-----\nVersion: x\n\n".to_vec();
    text.extend_from_slice(body);
    text.extend_from_slice(b"\n-----END PGP MESSAGE-----\n");
    text
}

#[test]
fn raw_packets_are_returned_unchanged() {
    assert_eq!(decode(&[0x99, 1, 2]), Ok(vec![0x99, 1, 2]));
    assert_eq!(decode(&[0xC6, b'-', b'A']), Ok(vec![0xC6, b'-', b'A']));
    assert_eq!(decode(&[0x80]), Ok(vec![0x80]));
}

#[test]
fn empty_input_is_returned_unchanged() {
    assert_eq!(decode(&[]), Ok(Vec::new()));
}

#[test]
fn armored_block_with_headers_decodes_to_its_payload() {
    assert_eq!(decode_text(PUB_BLOCK), Ok(P1.to_vec()));
}

#[test]
fn crlf_and_cr_line_endings_decode() {
    assert_eq!(decode_text(CRLF_BLOCK), Ok(P1.to_vec()));
    assert_eq!(decode_text(CR_BLOCK), Ok(P1.to_vec()));
}

#[test]
fn block_without_crc_or_end_line_decodes() {
    assert_eq!(decode_text(NO_CRC_BLOCK), Ok(P1.to_vec()));
    let without_newline = PUB_BLOCK.trim_end();
    assert_eq!(decode_text(without_newline), Ok(P1.to_vec()));
    let no_end = PUB_BLOCK.replace("-----END PGP PUBLIC KEY BLOCK-----\n", "");
    assert_eq!(decode_text(&no_end), Ok(P1.to_vec()));
}

#[test]
fn spaces_and_vertical_tabs_in_the_body_are_skipped() {
    let text = "-----BEGIN PGP PUBLIC KEY BLOCK-----\nVersion: x\n\nmQEN BAAB\tAgME\x0bBQYH\x0cCAkKCw==\n=4xuH\n-----END PGP PUBLIC KEY BLOCK-----\n";
    assert_eq!(decode_text(text), Ok(P1.to_vec()));
}

#[test]
fn bad_crc_fails_the_crc_check() {
    assert_eq!(decode_text(BAD_CRC_BLOCK), Err(ArmorError::CrcCheckFailed));
}

#[test]
fn crc_of_one_or_two_bytes_is_malformed() {
    assert_eq!(decode_text(AA_CRC_BLOCK), Err(ArmorError::MalformedCrc));
    assert_eq!(decode_text(AAA_CRC_BLOCK), Err(ArmorError::MalformedCrc));
}

#[test]
fn crc_line_cut_short_is_unexpected_end() {
    let text = "-----BEGIN PGP MESSAGE-----\nVersion: x\n\nmQENBAAB\n=4x";
    assert_eq!(decode_text(text), Err(ArmorError::UnexpectedEnd));
}

#[test]
fn header_line_without_colon_is_invalid() {
    assert_eq!(
        decode_text(BAD_HEADER_BLOCK),
        Err(ArmorError::InvalidArmorHeader)
    );
    assert_eq!(
        decode_text(NO_BLANK_BLOCK),
        Err(ArmorError::InvalidArmorHeader)
    );
}

#[test]
fn header_line_without_colon_in_a_later_block_is_invalid() {
    let text = format!("{PUB_BLOCK}{BAD_HEADER_BLOCK}");
    assert_eq!(decode_text(&text), Err(ArmorError::InvalidArmorHeader));
}

#[test]
fn crlf_headers_must_be_followed_by_lf() {
    let text = "-----BEGIN PGP MESSAGE-----\r\nVersion: x\n\nmQEN\n";
    assert_eq!(decode_text(text), Err(ArmorError::InconsistentLineEndings));
}

#[test]
fn crlf_begin_line_then_end_of_input_is_inconsistent() {
    assert_eq!(
        decode_text("-----BEGIN PGP MESSAGE-----\r\n"),
        Err(ArmorError::InconsistentLineEndings)
    );
}

#[test]
fn headers_that_end_at_end_of_input_decode_nothing() {
    assert_eq!(
        decode_text("-----BEGIN PGP MESSAGE-----\nVersion: x\n"),
        Ok(Vec::new())
    );
}

#[test]
fn text_before_begin_is_skipped() {
    let text = format!("junk\nab-cd\n{PUB_BLOCK}");
    assert_eq!(decode_text(&text), Ok(P1.to_vec()));
}

#[test]
fn text_without_begin_decodes_nothing() {
    assert_eq!(decode_text("hello world"), Ok(Vec::new()));
}

#[test]
fn several_blocks_decode_to_their_concatenated_payloads() {
    let text = format!("{PUB_BLOCK}{PUB_BLOCK}");
    let expected: Vec<u8> = P1.iter().chain(P1.iter()).copied().collect();
    assert_eq!(decode_text(&text), Ok(expected));
}

#[test]
fn trailing_text_after_a_block_is_ignored() {
    let text = format!("{PUB_BLOCK}junk\n");
    assert_eq!(decode_text(&text), Ok(P1.to_vec()));
}

#[test]
fn base64_without_headers_decodes() {
    assert_eq!(decode_text(P2_BARE), Ok(p2()));
    assert_eq!(
        decode(b"mQENBGr9z9sB"),
        Ok(vec![153, 1, 13, 4, 106, 253, 207, 219, 1])
    );
}

#[test]
fn base64_without_headers_followed_by_end_line() {
    let text = format!("{P2_BARE}-----END PGP PUBLIC KEY BLOCK-----\n");
    assert_eq!(decode_text(&text), Ok(p2()));
}

#[test]
fn padded_bare_base64_has_headers_and_decodes_nothing() {
    assert_eq!(decode_text(BARE_P1), Ok(Vec::new()));
}

#[test]
fn a_non_base64_byte_inside_the_peek_window_means_headers() {
    let mut text = P2_BARE.to_string();
    text.insert(59, '#');
    assert_eq!(decode_text(&text), Ok(Vec::new()));
}

#[test]
fn a_non_base64_byte_after_the_peek_window_is_body_data() {
    let mut text = P2_BARE.to_string();
    text.insert(60, '#');
    assert_eq!(decode_text(&text), Err(ArmorError::InvalidArmor));
}

#[test]
fn a_head_shorter_than_four_bytes_means_headers() {
    assert_eq!(decode(b"ABC"), Ok(Vec::new()));
}

#[test]
fn an_eight_character_head_without_the_lead_bit_means_headers() {
    assert_eq!(decode(b"ABCDEFGH"), Ok(Vec::new()));
}

#[test]
fn a_head_with_fewer_than_four_valid_characters_is_invalid() {
    assert_eq!(decode(b"A\n\n\n"), Err(ArmorError::Base64InvalidCharacters));
    assert_eq!(
        decode(b"\n\n\n\n"),
        Err(ArmorError::Base64InvalidCharacters)
    );
}

#[test]
fn a_head_of_four_to_seven_valid_characters_is_invalid_at_end() {
    assert_eq!(
        decode(b"ABCD"),
        Err(ArmorError::Base64InvalidCharactersAtEnd)
    );
    assert_eq!(
        decode(b"ABCDEFG"),
        Err(ArmorError::Base64InvalidCharactersAtEnd)
    );
    assert_eq!(
        decode(b"AB\n\n\n\nCD"),
        Err(ArmorError::Base64InvalidCharactersAtEnd)
    );
}

#[test]
fn a_head_starting_with_a_line_break_overruns_the_buffer() {
    assert_eq!(decode(b"\nABCD"), Err(ArmorError::Base64IndexOutOfBounds));
    assert_eq!(
        decode(b"\nABCDEFG"),
        Err(ArmorError::Base64IndexOutOfBounds)
    );
    assert_eq!(
        decode(b"\r\nmQENBG"),
        Err(ArmorError::Base64IndexOutOfBounds)
    );
}

#[test]
fn a_head_with_a_line_break_and_fewer_than_four_characters_is_invalid() {
    assert_eq!(
        decode(b"\nAB\n\n"),
        Err(ArmorError::Base64InvalidCharacters)
    );
}

#[test]
fn a_head_of_eight_valid_characters_is_checked_for_the_lead_bit() {
    assert_eq!(decode(b"ABCDEFGH"), Ok(Vec::new()));
    assert_eq!(decode(b"mQENBGxx"), Ok(vec![153, 1, 13, 4, 108, 113]));
}

#[test]
fn a_leading_line_break_of_the_head_overruns_the_buffer_even_with_base64_after_it() {
    let mut text = P2_BARE.to_string();
    text.insert(0, '\n');
    assert_eq!(decode_text(&text), Err(ArmorError::Base64IndexOutOfBounds));
}

#[test]
fn a_line_break_between_quads_is_skipped() {
    assert_eq!(
        decode(&armored_body(b"mQEN\rBAAB")),
        Ok(vec![153, 1, 13, 4, 0, 1])
    );
}

#[test]
fn a_high_first_byte_is_raw_whatever_follows() {
    let input = vec![0xC6u8; 100];
    assert_eq!(decode(&input), Ok(input.clone()));
}

#[test]
fn masked_tail_does_not_check_its_characters() {
    assert_eq!(decode(&armored_body(b"A#==")), Ok(vec![15]));
}

#[test]
fn three_character_tail_is_decoded() {
    assert_eq!(decode(&armored_body(b"QUI=")), Ok(b"AB".to_vec()));
}

#[test]
fn invalid_character_in_a_three_character_tail_is_invalid() {
    assert_eq!(
        decode(&armored_body(b"Q#I=")),
        Err(ArmorError::InvalidArmor)
    );
}

#[test]
fn invalid_character_in_a_full_quad_is_invalid() {
    assert_eq!(
        decode(&armored_body(b"A#AA")),
        Err(ArmorError::InvalidArmor)
    );
}

#[test]
fn quad_cut_short_is_unexpected_end() {
    let text = "-----BEGIN PGP MESSAGE-----\nVersion: x\n\nQU";
    assert_eq!(decode_text(text), Err(ArmorError::UnexpectedEnd));
}

#[test]
fn quad_cut_short_by_a_high_byte_is_invalid_armor() {
    assert_eq!(
        decode(&armored_body(b"AB\x80CD")),
        Err(ArmorError::InvalidArmor)
    );
}

#[test]
fn a_high_byte_at_the_start_of_a_line_is_invalid_armor() {
    assert_eq!(
        decode(&armored_body(b"\x80AAA")),
        Err(ArmorError::InvalidArmor)
    );
}

#[test]
fn a_high_byte_in_the_crc_line_is_invalid_armor() {
    let text =
        b"-----BEGIN PGP MESSAGE-----\nVersion: x\n\nmQEN\n=\x80AAA\n-----END PGP MESSAGE-----\n";
    assert_eq!(decode(text), Err(ArmorError::InvalidArmor));
}

#[test]
fn a_body_line_that_starts_with_a_dash_is_invalid_armor() {
    assert_eq!(
        decode(&armored_body(b"-AAA")),
        Err(ArmorError::InvalidArmor)
    );
}

#[test]
fn a_crc_line_directly_after_a_break_with_junk_is_invalid_armor() {
    let text = "-----BEGIN PGP MESSAGE-----\nVersion: x\n\nmQEN\n=\n-----END PGP MESSAGE-----\n";
    assert_eq!(decode(text.as_bytes()), Err(ArmorError::InvalidArmor));
}

#[test]
fn a_quad_that_starts_with_equals_is_invalid_armor() {
    assert_eq!(
        decode(&armored_body(b"=AAAA")),
        Err(ArmorError::InvalidArmor)
    );
}

#[test]
fn several_line_breaks_before_a_quad_are_skipped() {
    assert_eq!(decode(&armored_body(b"\n\n\nQUI=")), Ok(b"AB".to_vec()));
}

#[test]
fn an_end_line_with_no_crc_still_ends_the_block() {
    let text = format!("{P2_BARE}-----END PGP PUBLIC KEY BLOCK-----");
    assert_eq!(decode_text(&text), Ok(p2()));
}

#[test]
fn encode_writes_the_block_of_the_public_key() {
    assert_eq!(encode(&P1), PUB_BLOCK);
}

#[test]
fn encode_wraps_the_body_at_64_characters() {
    assert_eq!(encode(&p2()), P2_BLOCK);
}

#[test]
fn encode_of_no_data_is_a_message_with_the_crc_of_nothing() {
    assert_eq!(
        encode(&[]),
        "-----BEGIN PGP MESSAGE-----\nVersion: BCPG v@RELEASE_NAME@\n\n=twTO\n-----END PGP MESSAGE-----\n"
    );
}

#[test]
fn encode_names_the_block_after_the_packet_tag_of_the_first_byte() {
    let block_of = |first: u8| encode(&[first]);
    assert!(block_of(0x99).starts_with("-----BEGIN PGP PUBLIC KEY BLOCK-----\n"));
    assert!(block_of(0xC6).starts_with("-----BEGIN PGP PUBLIC KEY BLOCK-----\n"));
    assert!(block_of(0x95).starts_with("-----BEGIN PGP PRIVATE KEY BLOCK-----\n"));
    assert!(block_of(0xC5).starts_with("-----BEGIN PGP PRIVATE KEY BLOCK-----\n"));
    assert!(block_of(0x89).starts_with("-----BEGIN PGP SIGNATURE-----\n"));
    assert!(block_of(0xC2).starts_with("-----BEGIN PGP SIGNATURE-----\n"));
    assert!(block_of(0x8D).starts_with("-----BEGIN PGP MESSAGE-----\n"));
    assert!(block_of(0xCB).starts_with("-----BEGIN PGP MESSAGE-----\n"));
}

#[test]
fn encoded_blocks_decode_back_to_their_data() {
    for b in 0..=255u8 {
        assert_eq!(decode_text(&encode(&[b])), Ok(vec![b]));
    }
    for len in 1..200usize {
        let data: Vec<u8> = (0..len).map(|i| (i * 7 + 3) as u8).collect();
        assert_eq!(decode_text(&encode(&data)), Ok(data));
    }
}

#[test]
fn a_block_missing_its_crc_line_still_decodes_its_payload() {
    let text = "-----BEGIN PGP MESSAGE-----\nVersion: x\n\nmQEN\n-----END PGP MESSAGE-----\n";
    assert_eq!(decode_text(text), Ok(vec![0x99, 1, 13]));
}

#[test]
fn a_block_of_crlf_headers_with_a_blank_crlf_line_decodes() {
    let text =
        "-----BEGIN PGP MESSAGE-----\r\nVersion: x\r\n\r\nmQEN\r\n-----END PGP MESSAGE-----\r\n";
    assert_eq!(decode_text(text), Ok(vec![0x99, 1, 13]));
}

#[test]
fn a_high_byte_after_line_breaks_is_invalid_armor() {
    assert_eq!(
        decode(&armored_body(b"\n\x80")),
        Err(ArmorError::InvalidArmor)
    );
    assert_eq!(
        decode(&armored_body(b"\n\n\x80")),
        Err(ArmorError::InvalidArmor)
    );
}

#[test]
fn a_high_byte_at_each_position_of_a_quad_is_invalid_armor() {
    assert_eq!(
        decode(&armored_body(b"A\x80BC")),
        Err(ArmorError::InvalidArmor)
    );
    assert_eq!(
        decode(&armored_body(b"AB\x80C")),
        Err(ArmorError::InvalidArmor)
    );
    assert_eq!(
        decode(&armored_body(b"ABC\x80")),
        Err(ArmorError::InvalidArmor)
    );
}

#[test]
fn a_high_byte_at_each_position_of_the_crc_is_invalid_armor() {
    let with_crc = |crc: &[u8]| {
        let mut text = b"-----BEGIN PGP MESSAGE-----\nVersion: x\n\nmQEN\n=".to_vec();
        text.extend_from_slice(crc);
        text.extend_from_slice(b"\n-----END PGP MESSAGE-----\n");
        text
    };
    assert_eq!(decode(&with_crc(b"A\x80AA")), Err(ArmorError::InvalidArmor));
    assert_eq!(decode(&with_crc(b"AA\x80A")), Err(ArmorError::InvalidArmor));
    assert_eq!(decode(&with_crc(b"AAA\x80")), Err(ArmorError::InvalidArmor));
}
