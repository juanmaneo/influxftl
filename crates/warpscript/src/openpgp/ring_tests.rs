// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Tests of the key rings: the layout of public and secret rings, the packets they drop or skip,
//! the expiry that their signatures give, and the failures of their packets.

use crate::error::pgp::PgpError;
use crate::openpgp::key::PublicKey;
use crate::openpgp::packet::write_packet;
use crate::openpgp::ring::{Key, Ring, read_rings};

fn mpi(bits: u16, bytes: &[u8]) -> Vec<u8> {
    let mut out = bits.to_be_bytes().to_vec();
    out.extend_from_slice(bytes);
    out
}

/// A version 4 key packet body created at 1000 seconds, with the given algorithm and an RSA
/// modulus and exponent. Each algorithm gives a key with its own ID.
fn key_body(algorithm: u8) -> Vec<u8> {
    let mut body = vec![4];
    body.extend_from_slice(&1_000u32.to_be_bytes());
    body.push(algorithm);
    body.extend_from_slice(&[mpi(16, &[0xAB, 0xCD]), mpi(2, &[0x03])].concat());
    body
}

/// A version 3 key packet body created at 1000 seconds, valid for `days`.
fn key_v3_body(days: u16) -> Vec<u8> {
    let mut body = vec![3];
    body.extend_from_slice(&1_000u32.to_be_bytes());
    body.extend_from_slice(&days.to_be_bytes());
    body.push(1);
    body.extend_from_slice(&[mpi(16, &[0xAB, 0xCD]), mpi(2, &[0x03])].concat());
    body
}

/// The key ID of a key packet body.
fn key_id(body: &[u8]) -> u64 {
    PublicKey::parse(body).expect("parses").0.key_id
}

/// A subpacket of one octet of length, with its tag.
fn subpacket(tag: u8, data: &[u8]) -> Vec<u8> {
    let mut out = vec![(data.len() + 1) as u8, tag];
    out.extend_from_slice(data);
    out
}

/// A version 4 signature of `sig_type`, with its hashed and unhashed subpackets and no MPI.
fn sig(sig_type: u8, hashed: &[Vec<u8>], unhashed: &[Vec<u8>]) -> Vec<u8> {
    let hashed = hashed.concat();
    let unhashed = unhashed.concat();
    let mut body = vec![4, sig_type, 1, 8];
    body.extend_from_slice(&(hashed.len() as u16).to_be_bytes());
    body.extend_from_slice(&hashed);
    body.extend_from_slice(&(unhashed.len() as u16).to_be_bytes());
    body.extend_from_slice(&unhashed);
    body.extend_from_slice(&[0xAB, 0xCD]);
    body
}

/// A signature by `issuer` with a key expiry of `expiry` seconds, and no creation time.
fn sig_with_expiry(sig_type: u8, issuer: u64, expiry: u32) -> Vec<u8> {
    sig(
        sig_type,
        &[
            subpacket(16, &issuer.to_be_bytes()),
            subpacket(9, &expiry.to_be_bytes()),
        ],
        &[],
    )
}

/// A signature by `issuer`, created at `created` seconds, with a key expiry of `expiry`.
fn dated_sig(sig_type: u8, issuer: u64, created: u32, expiry: u32) -> Vec<u8> {
    sig(
        sig_type,
        &[
            subpacket(2, &created.to_be_bytes()),
            subpacket(16, &issuer.to_be_bytes()),
            subpacket(9, &expiry.to_be_bytes()),
        ],
        &[],
    )
}

/// The packets of a list of (tag, body) pairs, as they are written.
fn packets(list: &[(u8, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (tag, body) in list {
        write_packet(&mut out, *tag, body);
    }
    out
}

fn read(list: &[(u8, Vec<u8>)]) -> Result<Vec<Ring>, PgpError> {
    read_rings(&packets(list))
}

/// The packets of `list`, followed by the raw `extra` bytes.
fn read_with_raw(list: &[(u8, Vec<u8>)], extra: &[u8]) -> Result<Vec<Ring>, PgpError> {
    let mut input = packets(list);
    input.extend_from_slice(extra);
    read_rings(&input)
}

/// The primary key of a public ring, signed by `sigs`.
fn primary_with(sigs: &[Vec<u8>]) -> Key {
    let mut list = vec![(6, key_body(1))];
    list.extend(sigs.iter().map(|s| (2, s.clone())));
    read(&list).expect("reads").remove(0).keys.remove(0)
}

/// The subkey of a public ring, a version 4 RSA_ENCRYPT key, signed by `subsigs`.
fn subkey_with(subsigs: &[Vec<u8>]) -> Key {
    let mut list = vec![(6, key_body(1)), (14, key_body(2))];
    list.extend(subsigs.iter().map(|s| (2, s.clone())));
    read(&list).expect("reads").remove(0).keys.remove(1)
}

#[test]
fn public_ring_with_user_ids_trust_and_subkeys() {
    let primary = key_body(1);
    let pid = key_id(&primary);
    let self_sig = sig(
        0x13,
        &[
            subpacket(2, &2_000u32.to_be_bytes()),
            subpacket(16, &pid.to_be_bytes()),
            subpacket(9, &3_600u32.to_be_bytes()),
        ],
        &[],
    );
    let user_sig = sig(0x13, &[subpacket(16, &pid.to_be_bytes())], &[]);
    let subkey = key_body(2);
    let binding = sig(0x18, &[subpacket(16, &pid.to_be_bytes())], &[]);
    let list = vec![
        (6, primary.clone()),
        (2, self_sig.clone()),
        (13, b"alice".to_vec()),
        (2, user_sig.clone()),
        (12, vec![0]),
        (14, subkey.clone()),
        (2, binding.clone()),
        (6, key_body(2)),
    ];
    let rings = read(&list).expect("reads");
    assert_eq!(rings.len(), 2);

    let ring = &rings[0];
    assert!(!ring.secret);
    assert!(ring.extras.is_empty());
    assert_eq!(ring.public_keys().count(), 2);
    let master = &ring.keys[0];
    assert_eq!(master.user_ids, vec![b"alice".to_vec()]);
    assert_eq!(master.signatures.len(), 2);
    assert_eq!(master.packets.len(), 5);
    assert!(master.is_master());
    assert_eq!(master.valid_seconds(), 3_600);
    let child = &ring.keys[1];
    assert!(child.subkey);
    assert!(!child.is_master());
    assert_eq!(child.signatures.len(), 1);

    let expected = packets(&[
        (6, primary),
        (2, self_sig),
        (13, b"alice".to_vec()),
        (2, user_sig),
        (12, vec![0]),
        (14, subkey),
        (2, binding),
    ]);
    assert_eq!(ring.encode(), expected);

    assert!(!rings[1].keys[0].is_master());
}

#[test]
fn version_3_key_validity_in_days() {
    let ring = read(&[(6, key_v3_body(2))]).expect("reads").remove(0);
    let key = &ring.keys[0];
    assert_eq!(key.public.version, 3);
    assert_eq!(key.valid_seconds(), 2 * 86_400);
    assert_eq!(key.expiry(), (1_000_000 + 172_800_000) * 1000);
}

#[test]
fn master_expiry_from_its_own_signatures() {
    let pid = key_id(&key_body(1));
    let key = primary_with(&[sig_with_expiry(0x13, pid, 3_600)]);
    assert_eq!(key.valid_seconds(), 3_600);
    assert_eq!(key.expiry(), (1_000_000 + 3_600_000) * 1000);
}

#[test]
fn master_without_expiry_never_expires() {
    let key = primary_with(&[]);
    assert_eq!(key.valid_seconds(), 0);
    assert_eq!(key.expiry(), 0);
}

#[test]
fn master_skips_other_issuers_and_unexpiring_signatures() {
    let pid = key_id(&key_body(1));
    let key = primary_with(&[
        sig_with_expiry(0x13, pid.wrapping_add(1), 99),
        sig(0x13, &[subpacket(16, &pid.to_be_bytes())], &[]),
    ]);
    assert_eq!(key.valid_seconds(), 0);
}

#[test]
fn master_keeps_the_newest_self_signature() {
    let pid = key_id(&key_body(1));
    let key = primary_with(&[
        dated_sig(0x13, pid, 1_000, 50),
        dated_sig(0x13, pid, 2_000, 100),
    ]);
    assert_eq!(key.valid_seconds(), 100);
    let key = primary_with(&[
        dated_sig(0x13, pid, 2_000, 100),
        dated_sig(0x13, pid, 1_000, 50),
    ]);
    assert_eq!(key.valid_seconds(), 100);
}

#[test]
fn subkey_expiry_keeps_the_longest_other_issuer_expiry() {
    let pid = key_id(&key_body(1));
    let key = subkey_with(&[
        sig_with_expiry(0x18, pid, 7_200),
        sig_with_expiry(0x18, pid, 100),
    ]);
    assert!(key.subkey);
    assert_eq!(key.valid_seconds(), 7_200);
}

#[test]
fn subkey_zero_expiry_means_never() {
    let pid = key_id(&key_body(1));
    let key = subkey_with(&[sig_with_expiry(0x18, pid, 0)]);
    assert_eq!(key.valid_seconds(), 0);
    assert_eq!(key.expiry(), 0);
}

#[test]
fn subkey_falls_back_to_the_direct_key_signature() {
    let pid = key_id(&key_body(1));
    let key = subkey_with(&[sig_with_expiry(0x1F, pid, 42)]);
    assert_eq!(key.valid_seconds(), 42);
}

#[test]
fn subkey_keeps_the_newest_self_signature() {
    let subkey_body = key_body(2);
    let sid = key_id(&subkey_body);
    let list = vec![
        (6, key_body(1)),
        (14, subkey_body),
        (2, dated_sig(0x18, sid, 3_000, 500)),
        (2, dated_sig(0x18, sid, 2_000, 900)),
    ];
    let ring = read(&list).expect("reads").remove(0);
    assert_eq!(ring.keys[1].valid_seconds(), 500);
}

#[test]
fn secret_ring_with_secret_and_public_subkeys() {
    let mut secret = key_body(1);
    secret.extend_from_slice(&[0, 1, 2, 3]);
    let pid = key_id(&key_body(1));
    let mut secret_subkey = key_body(2);
    secret_subkey.extend_from_slice(&[0, 9]);
    let list = vec![
        (5, secret),
        (61, vec![9, 9]),
        (2, sig_with_expiry(0x13, pid, 3_600)),
        (13, b"alice".to_vec()),
        (17, vec![1, 2]),
        (7, secret_subkey),
        (61, vec![1]),
        (2, sig(0x18, &[subpacket(16, &pid.to_be_bytes())], &[])),
        (14, key_body(2)),
        (2, sig(0x18, &[subpacket(16, &pid.to_be_bytes())], &[])),
        (13, b"stray".to_vec()),
    ];
    let rings = read(&list).expect("reads");
    assert_eq!(rings.len(), 1);
    let ring = &rings[0];
    assert!(ring.secret);
    assert_eq!(ring.keys.len(), 2);
    assert_eq!(ring.extras.len(), 1);
    assert_eq!(ring.public_keys().count(), 3);
    assert!(ring.keys[0].secret && !ring.keys[0].subkey);
    assert!(ring.keys[1].secret && ring.keys[1].subkey);
    assert!(!ring.extras[0].secret && ring.extras[0].subkey);
    assert_eq!(ring.keys[0].user_ids, vec![b"alice".to_vec()]);
    assert_eq!(ring.keys[0].attributes, vec![vec![1, 2]]);
    assert_eq!(ring.keys[0].valid_seconds(), 3_600);
    assert_ne!(ring.encode(), Vec::<u8>::new());
}

#[test]
fn secret_ring_can_end_after_its_primary_key() {
    let ring = read(&[(5, key_body(1))]).expect("reads").remove(0);
    assert!(ring.secret);
    assert_eq!(ring.keys.len(), 1);
    assert!(ring.extras.is_empty());
}

#[test]
fn signatures_of_unread_versions_are_dropped_with_their_trust() {
    let list = vec![(6, key_body(1)), (2, vec![5, 0]), (12, vec![0])];
    let ring = read(&list).expect("reads").remove(0);
    assert_eq!(ring.keys[0].signatures.len(), 0);
    assert_eq!(ring.keys[0].packets.len(), 1);
}

#[test]
fn version_3_signature_is_read_with_its_trust() {
    let mut v3 = vec![3, 5, 0x13];
    v3.extend_from_slice(&1_000u32.to_be_bytes());
    v3.extend_from_slice(&7u64.to_be_bytes());
    v3.extend_from_slice(&[1, 8, 0xAB, 0xCD]);
    let list = vec![(6, key_body(1)), (2, v3), (12, vec![0])];
    let ring = read(&list).expect("reads").remove(0);
    assert_eq!(ring.keys[0].signatures.len(), 1);
    assert_eq!(ring.keys[0].packets.len(), 3);
}

#[test]
fn trust_before_a_signature_is_kept() {
    let list = vec![(6, key_body(1)), (12, vec![0]), (2, sig(0x13, &[], &[]))];
    let ring = read(&list).expect("reads").remove(0);
    assert_eq!(ring.keys[0].packets.len(), 3);
    assert_eq!(ring.keys[0].signatures.len(), 1);
}

#[test]
fn other_packets_are_skipped_and_empty_input_has_no_ring() {
    assert_eq!(
        read(&[(13, b"alice".to_vec()), (2, vec![4])]).map(|rings| rings.len()),
        Ok(0)
    );
    assert_eq!(read_rings(&[]).map(|rings| rings.len()), Ok(0));
}

#[test]
fn key_packet_failures_are_reported() {
    let bad = vec![4, 0, 0, 0, 0, 99];
    let unknown = PgpError::UnknownAlgorithm(99);
    assert_eq!(read(&[(6, bad.clone())]).map(|_| ()), Err(unknown.clone()));
    assert_eq!(read(&[(5, bad.clone())]).map(|_| ()), Err(unknown.clone()));
    assert_eq!(
        read(&[(6, key_body(1)), (14, bad.clone())]).map(|_| ()),
        Err(unknown.clone())
    );
    assert_eq!(
        read(&[(5, key_body(1)), (7, bad.clone())]).map(|_| ()),
        Err(unknown.clone())
    );
    assert_eq!(
        read(&[(5, key_body(1)), (14, bad)]).map(|_| ()),
        Err(unknown)
    );
}

#[test]
fn packet_stream_failures_are_reported() {
    let key = key_body(1);
    assert_eq!(
        read_with_raw(&[], &[0x06]).map(|_| ()),
        Err(PgpError::InvalidHeader)
    );
    assert_eq!(
        read_with_raw(&[(6, key.clone())], &[0x06]).map(|_| ()),
        Err(PgpError::InvalidHeader)
    );
    assert_eq!(
        read_with_raw(&[(5, key.clone())], &[0x06]).map(|_| ()),
        Err(PgpError::InvalidHeader)
    );
    assert_eq!(
        read_with_raw(&[(5, key.clone()), (2, sig(0x13, &[], &[]))], &[0x06]).map(|_| ()),
        Err(PgpError::InvalidHeader)
    );
    assert_eq!(
        read_with_raw(&[(5, key.clone()), (7, key)], &[0x06]).map(|_| ()),
        Err(PgpError::InvalidHeader)
    );
    assert_eq!(
        read_with_raw(&[], &[0x98, 10, 1]).map(|_| ()),
        Err(PgpError::PrematureEnd)
    );
    assert_eq!(
        read_with_raw(&[], &[0xCD, 5, 1]).map(|_| ()),
        Err(PgpError::PrematureEnd)
    );
}

#[test]
fn unreadable_signature_content_is_reported() {
    let list = vec![(6, key_body(1)), (2, vec![4, 0x13])];
    assert_eq!(read(&list).map(|_| ()), Err(PgpError::PrematureEnd));
}

/// A packet of `tag` whose length announces ten bytes, of which only one is there.
#[test]
fn truncated_trust_after_a_signature_fails() {
    let input = [
        packets(&[(6, key_body(1)), (2, sig(0x1F, &[], &[]))]),
        truncated(12),
    ]
    .concat();
    assert_eq!(read_rings(&input).map(|_| ()), Err(PgpError::PrematureEnd));
}

fn truncated(tag: u8) -> Vec<u8> {
    if tag <= 15 {
        vec![0x80 | (tag << 2), 10, 1]
    } else {
        vec![0xC0 | tag, 10, 1]
    }
}

#[test]
fn truncated_packets_fail_where_they_are_read() {
    let key = key_body(1);
    let subkey = key_body(2);
    let uid = vec![(6, key.clone()), (13, b"a".to_vec())];
    let cases: Vec<Vec<u8>> = vec![
        truncated(6),
        [packets(&[(6, key.clone())]), truncated(12)].concat(),
        [packets(&[(6, key.clone())]), truncated(2)].concat(),
        [packets(&uid), truncated(12)].concat(),
        [packets(&uid), truncated(2)].concat(),
        [packets(&[(6, key.clone())]), truncated(13)].concat(),
        [packets(&[(6, key.clone())]), truncated(17)].concat(),
        [packets(&[(6, key.clone())]), truncated(14)].concat(),
        [
            packets(&[(6, key.clone()), (14, subkey.clone())]),
            truncated(12),
        ]
        .concat(),
        truncated(5),
        [packets(&[(5, key.clone())]), truncated(61)].concat(),
        [packets(&[(5, key.clone())]), truncated(12)].concat(),
        [packets(&[(5, key.clone())]), truncated(2)].concat(),
        [packets(&[(5, key.clone())]), truncated(13)].concat(),
        [
            packets(&[(5, key.clone()), (7, subkey.clone())]),
            truncated(61),
        ]
        .concat(),
        [
            packets(&[(5, key.clone()), (7, subkey.clone())]),
            truncated(12),
        ]
        .concat(),
        [packets(&[(5, key.clone())]), truncated(7)].concat(),
        [packets(&[(5, key.clone())]), truncated(14)].concat(),
        [
            packets(&[(5, key.clone()), (14, subkey.clone())]),
            truncated(12),
        ]
        .concat(),
    ];
    for input in cases {
        assert_eq!(
            read_rings(&input).map(|_| ()),
            Err(PgpError::PrematureEnd),
            "{input:?}"
        );
    }
}
