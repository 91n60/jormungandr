use super::WalletError as Error;
use crate::testing::network_builder::WalletAlias;
use assert_fs::fixture::{ChildPath, PathChild};
use bech32::FromBase32;
use bech32::ToBase32;
use chain_vote::{
    committee::ElectionPublicKey, MemberCommunicationKey, MemberCommunicationPublicKey,
    MemberPublicKey, MemberState, OpeningVoteKey, CRS,
};
use jormungandr_lib::crypto::account::Identifier;
use rand_core::{CryptoRng, RngCore};
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Write;

pub const COMMUNICATION_SK_HRP: &str = "p256k1_vcommsk";
pub const MEMBER_SK_HRP: &str = "p256k1_membersk";
pub const ENCRYPTING_VOTE_PK_HRP: &str = "p256k1_votepk";

#[derive(Clone)]
pub struct PrivateVoteCommitteeData {
    alias: String,
    communication_key: MemberCommunicationKey,
    member_secret_key: OpeningVoteKey,
    member_public_key: MemberPublicKey,
    election_public_key: ElectionPublicKey,
}

impl PrivateVoteCommitteeData {
    pub fn new(
        alias: String,
        communication_key: MemberCommunicationKey,
        member_secret_key: OpeningVoteKey,
        member_public_key: MemberPublicKey,
        election_public_key: ElectionPublicKey,
    ) -> Self {
        Self {
            alias,
            communication_key,
            member_secret_key,
            member_public_key,
            election_public_key,
        }
    }

    pub fn member_public_key(&self) -> MemberPublicKey {
        self.member_public_key.clone()
    }

    pub fn member_secret_key(&self) -> OpeningVoteKey {
        self.member_secret_key.clone()
    }

    pub fn encrypting_vote_key(&self) -> ElectionPublicKey {
        self.election_public_key.clone()
    }

    pub fn alias(&self) -> String {
        self.alias.clone()
    }

    pub fn write_to(&self, directory: ChildPath) {
        std::fs::create_dir_all(directory.path()).unwrap();
        self.write_communication_key(&directory);
        self.write_member_secret_key(&directory);
        self.write_encrypting_vote_key(&directory);
    }

    fn write_encrypting_vote_key(&self, directory: &ChildPath) {
        let path = directory.child("encrypting_vote_key.sk");
        let mut file = File::create(path.path()).unwrap();
        writeln!(file, "{}", self.encrypting_vote_key().to_base32().unwrap()).unwrap()
    }

    fn write_communication_key(&self, directory: &ChildPath) {
        let path = directory.child("communication_key.sk");
        let mut file = File::create(path.path()).unwrap();

        writeln!(
            file,
            "{}",
            bech32::encode(
                COMMUNICATION_SK_HRP,
                self.communication_key.to_bytes().to_base32()
            )
            .unwrap()
        )
        .unwrap()
    }

    fn write_member_secret_key(&self, directory: &ChildPath) {
        let path = directory.child("member_secret_key.sk");
        let mut file = File::create(path.path()).unwrap();
        writeln!(
            file,
            "{}",
            bech32::encode(
                MEMBER_SK_HRP,
                self.member_secret_key().to_bytes().to_base32()
            )
            .unwrap()
        )
        .unwrap()
    }
}

pub trait ElectionPublicKeyExtension {
    fn to_base32(&self) -> Result<String, bech32::Error>;
}

impl ElectionPublicKeyExtension for ElectionPublicKey {
    fn to_base32(&self) -> Result<String, bech32::Error> {
        bech32::encode(ENCRYPTING_VOTE_PK_HRP, self.to_bytes().to_base32())
    }
}

pub fn encrypting_key_from_base32(key: &str) -> Result<ElectionPublicKey, Error> {
    let (hrp, data) = bech32::decode(&key).map_err(Error::InvalidBech32)?;
    if hrp != ENCRYPTING_VOTE_PK_HRP {
        return Err(Error::InvalidBech32Key {
            expected: ENCRYPTING_VOTE_PK_HRP.to_string(),
            actual: hrp,
        });
    }
    let key_bin = Vec::<u8>::from_base32(&data)?;
    chain_vote::EncryptingVoteKey::from_bytes(&key_bin).ok_or(Error::VoteEncryptingKey)
}

impl fmt::Debug for PrivateVoteCommitteeData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("PrivateVoteCommitteeData")?;
        write!(
            f,
            "communication key: {:?}",
            self.communication_key.to_public().to_bytes()
        )?;
        write!(
            f,
            "member public key: {:?}",
            self.member_public_key.to_bytes()
        )?;
        f.write_str(")")
    }
}

#[derive(Clone, Debug)]
pub struct PrivateVoteCommitteeDataManager {
    data: HashMap<Identifier, PrivateVoteCommitteeData>,
}

impl PrivateVoteCommitteeDataManager {
    pub fn new<RNG>(
        mut rng: &mut RNG,
        committees: Vec<(WalletAlias, Identifier)>,
        threshold: usize,
    ) -> Self
    where
        RNG: RngCore + CryptoRng,
    {
        let crs = CRS::random(rng);
        let mut data = HashMap::new();

        let communication_secret_keys: Vec<MemberCommunicationKey> =
            std::iter::from_fn(|| Some(MemberCommunicationKey::new(&mut rng)))
                .take(committees.len())
                .collect();
        let communication_public_keys: Vec<MemberCommunicationPublicKey> =
            communication_secret_keys
                .iter()
                .map(|x| x.to_public())
                .collect();

        for (index, (alias, pk)) in committees.iter().enumerate() {
            let ms = MemberState::new(&mut rng, threshold, &crs, &communication_public_keys, index);

            let communication_secret_key = communication_secret_keys.get(index).unwrap();
            let encrypting_vote_key =
                ElectionPublicKey::from_participants(&[ms.public_key().clone()]);

            data.insert(
                pk.clone(),
                PrivateVoteCommitteeData::new(
                    alias.clone(),
                    communication_secret_key.clone(),
                    ms.secret_key().clone(),
                    ms.public_key().clone(),
                    encrypting_vote_key,
                ),
            );
        }

        Self { data }
    }

    pub fn get(&self, identifier: &Identifier) -> Option<&PrivateVoteCommitteeData> {
        self.data.get(identifier)
    }

    pub fn write_to(&self, directory: ChildPath) -> std::io::Result<()> {
        for (id, data) in self.data.iter() {
            let item_directory = directory.child(id.to_bech32_str());
            data.write_to(item_directory);
        }
        Ok(())
    }

    pub fn member_public_keys(&self) -> Vec<MemberPublicKey> {
        self.data.values().map(|x| x.member_public_key()).collect()
    }
}
