//! SEP-10 for non-custodial G-address wallets. Challenges are never submitted on-chain.
use crate::xdr::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signer as _, SigningKey, VerifyingKey};
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

#[derive(Debug, thiserror::Error)]
pub enum ChallengeError {
    #[error("invalid SEP-10 configuration")]
    Configuration,
    #[error("account must be a valid Stellar G-address distinct from the server")]
    Account,
    #[error("malformed challenge transaction")]
    Malformed,
    #[error("challenge has expired or is not yet valid")]
    TimeBounds,
    #[error("challenge source, domain, or operations do not match this server")]
    Contents,
    #[error("challenge signatures do not authorize this account")]
    Signature,
}

/// Contains the server's signing key. Intentionally does not implement Debug.
pub struct Sep10 {
    key: SigningKey,
    passphrase: String,
    home_domain: String,
    web_auth_domain: String,
    ttl: u64,
}

pub struct Challenge {
    pub transaction: String,
    pub hash: [u8; 32],
    pub expires_at: u64,
}

pub struct ParsedChallenge {
    pub account: String,
    pub hash: [u8; 32],
    pub expires_at: u64,
    envelope: TransactionV1Envelope,
    server_signature: usize,
}

impl Sep10 {
    pub fn from_settings(settings: &config::Settings) -> Result<Self, ChallengeError> {
        let endpoint = url::Url::parse(&settings.sep10_web_auth_endpoint)
            .map_err(|_| ChallengeError::Configuration)?;
        if endpoint.host_str() != Some(settings.sep10_web_auth_domain.as_str())
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || !(endpoint.scheme() == "https"
                || (endpoint.scheme() == "http"
                    && matches!(endpoint.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))))
        {
            return Err(ChallengeError::Configuration);
        }
        Self::new(
            &settings.sep10_signing_seed,
            &settings.stellar_network_passphrase,
            &settings.sep10_home_domain,
            &settings.sep10_web_auth_domain,
            settings.auth_challenge_ttl_secs,
        )
    }

    pub fn new(
        seed: &str,
        passphrase: &str,
        home_domain: &str,
        web_auth_domain: &str,
        ttl: u64,
    ) -> Result<Self, ChallengeError> {
        let seed = stellar_strkey::ed25519::PrivateKey::from_string(seed)
            .map_err(|_| ChallengeError::Configuration)?;
        for (domain, max) in [(home_domain, 59), (web_auth_domain, 64)] {
            if domain.is_empty()
                || domain.len() > max
                || !domain.is_ascii()
                || domain.bytes().any(|c| !c.is_ascii_alphanumeric() && c != b'.' && c != b'-')
            {
                return Err(ChallengeError::Configuration);
            }
        }
        if passphrase.is_empty() || !(1..=900).contains(&ttl) {
            return Err(ChallengeError::Configuration);
        }
        Ok(Self {
            key: SigningKey::from_bytes(&seed.0),
            passphrase: passphrase.to_owned(),
            home_domain: home_domain.to_owned(),
            web_auth_domain: web_auth_domain.to_owned(),
            ttl,
        })
    }

    pub fn server_account(&self) -> String {
        format!("{}", stellar_strkey::ed25519::PublicKey(self.key.verifying_key().to_bytes()))
    }

    pub fn build(&self, account: &str, now: u64) -> Result<Challenge, ChallengeError> {
        let client = stellar_strkey::ed25519::PublicKey::from_string(account)
            .map_err(|_| ChallengeError::Account)?;
        if account == self.server_account() {
            return Err(ChallengeError::Account);
        }
        let server = self.key.verifying_key().to_bytes();
        let mut nonce = [0u8; 48];
        OsRng.fill_bytes(&mut nonce);
        let expires_at = now.checked_add(self.ttl).ok_or(ChallengeError::TimeBounds)?;
        let tx = Transaction {
            source_account: muxed(server),
            fee: 200,
            seq_num: SequenceNumber(0),
            cond: Preconditions::Time(TimeBounds {
                min_time: TimePoint(now),
                max_time: TimePoint(expires_at),
            }),
            memo: Memo::None,
            ext: TransactionExt::V0,
            operations: vec![
                manage_data(
                    client.0,
                    &format!("{} auth", self.home_domain),
                    STANDARD.encode(nonce).as_bytes(),
                )?,
                manage_data(server, "web_auth_domain", self.web_auth_domain.as_bytes())?,
            ]
            .try_into()
            .map_err(|_| ChallengeError::Malformed)?,
        };
        let hash = transaction_hash(&tx, &self.passphrase)?;
        let envelope = TransactionEnvelope::Tx(TransactionV1Envelope {
            tx,
            signatures: vec![sign(&self.key, &hash)]
                .try_into()
                .map_err(|_| ChallengeError::Malformed)?,
        });
        Ok(Challenge {
            transaction: envelope.to_xdr_base64(limits()).map_err(|_| ChallengeError::Malformed)?,
            hash,
            expires_at,
        })
    }

    /// Validate the envelope before any network or database access.
    pub fn parse(&self, encoded: &str, now: u64) -> Result<ParsedChallenge, ChallengeError> {
        if encoded.len() > 16_384 {
            return Err(ChallengeError::Malformed);
        }
        let TransactionEnvelope::Tx(envelope) =
            TransactionEnvelope::from_xdr_base64(encoded, limits())
                .map_err(|_| ChallengeError::Malformed)?
        else {
            return Err(ChallengeError::Malformed);
        };
        let tx = &envelope.tx;
        if tx.source_account != muxed(self.key.verifying_key().to_bytes())
            || tx.seq_num.0 != 0
            || tx.memo != Memo::None
            || tx.ext != TransactionExt::V0
            || tx.operations.len() != 2
        {
            return Err(ChallengeError::Contents);
        }
        let Preconditions::Time(bounds) = &tx.cond else {
            return Err(ChallengeError::TimeBounds);
        };
        if bounds.max_time.0 <= bounds.min_time.0
            || bounds.max_time.0 - bounds.min_time.0 > self.ttl
            || now < bounds.min_time.0
            || now >= bounds.max_time.0
        {
            return Err(ChallengeError::TimeBounds);
        }
        let first = &tx.operations[0];
        let Some(MuxedAccount::Ed25519(client)) = &first.source_account else {
            return Err(ChallengeError::Account);
        };
        if client.0 == self.key.verifying_key().to_bytes() {
            return Err(ChallengeError::Account);
        }
        let OperationBody::ManageData(data) = &first.body else {
            return Err(ChallengeError::Contents);
        };
        let Some(value) = &data.data_value else {
            return Err(ChallengeError::Contents);
        };
        if data.data_name.as_slice() != format!("{} auth", self.home_domain).as_bytes()
            || value.0.len() != 64
            || STANDARD.decode(value.0.as_slice()).map_or(true, |n| n.len() != 48)
            || tx.operations[1]
                != manage_data(
                    self.key.verifying_key().to_bytes(),
                    "web_auth_domain",
                    self.web_auth_domain.as_bytes(),
                )?
        {
            return Err(ChallengeError::Contents);
        }
        let hash = transaction_hash(tx, &self.passphrase)?;
        let server_signatures: Vec<_> = envelope
            .signatures
            .iter()
            .enumerate()
            .filter(|(_, signature)| {
                verifies(&self.key.verifying_key().to_bytes(), &hash, signature)
            })
            .map(|(i, _)| i)
            .collect();
        if server_signatures.len() != 1 {
            return Err(ChallengeError::Signature);
        }
        Ok(ParsedChallenge {
            account: format!("{}", stellar_strkey::ed25519::PublicKey(client.0)),
            hash,
            expires_at: bounds.max_time.0,
            server_signature: server_signatures[0],
            envelope,
        })
    }
}

impl ParsedChallenge {
    /// SEP-10 medium threshold for existing accounts; master signature only for
    /// unfunded accounts. An RPC failure must never be treated as "unfunded".
    pub fn verify_signatures(&self, account: Option<&AccountEntry>) -> Result<(), ChallengeError> {
        let client = stellar_strkey::ed25519::PublicKey::from_string(&self.account)
            .map_err(|_| ChallengeError::Account)?
            .0;
        let (mut signers, threshold) = if let Some(account) = account {
            if account.account_id != AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(client))) {
                return Err(ChallengeError::Account);
            }
            let mut signers = vec![(client, u32::from(account.thresholds.0[0]))];
            signers.extend(account.signers.iter().filter_map(|s| match &s.key {
                SignerKey::Ed25519(key) => Some((key.0, s.weight)),
                _ => None,
            }));
            (signers, u32::from(account.thresholds.0[2]).max(1))
        } else {
            (vec![(client, 1)], 1)
        };
        signers.retain(|(key, weight)| {
            *weight > 0
                && !verifies(key, &self.hash, &self.envelope.signatures[self.server_signature])
        });
        let mut used = HashSet::new();
        let mut weight = 0u32;
        for (index, signature) in self.envelope.signatures.iter().enumerate() {
            if index == self.server_signature {
                continue;
            }
            let Some((key, signer_weight)) =
                signers.iter().find(|(key, _)| verifies(key, &self.hash, signature))
            else {
                return Err(ChallengeError::Signature);
            };
            if !used.insert(*key) {
                return Err(ChallengeError::Signature);
            }
            weight = weight.saturating_add(*signer_weight);
        }
        if weight < threshold {
            return Err(ChallengeError::Signature);
        }
        Ok(())
    }
}

fn limits() -> Limits {
    Limits { depth: 32, len: 16_384 }
}
fn muxed(key: [u8; 32]) -> MuxedAccount {
    MuxedAccount::Ed25519(Uint256(key))
}
fn manage_data(source: [u8; 32], name: &str, value: &[u8]) -> Result<Operation, ChallengeError> {
    Ok(Operation {
        source_account: Some(muxed(source)),
        body: OperationBody::ManageData(ManageDataOp {
            data_name: name
                .as_bytes()
                .to_vec()
                .try_into()
                .map_err(|_| ChallengeError::Configuration)?,
            data_value: Some(DataValue(
                value.to_vec().try_into().map_err(|_| ChallengeError::Configuration)?,
            )),
        }),
    })
}

pub fn transaction_hash(tx: &Transaction, passphrase: &str) -> Result<[u8; 32], ChallengeError> {
    let payload = TransactionSignaturePayload {
        network_id: Hash(Sha256::digest(passphrase.as_bytes()).into()),
        tagged_transaction: TransactionSignaturePayloadTaggedTransaction::Tx(tx.clone()),
    };
    Ok(Sha256::digest(payload.to_xdr(limits()).map_err(|_| ChallengeError::Malformed)?).into())
}

fn sign(key: &SigningKey, hash: &[u8; 32]) -> DecoratedSignature {
    let public = key.verifying_key().to_bytes();
    DecoratedSignature {
        hint: SignatureHint(public[28..].try_into().unwrap()),
        signature: Signature(key.sign(hash).to_bytes().to_vec().try_into().unwrap()),
    }
}
fn verifies(key: &[u8; 32], hash: &[u8; 32], signature: &DecoratedSignature) -> bool {
    if signature.hint.0 != key[28..] {
        return false;
    }
    let Ok(key) = VerifyingKey::from_bytes(key) else {
        return false;
    };
    let Ok(signature) = ed25519_dalek::Signature::from_slice(signature.signature.0.as_slice())
    else {
        return false;
    };
    key.verify_strict(hash, &signature).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    const NETWORK: &str = "Test SDF Network ; September 2015";
    fn setup() -> (Sep10, SigningKey, String) {
        let server = stellar_strkey::ed25519::PrivateKey([7; 32]).to_string();
        let client = SigningKey::from_bytes(&[9; 32]);
        let address =
            format!("{}", stellar_strkey::ed25519::PublicKey(client.verifying_key().to_bytes()));
        (
            Sep10::new(&server, NETWORK, "example.com", "auth.example.com", 900).unwrap(),
            client,
            address,
        )
    }
    fn signed(challenge: &str, keys: &[&SigningKey], network: &str) -> String {
        let TransactionEnvelope::Tx(mut envelope) =
            TransactionEnvelope::from_xdr_base64(challenge, limits()).unwrap()
        else {
            panic!()
        };
        let hash = transaction_hash(&envelope.tx, network).unwrap();
        let mut signatures = envelope.signatures.to_vec();
        signatures.extend(keys.iter().map(|key| sign(key, &hash)));
        envelope.signatures = signatures.try_into().unwrap();
        TransactionEnvelope::Tx(envelope).to_xdr_base64(limits()).unwrap()
    }
    #[test]
    fn valid_unfunded_wallet_and_unique_nonces() {
        let (service, key, address) = setup();
        let challenge = service.build(&address, 100).unwrap();
        assert_ne!(challenge.hash, service.build(&address, 100).unwrap().hash);
        let parsed = service.parse(&signed(&challenge.transaction, &[&key], NETWORK), 101).unwrap();
        parsed.verify_signatures(None).unwrap();
        assert_eq!(parsed.account, address);
    }
    #[test]
    fn rejects_expired_future_malformed_wrong_signer_and_network() {
        let (service, key, address) = setup();
        let challenge = service.build(&address, 100).unwrap();
        let valid = signed(&challenge.transaction, &[&key], NETWORK);
        assert!(matches!(service.parse(&valid, 1000), Err(ChallengeError::TimeBounds)));
        assert!(matches!(service.parse(&valid, 99), Err(ChallengeError::TimeBounds)));
        assert!(service.parse("not xdr", 101).is_err());
        for bad in [
            signed(&challenge.transaction, &[&SigningKey::from_bytes(&[8; 32])], NETWORK),
            signed(&challenge.transaction, &[&key], "wrong network"),
            signed(&challenge.transaction, &[&key, &key], NETWORK),
            challenge.transaction,
        ] {
            assert!(service.parse(&bad, 101).unwrap().verify_signatures(None).is_err());
        }
    }
    #[test]
    fn rejects_modified_sequence_domain_source_operations_and_server_signature() {
        let (service, _, address) = setup();
        let challenge = service.build(&address, 100).unwrap();
        let TransactionEnvelope::Tx(original) =
            TransactionEnvelope::from_xdr_base64(&challenge.transaction, limits()).unwrap()
        else {
            panic!()
        };
        let mut variants = Vec::new();
        let mut changed = original.clone();
        changed.tx.seq_num.0 = 1;
        variants.push(changed);
        let mut changed = original.clone();
        changed.tx.source_account = muxed([1; 32]);
        variants.push(changed);
        let mut changed = original.clone();
        changed.tx.operations =
            vec![manage_data([1; 32], "evil auth", &[0; 64]).unwrap()].try_into().unwrap();
        variants.push(changed);
        let mut changed = original.clone();
        changed.signatures = VecM::default();
        variants.push(changed);
        let mut changed = original;
        changed.tx.fee += 1;
        variants.push(changed);
        for changed in variants {
            assert!(
                service
                    .parse(&TransactionEnvelope::Tx(changed).to_xdr_base64(limits()).unwrap(), 101)
                    .is_err()
            );
        }
    }
    #[test]
    fn enforces_medium_threshold_and_disabled_master_key() {
        let (service, master, address) = setup();
        let signer = SigningKey::from_bytes(&[3; 32]);
        let mut account = AccountEntry {
            account_id: AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(
                master.verifying_key().to_bytes(),
            ))),
            balance: 1,
            seq_num: SequenceNumber(1),
            num_sub_entries: 1,
            inflation_dest: None,
            flags: 0,
            home_domain: Default::default(),
            thresholds: Thresholds([1, 1, 2, 2]),
            signers: vec![Signer {
                key: SignerKey::Ed25519(Uint256(signer.verifying_key().to_bytes())),
                weight: 1,
            }]
            .try_into()
            .unwrap(),
            ext: AccountEntryExt::V0,
        };
        let challenge = service.build(&address, 100).unwrap();
        let one = service.parse(&signed(&challenge.transaction, &[&master], NETWORK), 101).unwrap();
        assert!(one.verify_signatures(Some(&account)).is_err());
        service
            .parse(&signed(&challenge.transaction, &[&master, &signer], NETWORK), 101)
            .unwrap()
            .verify_signatures(Some(&account))
            .unwrap();
        account.thresholds = Thresholds([0, 1, 1, 1]);
        assert!(one.verify_signatures(Some(&account)).is_err());
        service
            .parse(&signed(&challenge.transaction, &[&signer], NETWORK), 101)
            .unwrap()
            .verify_signatures(Some(&account))
            .unwrap();
    }
}
