use std::convert::{Infallible, identity};
use std::fmt::Debug;

use zcash_client_backend::data_api::testing::CacheInsertionResult;
use zcash_client_backend::{
    data_api::{
        OutputOfSentTx, SAPLING_SHARD_HEIGHT, WalletTest,
        testing::{DataStoreFactory, Reset, TestCache, TestState},
    },
    proto::compact_formats::CompactBlock,
    wallet::{Note, NoteId, ReceivedNote, Recipient},
};
use zcash_keys::address::Address;
use zcash_protocol::{
    ShieldedProtocol, TxId,
    consensus::BlockHeight,
    local_consensus::LocalNetwork,
    value::{ZatBalance, Zatoshis},
};

use shardtree::store::ShardStore;

use crate::{Account, AccountId, Error, MemBlockCache, MemoryWalletDb, SentNoteId};

#[cfg(feature = "transparent-inputs")]
use {
    zcash_client_backend::{
        data_api::{InputSource, WalletRead, wallet::TargetHeight},
        wallet::WalletTransparentOutput,
    },
    zcash_keys::keys::transparent::gap_limits::GapLimits,
};

pub mod pool;

#[cfg(test)]
#[cfg(feature = "transparent-inputs")]
mod transparent;

/// A test data store factory for in-memory databases
/// Very simple implementation just creates a new MemoryWalletDb
pub(crate) struct TestMemDbFactory;

impl TestMemDbFactory {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl DataStoreFactory for TestMemDbFactory {
    type Error = ();
    type AccountId = AccountId;
    type Account = Account;
    type DsError = Error;
    type DataStore = MemoryWalletDb<LocalNetwork>;

    fn new_data_store(
        &self,
        network: LocalNetwork,
        #[cfg(feature = "transparent-inputs")] _gap_limits: Option<GapLimits>,
    ) -> Result<Self::DataStore, Self::Error> {
        Ok(MemoryWalletDb::new(network, 100))
    }
}

#[derive(Debug)]
pub struct MemBlockCacheInsertionResult {
    txids: Vec<TxId>,
}

impl CacheInsertionResult for MemBlockCacheInsertionResult {
    fn txids(&self) -> &[TxId] {
        &self.txids[..]
    }
}

impl TestCache for MemBlockCache {
    type BsError = Infallible;
    type BlockSource = MemBlockCache;
    type InsertResult = MemBlockCacheInsertionResult;

    fn block_source(&self) -> &Self::BlockSource {
        self
    }

    fn insert(&mut self, cb: &CompactBlock) -> Self::InsertResult {
        let txids = cb.vtx.iter().map(|tx| tx.txid()).collect();
        self.0.write().unwrap().insert(cb.height(), cb.clone());
        MemBlockCacheInsertionResult { txids }
    }

    fn truncate_to_height(&mut self, height: BlockHeight) {
        self.0.write().unwrap().retain(|k, _| *k <= height);
    }
}

impl<P> Reset for MemoryWalletDb<P>
where
    P: zcash_protocol::consensus::Parameters + Clone + Debug + PartialEq,
{
    type Handle = ();

    fn reset<C>(st: &mut TestState<C, Self, LocalNetwork>) {
        let new_wallet = MemoryWalletDb::new(st.wallet().params.clone(), 100);
        let _ = std::mem::replace(st.wallet_mut(), new_wallet);
    }
}

impl<P> WalletTest for MemoryWalletDb<P>
where
    P: zcash_protocol::consensus::Parameters + Clone + Debug + PartialEq,
{
    #[allow(clippy::type_complexity)]
    fn get_sent_outputs(&self, txid: &TxId) -> Result<Vec<OutputOfSentTx>, Error> {
        self.sent_notes
            .iter()
            .filter(|(note_id, _)| note_id.txid() == txid)
            .map(|(_, note)| {
                Ok(match note.to.clone() {
                    Recipient::External {
                        recipient_address, ..
                    } => OutputOfSentTx::from_parts(
                        note.value,
                        Some(
                            Address::try_from_zcash_address(&self.params, recipient_address)
                                .map_err(Error::from)?,
                        ),
                        #[cfg(feature = "transparent-inputs")]
                        None,
                    ),
                    #[cfg(feature = "transparent-inputs")]
                    Recipient::EphemeralTransparent {
                        ephemeral_address,
                        receiving_account,
                        ..
                    } => {
                        let account = self.get_account(receiving_account)?.unwrap();
                        let (_addr, meta) = account
                            .ephemeral_addresses()?
                            .into_iter()
                            .find(|(addr, _)| addr == &ephemeral_address)
                            .expect("ephemeral address exists in the wallet");
                        OutputOfSentTx::from_parts(
                            note.value,
                            Some(Address::from(ephemeral_address)),
                            Some((
                                Address::from(ephemeral_address),
                                meta.address_index()
                                    .expect("ephemeral addresses are derived"),
                            )),
                        )
                    }
                    Recipient::InternalAccount { .. } => OutputOfSentTx::from_parts(
                        note.value,
                        None,
                        #[cfg(feature = "transparent-inputs")]
                        None,
                    ),
                    #[cfg(feature = "transparent-inputs")]
                    Recipient::InternalTransparent {
                        recipient_address, ..
                    } => OutputOfSentTx::from_parts(
                        note.value,
                        Some(Address::from(recipient_address)),
                        None,
                    ),
                })
            })
            .collect::<Result<_, Error>>()
    }

    /// Fetches the transparent output corresponding to the provided `outpoint`.
    /// Allows selecting unspendable outputs for testing purposes.
    ///
    /// Returns `Ok(None)` if the UTXO is not known to belong to the wallet or is not
    /// spendable as of the chain tip height.
    #[cfg(feature = "transparent-inputs")]
    fn get_transparent_output(
        &self,
        outpoint: &::transparent::bundle::OutPoint,
        _spendable_as_of: Option<TargetHeight>,
    ) -> Result<Option<WalletTransparentOutput<AccountId>>, <Self as InputSource>::Error> {
        // FIXME: perform spendability check according to `_spendable_as_of`
        Ok(self
            .transparent_received_outputs
            .get(outpoint)
            .map(|txo| (txo, self.tx_table.get(&txo.transaction_id)))
            .and_then(|(txo, tx)| {
                txo.to_wallet_transparent_output(
                    outpoint,
                    tx.and_then(|tx| tx.mined_height()),
                    self.find_funding_account(&txo.transaction_id),
                )
            }))
    }

    fn get_notes(
        &self,
        protocol: zcash_protocol::ShieldedProtocol,
    ) -> Result<Vec<ReceivedNote<NoteId, Note>>, Error> {
        Ok(self
            .received_notes
            .iter()
            .filter(|rn| rn.note.protocol() == protocol)
            .cloned()
            .map(Into::into)
            .collect())
    }

    /// Returns the note IDs for shielded notes sent by the wallet in a particular
    /// transaction.
    fn get_sent_note_ids(
        &self,
        txid: &TxId,
        protocol: ShieldedProtocol,
    ) -> Result<Vec<NoteId>, Error> {
        Ok(self
            .get_sent_notes()
            .keys()
            .filter_map(|id| {
                if let SentNoteId::Shielded(id) = id {
                    if id.txid() == txid && id.protocol() == protocol {
                        Some(*id)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect())
    }

    /// Returns a vector of transaction summaries.
    ///
    /// Currently test-only, as production use could return a very large number of results; either
    /// pagination or a streaming design will be necessary to stabilize this feature for production
    /// use.⁄
    fn get_tx_history(
        &self,
    ) -> Result<Vec<zcash_client_backend::data_api::testing::TransactionSummary<AccountId>>, Error>
    {
        let mut history = self
            .tx_table
            .iter()
            .map(|(txid, tx)| {
                // find all the notes associated with this transaction
                // A transaction may send and/or receive one or more notes

                // notes spent (consumed) by the transaction
                let spent_notes = self
                    .received_note_spends
                    .iter()
                    .filter(|(_, spend_txid)| *spend_txid == txid)
                    .collect::<Vec<_>>();

                let spent_utxos = self
                    .transparent_received_output_spends
                    .iter()
                    .filter(|(_, spend_txid)| *spend_txid == txid)
                    .collect::<Vec<_>>();

                // notes produced (sent) by the transaction (excluding change)
                let sent_notes = self
                    .sent_notes
                    .iter()
                    .filter(|(note_id, _)| note_id.txid() == txid)
                    .filter(|(note_id, _)| {
                        // use a join on the received notes table to detect which are change
                        self.received_notes.iter().any(|received_note| {
                            SentNoteId::from(received_note.note_id) == **note_id
                                && !received_note.is_change
                        })
                    })
                    .collect::<Vec<_>>();

                let received_txo = self
                    .transparent_received_outputs
                    .iter()
                    .filter(|(outpoint, _received_output)| outpoint.txid() == txid)
                    .collect::<Vec<_>>();

                let sent_txo_value: u64 = received_txo
                    .iter()
                    .map(|(_, o)| u64::from(o.txout.value()))
                    .sum();

                // notes received by the transaction
                let received_notes = self
                    .received_notes
                    .iter()
                    .filter(|received_note| received_note.txid() == *txid)
                    .collect::<Vec<_>>();

                // A transaction can send and receive notes to/from multiple accounts
                // For a transaction to be visible to this wallet it must have either scanned it from the chain
                // or been created by this wallet so there are number of ways we can detect the account ID
                let receiving_account_id = received_notes.first().map(|note| note.account_id());
                let sending_account_id = sent_notes.first().map(|(_, note)| note.from_account_id);
                let receiving_transparent_account_id = received_txo
                    .first()
                    .map(|(_, received)| received.account_id);
                let sent_txo_account_id = spent_utxos.first().and_then(|(outpoint, _)| {
                    // any spent txo was first a received txo
                    self.transparent_received_outputs
                        .get(outpoint)
                        .map(|txo| txo.account_id)
                });

                // take the first non-none account_id
                let account_id = vec![
                    receiving_account_id,
                    sending_account_id,
                    receiving_transparent_account_id,
                    sent_txo_account_id,
                ]
                .into_iter()
                .find_map(identity)
                .ok_or(Error::Other(
                    format!("Account id could not be found for tx: {}", txid).to_string(),
                ))?;

                let balance_gained: u64 = received_notes
                    .iter()
                    .map(|note| note.note.value().into_u64())
                    .sum::<u64>()
                    + sent_txo_value;

                let balance_lost: u64 = self // includes change
                    .sent_notes
                    .iter()
                    .filter(|(note_id, _)| note_id.txid() == txid)
                    .map(|(_, sent_note)| sent_note.value.into_u64())
                    .sum::<u64>()
                    + tx.fee().map(u64::from).unwrap_or(0);

                let is_shielding = {
                    //All of the wallet-spent and wallet-received notes are consistent with a shielding transaction.
                    // e.g. only transparent outputs are spend and only shielded notes are received
                    spent_notes.is_empty() && !spent_utxos.is_empty()
                        // The transaction contains at least one wallet-received note.
                        && !received_notes.is_empty()
                        // We do not know about any external outputs of the transaction.
                        && sent_notes.is_empty()
                };

                let has_change = received_notes.iter().any(|note| note.is_change);

                Ok(
                    zcash_client_backend::data_api::testing::TransactionSummary::from_parts(
                        account_id,                                                             // account_id
                        *txid,              // txid
                        tx.expiry_height(), // expiry_height
                        tx.mined_height(),  // mined_height
                        ZatBalance::from_i64((balance_gained as i64) - (balance_lost as i64))?, // account_value_delta
                        Zatoshis::from_u64(balance_lost)?,
                        Zatoshis::from_u64(balance_gained)?,
                        tx.fee(),                              // fee_paid
                        spent_notes.len() + spent_utxos.len(), // spent_note_count
                        has_change,                            // has_change
                        sent_notes.len(),                      // sent_note_count (excluding change)
                        received_notes.iter().filter(|note| !note.is_change).count(), // received_note_count (excluding change)
                        0,            // Unimplemented: memo_count
                        false,        // Unimplemented: expired_unmined
                        is_shielding, // is_shielding
                    ),
                )
            })
            .collect::<Result<Vec<_>, Error>>()?;
        history.sort_by(|a, b| {
            b.mined_height()
                .cmp(&a.mined_height())
                .then(b.txid().cmp(&a.txid()))
        });
        Ok(history)
    }

    fn get_checkpoint_history(
        &self,
        protocol: &ShieldedProtocol,
    ) -> Result<Vec<(BlockHeight, Option<incrementalmerkletree::Position>)>, Error> {
        let mut checkpoints = Vec::new();

        match protocol {
            ShieldedProtocol::Sapling => {
                self.sapling_tree
                    .store()
                    .for_each_checkpoint(usize::MAX, |id, cp| {
                        checkpoints.push((*id, cp.position()));
                        Ok(())
                    })?;
            }
            #[cfg(feature = "orchard")]
            ShieldedProtocol::Orchard => {
                self.orchard_tree
                    .store()
                    .for_each_checkpoint(usize::MAX, |id, cp| {
                        checkpoints.push((*id, cp.position()));
                        Ok(())
                    })?;
            }
            #[cfg(not(feature = "orchard"))]
            _ => {}
        }

        checkpoints.sort_by_key(|(a, _)| *a);

        Ok(checkpoints)
    }

    fn finally(&self) {
        // ensure the wallet state at the conclusion of each test can be round-tripped through serialization
        let proto = crate::proto::memwallet::MemoryWallet::from(self);
        let recovered_wallet =
            MemoryWalletDb::new_from_proto(proto.clone(), self.params.clone(), 100).unwrap();

        assert_eq!(self, &recovered_wallet);

        // ensure the trees can be roundtripped
        use crate::wallet_commitment_trees::serialization::{tree_from_protobuf, tree_to_protobuf};

        let tree_proto = tree_to_protobuf(&self.sapling_tree).unwrap().unwrap();
        let recovered_tree: shardtree::ShardTree<
            shardtree::store::memory::MemoryShardStore<sapling::Node, BlockHeight>,
            { SAPLING_SHARD_HEIGHT * 2 },
            SAPLING_SHARD_HEIGHT,
        > = tree_from_protobuf(tree_proto, 100, 16.into()).unwrap();

        assert_eq!(
            self.sapling_tree.store().get_shard_roots(),
            recovered_tree.store().get_shard_roots()
        );
    }
}

#[cfg(all(test, feature = "pczt-tests"))]
mod tests {
    use rand_core::OsRng;
    use secrecy::SecretVec;
    use transparent::builder::TransparentSigningSet;
    use transparent::keys::IncomingViewingKey;
    use zcash_client_backend::data_api::{
        AccountBirthday, WalletTest as _, WalletWrite as _, chain::ChainState,
        wallet::decrypt_and_store_transaction,
    };
    use zcash_keys::{address::Address, keys::UnifiedSpendingKey};
    use zcash_primitives::{
        block::BlockHash,
        transaction::builder::{BuildConfig, Builder},
    };
    use zcash_protocol::{
        consensus::{NetworkUpgrade, Parameters as _, ZIP212_GRACE_PERIOD},
        memo::MemoBytes,
        value::Zatoshis,
    };
    use zip32::Scope;

    use super::MemoryWalletDb;

    #[test]
    fn decrypt_and_store_transaction_without_chain_context_can_surface_legacy_sapling_state_through_sent_history()
     {
        let network = zcash_protocol::local_consensus::LocalNetwork {
            overwinter: Some(zcash_protocol::consensus::BlockHeight::from_u32(1)),
            sapling: Some(zcash_protocol::consensus::BlockHeight::from_u32(1)),
            blossom: Some(zcash_protocol::consensus::BlockHeight::from_u32(1)),
            heartwood: Some(zcash_protocol::consensus::BlockHeight::from_u32(1)),
            canopy: Some(zcash_protocol::consensus::BlockHeight::from_u32(1)),
            nu5: Some(zcash_protocol::consensus::BlockHeight::from_u32(1)),
            nu6: None,
            nu6_1: None,
            #[cfg(zcash_unstable = "nu7")]
            nu7: None,
            #[cfg(zcash_unstable = "zfuture")]
            z_future: None,
        };

        let legacy_target_height = network
            .activation_height(NetworkUpgrade::Sapling)
            .expect("Sapling activation height is known on the local test network");
        let current_semantics_height = network
            .activation_height(NetworkUpgrade::Canopy)
            .expect("Canopy activation height is known on the local test network")
            + ZIP212_GRACE_PERIOD
            + 10;

        let seed = SecretVec::new(vec![0x11; 32]);
        let birthday = AccountBirthday::from_parts(
            ChainState::empty(legacy_target_height - 1, BlockHash([0; 32])),
            None,
        );

        let mut wallet_without_context = MemoryWalletDb::new(network, 100);
        let (_account_id, sender_usk) = wallet_without_context
            .create_account("test account", &seed, &birthday, None)
            .unwrap();
        let sender_dfvk = sender_usk.sapling().to_diversifiable_full_viewing_key();

        let mut wallet_with_context = MemoryWalletDb::new(network, 100);
        wallet_with_context
            .create_account("test account", &seed, &birthday, None)
            .unwrap();

        let transparent_account_sk = transparent::keys::AccountPrivKey::from_seed(
            &network,
            &[0x88; 32],
            zip32::AccountId::ZERO,
        )
        .unwrap();
        let (transparent_addr, address_index) = transparent_account_sk
            .to_account_pubkey()
            .derive_external_ivk()
            .unwrap()
            .default_address();
        let transparent_sk = transparent_account_sk
            .derive_external_secret_key(address_index)
            .unwrap();
        let secp = secp256k1::Secp256k1::signing_only();
        let transparent_pubkey = transparent_sk.public_key(&secp);

        let legacy_external_usk =
            UnifiedSpendingKey::from_seed(&network, &[0x99; 32], zip32::AccountId::ZERO).unwrap();
        let legacy_external_recipient = legacy_external_usk
            .sapling()
            .to_diversifiable_full_viewing_key()
            .default_address()
            .1;
        let legacy_external_recipient_addr: Address = legacy_external_recipient.into();

        let legacy_change_usk =
            UnifiedSpendingKey::from_seed(&network, &[0xaa; 32], zip32::AccountId::ZERO).unwrap();
        let legacy_change_recipient = legacy_change_usk
            .sapling()
            .to_diversifiable_full_viewing_key()
            .default_address()
            .1;

        let utxo = transparent::bundle::OutPoint::fake();
        let coin = transparent::bundle::TxOut::new(
            Zatoshis::const_from_u64(1_000_000),
            transparent_addr.script().into(),
        );

        let mut legacy_builder = Builder::new(
            network,
            legacy_target_height,
            BuildConfig::Standard {
                sapling_anchor: Some(sapling::Anchor::empty_tree()),
                orchard_anchor: Some(orchard::Anchor::empty_tree()),
            },
        );
        legacy_builder
            .add_transparent_p2pkh_input(transparent_pubkey, utxo, coin)
            .unwrap();
        legacy_builder
            .add_sapling_output::<core::convert::Infallible>(
                Some(sender_dfvk.to_ovk(Scope::External)),
                legacy_external_recipient,
                Zatoshis::const_from_u64(100_000),
                MemoBytes::empty(),
            )
            .unwrap();
        legacy_builder
            .add_sapling_output::<core::convert::Infallible>(
                Some(sender_dfvk.to_ovk(Scope::Internal)),
                legacy_change_recipient,
                Zatoshis::const_from_u64(885_000),
                MemoBytes::empty(),
            )
            .unwrap();

        let mut transparent_signing_set = TransparentSigningSet::new();
        transparent_signing_set.add_key(transparent_sk);
        let legacy_tx = legacy_builder
            .mock_build(&transparent_signing_set, &[], &[], OsRng)
            .unwrap()
            .transaction()
            .clone();
        let legacy_txid = legacy_tx.txid();

        decrypt_and_store_transaction(&network, &mut wallet_without_context, &legacy_tx, None)
            .unwrap();

        let initial_legacy_sent_outputs = wallet_without_context
            .get_sent_outputs(&legacy_txid)
            .unwrap();
        let initial_legacy_displayed = initial_legacy_sent_outputs
            .iter()
            .find_map(|output| output.external_recipient().cloned())
            .expect("missing chain context should surface the legacy external recipient");
        assert_eq!(initial_legacy_displayed, legacy_external_recipient_addr);

        decrypt_and_store_transaction(
            &network,
            &mut wallet_with_context,
            &legacy_tx,
            Some(current_semantics_height),
        )
        .unwrap();
        let contextual_outputs = wallet_with_context.get_sent_outputs(&legacy_txid).unwrap();
        assert!(
            !contextual_outputs.is_empty(),
            "the transaction remains visible to sent-history reconstruction even when reprocessed under current semantics in this in-memory path",
        );
    }
}
