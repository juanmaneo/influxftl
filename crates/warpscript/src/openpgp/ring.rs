// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The key rings of OpenPGP: the public and secret rings read from a stream of packets, as
//! BouncyCastle's `PGPPublicKeyRing`, `PGPSecretKeyRing` and `PGPObjectFactory` read them.
//!
//! A key keeps the packets it was read from, so that it is written back as it was read.

use crate::error::pgp::PgpError;
use crate::openpgp::key::PublicKey;
use crate::openpgp::packet::{Packet, PacketReader, write_packet};
use crate::openpgp::signature::Signature;

/// The tag of a public key packet, which starts a public key ring.
const PUBLIC_KEY: u8 = 6;
/// The tag of a secret key packet, which starts a secret key ring.
const SECRET_KEY: u8 = 5;
/// The tag of a secret subkey packet.
const SECRET_SUBKEY: u8 = 7;
/// The tag of a public subkey packet.
const PUBLIC_SUBKEY: u8 = 14;
/// The tag of a trust packet, which follows a key, a signature or a user ID.
const TRUST: u8 = 12;
/// The tag of a user ID packet.
const USER_ID: u8 = 13;
/// The tag of a user attribute packet (an image, in practice).
const USER_ATTRIBUTE: u8 = 17;
/// The tag of a signature packet.
const SIGNATURE: u8 = 2;
/// The tag of the packets that BouncyCastle drops from a secret key (EXPERIMENTAL_2).
const DROPPED_AFTER_SECRET: u8 = 61;

/// A key of a ring, with what BouncyCastle attaches to it: its packets, its user IDs and its
/// signatures.
#[derive(Debug, Clone)]
pub struct Key {
    /// The public part of the key packet.
    pub public: PublicKey,
    /// Whether the key is a secret key (a secret key packet of a secret ring).
    pub secret: bool,
    /// Whether the key is a subkey (its packet has tag 7 or 14).
    pub subkey: bool,
    /// The packets of the key, in the order they were read.
    pub packets: Vec<Packet>,
    /// The user IDs of the key (the bodies of the user ID packets).
    pub user_ids: Vec<Vec<u8>>,
    /// The user attributes of the key (the bodies of the user attribute packets).
    pub attributes: Vec<Vec<u8>>,
    /// The signatures of the key, in the order they were read.
    pub signatures: Vec<Signature>,
}

/// A public or a secret key ring: a primary key and its subkeys.
#[derive(Debug, Clone)]
pub struct Ring {
    /// Whether the ring is a secret key ring.
    pub secret: bool,
    /// The keys of the ring: the primary key first, then the subkeys (for a secret ring, its
    /// secret keys).
    pub keys: Vec<Key>,
    /// The public subkeys of a secret ring, which are kept apart from its secret keys.
    pub extras: Vec<Key>,
}

impl Key {
    /// A key from its key packet.
    fn new(packet: Packet, secret: bool) -> Result<Self, PgpError> {
        let (public, _) = PublicKey::parse(&packet.body)?;
        let subkey = matches!(packet.tag, SECRET_SUBKEY | PUBLIC_SUBKEY);
        Ok(Self {
            public,
            secret,
            subkey,
            packets: vec![packet],
            user_ids: Vec::new(),
            attributes: Vec::new(),
            signatures: Vec::new(),
        })
    }

    /// Whether the key is a master key (`PGPPublicKey.isMasterKey`): a primary key that does
    /// not only encrypt, or whose algorithm is RSA_GENERAL.
    pub fn is_master(&self) -> bool {
        !self.subkey && (!self.public.is_encryption() || self.public.algorithm == 1)
    }

    /// The packets of the key, written as they are (`getEncoded(false)`).
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for packet in &self.packets {
            write_packet(&mut out, packet.tag, &packet.body);
        }
        out
    }

    /// The validity of the key in seconds, or -1 when no signature says (`getValidSeconds`).
    pub fn valid_seconds(&self) -> i64 {
        if self.public.version <= 3 {
            return self.public.valid_seconds_v3();
        }
        let (types, is_master): (&[u8], bool) = if self.is_master() {
            (&[0x13, 0x12, 0x11, 0x10, 0x1F], true)
        } else {
            (&[0x18, 0x1F], false)
        };
        types
            .iter()
            .map(|&sig_type| self.expiration_from_signatures(is_master, sig_type))
            .find(|&seconds| seconds >= 0)
            .unwrap_or(0)
    }

    /// The expiry of the key, as `PGPINFO` reports it: 0 when the key never expires.
    pub fn expiry(&self) -> i64 {
        match self.valid_seconds() {
            0 => 0,
            seconds => (self.public.creation_ms() + seconds * 1000) * 1000,
        }
    }

    /// The expiry that the signatures of one type give, or -1 when none has an expiry
    /// (`getExpirationTimeFromSig`).
    fn expiration_from_signatures(&self, is_master: bool, sig_type: u8) -> i64 {
        let mut last_expiry = -1;
        let mut last_creation = -1;
        for sig in self.signatures.iter().filter(|s| s.sig_type == sig_type) {
            if is_master && sig.key_id != self.public.key_id {
                continue;
            }
            let Some(expiration) = sig.key_expiration() else {
                continue;
            };
            let expiration = i64::from(expiration);
            if sig.key_id == self.public.key_id {
                if sig.creation_ms > last_creation {
                    last_creation = sig.creation_ms;
                    last_expiry = expiration;
                }
            } else if expiration == 0 {
                last_expiry = 0;
            } else if expiration > last_expiry {
                last_expiry = expiration;
            }
        }
        last_expiry
    }
}

impl Ring {
    /// The public keys of the ring: its keys, then its extra public subkeys
    /// (`getPublicKeys`).
    pub fn public_keys(&self) -> impl Iterator<Item = &Key> {
        self.keys.iter().chain(self.extras.iter())
    }

    /// The ring as its packets are written (`getEncoded(false)`).
    pub fn encode(&self) -> Vec<u8> {
        self.keys
            .iter()
            .chain(self.extras.iter())
            .flat_map(Key::encode)
            .collect()
    }
}

/// Reads the rings of a stream of packets, keeping only the rings (`PGPObjectFactory` read in a
/// loop, as `PGPRING` does). The other packets are read and dropped.
pub fn read_rings(input: &[u8]) -> Result<Vec<Ring>, PgpError> {
    let mut reader = PacketReader::new(input);
    let mut rings = Vec::new();
    while let Some(tag) = reader.next_tag()? {
        match tag {
            PUBLIC_KEY => rings.push(read_public_ring(&mut reader)?),
            SECRET_KEY => rings.push(read_secret_ring(&mut reader)?),
            _ => {
                reader.read()?;
            }
        }
    }
    Ok(rings)
}

/// A public key ring, from its primary key packet to the first packet that is not a subkey.
fn read_public_ring(reader: &mut PacketReader<'_>) -> Result<Ring, PgpError> {
    let mut primary = Key::new(reader.read()?, false)?;
    read_certification(reader, &mut primary)?;
    let mut keys = vec![primary];
    while reader.peek_tag() == Some(PUBLIC_SUBKEY) {
        let mut subkey = Key::new(reader.read()?, false)?;
        read_subkey_certification(reader, &mut subkey)?;
        keys.push(subkey);
    }
    Ok(Ring {
        secret: false,
        keys,
        extras: Vec::new(),
    })
}

/// A secret key ring: its secret key, its secret subkeys, then the public subkeys apart.
fn read_secret_ring(reader: &mut PacketReader<'_>) -> Result<Ring, PgpError> {
    let mut primary = Key::new(reader.read()?, true)?;
    skip_dropped(reader)?;
    read_certification(reader, &mut primary)?;
    let mut keys = vec![primary];
    let mut extras = Vec::new();
    loop {
        match reader.peek_tag() {
            Some(SECRET_SUBKEY) => {
                let mut subkey = Key::new(reader.read()?, true)?;
                skip_dropped(reader)?;
                read_subkey_certification(reader, &mut subkey)?;
                keys.push(subkey);
            }
            Some(PUBLIC_SUBKEY) => {
                let mut subkey = Key::new(reader.read()?, false)?;
                read_subkey_certification(reader, &mut subkey)?;
                extras.push(subkey);
            }
            _ => break,
        }
    }
    Ok(Ring {
        secret: true,
        keys,
        extras,
    })
}

/// The packets that BouncyCastle reads and drops after a secret key.
fn skip_dropped(reader: &mut PacketReader<'_>) -> Result<(), PgpError> {
    while reader.peek_tag() == Some(DROPPED_AFTER_SECRET) {
        reader.read()?;
    }
    Ok(())
}

/// The trust, the signatures, then the user IDs and attributes of a primary key, each with its
/// trust and signatures.
fn read_certification(reader: &mut PacketReader<'_>, key: &mut Key) -> Result<(), PgpError> {
    read_optional_trust(reader, key)?;
    read_signatures(reader, key)?;
    loop {
        match reader.peek_tag() {
            Some(USER_ID) => {
                let packet = reader.read()?;
                key.user_ids.push(packet.body.clone());
                key.packets.push(packet);
            }
            Some(USER_ATTRIBUTE) => {
                let packet = reader.read()?;
                key.attributes.push(packet.body.clone());
                key.packets.push(packet);
            }
            _ => return Ok(()),
        }
        read_optional_trust(reader, key)?;
        read_signatures(reader, key)?;
    }
}

/// The trust and the signatures of a subkey.
fn read_subkey_certification(reader: &mut PacketReader<'_>, key: &mut Key) -> Result<(), PgpError> {
    read_optional_trust(reader, key)?;
    read_signatures(reader, key)
}

/// A trust packet, if the next packet is one.
fn read_optional_trust(reader: &mut PacketReader<'_>, key: &mut Key) -> Result<(), PgpError> {
    if reader.peek_tag() == Some(TRUST) {
        key.packets.push(reader.read()?);
    }
    Ok(())
}

/// The signatures that follow, each with the trust that may come after it. A signature of a
/// version this port does not read is dropped with its trust left unread, as BouncyCastle does.
fn read_signatures(reader: &mut PacketReader<'_>, key: &mut Key) -> Result<(), PgpError> {
    while reader.peek_tag() == Some(SIGNATURE) {
        let packet = reader.read()?;
        if let Some(signature) = Signature::parse(&packet.body)? {
            key.signatures.push(signature);
            key.packets.push(packet);
            read_optional_trust(reader, key)?;
        }
    }
    Ok(())
}
