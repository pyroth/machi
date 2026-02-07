//! EVM-compatible wallet implementation.
//!
//! Provides [`EvmWallet`] for signing transactions and interacting with
//! EVM-compatible blockchains. Built on top of [`kobe`] for HD key derivation
//! and [`alloy`] for signing and RPC communication.

use alloy::network::Ethereum;
use alloy::primitives::{Address, U256};
use alloy::providers::{DynProvider, Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::{Signer, SignerSync};
use std::sync::Arc;
use tracing::info;

use super::error::WalletError;
use crate::tool::BoxedTool;

/// Builder for constructing an [`EvmWallet`].
///
/// Created by [`EvmWallet::builder`]. Use method chaining to configure
/// the wallet, then call [`build`](Self::build).
///
/// # Examples
///
/// ```rust,ignore
/// // From HD mnemonic
/// let wallet = EvmWallet::builder()
///     .mnemonic("abandon abandon ...")
///     .index(0)
///     .rpc_url("https://eth-mainnet.g.alchemy.com/v2/xxx")
///     .build()?;
///
/// // From private key
/// let wallet = EvmWallet::builder()
///     .private_key("0xabc...")
///     .rpc_url("https://eth-mainnet.g.alchemy.com/v2/xxx")
///     .build()?;
/// ```
#[derive(Debug, Default)]
pub struct EvmWalletBuilder {
    /// BIP39 mnemonic phrase.
    mnemonic: Option<String>,
    /// BIP39 passphrase (optional "25th word").
    passphrase: Option<String>,
    /// HD derivation index (default 0).
    index: u32,
    /// Raw private key hex string.
    private_key: Option<String>,
    /// JSON-RPC endpoint URL.
    rpc_url: Option<String>,
    /// Chain ID (auto-detected if not set).
    chain_id: Option<u64>,
}

impl EvmWalletBuilder {
    /// Set the BIP39 mnemonic phrase for HD key derivation.
    #[must_use]
    pub fn mnemonic(mut self, mnemonic: impl Into<String>) -> Self {
        self.mnemonic = Some(mnemonic.into());
        self
    }

    /// Set the BIP39 passphrase (optional "25th word").
    #[must_use]
    pub fn passphrase(mut self, passphrase: impl Into<String>) -> Self {
        self.passphrase = Some(passphrase.into());
        self
    }

    /// Set the HD derivation index (default 0).
    #[must_use]
    pub const fn index(mut self, index: u32) -> Self {
        self.index = index;
        self
    }

    /// Set the private key directly (hex string, with or without 0x prefix).
    #[must_use]
    pub fn private_key(mut self, key: impl Into<String>) -> Self {
        self.private_key = Some(key.into());
        self
    }

    /// Set the JSON-RPC endpoint URL.
    #[must_use]
    pub fn rpc_url(mut self, url: impl Into<String>) -> Self {
        self.rpc_url = Some(url.into());
        self
    }

    /// Set the chain ID explicitly (auto-detected from RPC if not set).
    #[must_use]
    pub const fn chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = Some(chain_id);
        self
    }

    /// Build the [`EvmWallet`].
    ///
    /// Either `mnemonic` or `private_key` must be set. `rpc_url` is required.
    pub async fn build(mut self) -> crate::Result<EvmWallet> {
        let rpc_url = self
            .rpc_url
            .take()
            .ok_or_else(|| WalletError::Config("rpc_url is required".into()))?;

        // Derive the signer from mnemonic or private key.
        let mut signer = if let Some(ref mnemonic) = self.mnemonic {
            self.signer_from_mnemonic(mnemonic)?
        } else if let Some(ref key) = self.private_key {
            Self::signer_from_private_key(key)?
        } else {
            return Err(
                WalletError::Config("either mnemonic or private_key is required".into()).into(),
            );
        };

        // Set chain ID on the signer if provided.
        if let Some(chain_id) = self.chain_id {
            signer.set_chain_id(Some(chain_id));
        }

        let address = signer.address();

        // Build provider with wallet and recommended fillers.
        let provider: DynProvider<Ethereum> = ProviderBuilder::new()
            .wallet(signer.clone())
            .connect(&rpc_url)
            .await
            .map_err(|e| WalletError::Provider(format!("failed to connect to '{rpc_url}': {e}")))?
            .erased();

        // Auto-detect chain ID if not explicitly set.
        let chain_id = if let Some(id) = self.chain_id {
            id
        } else {
            provider
                .get_chain_id()
                .await
                .map_err(|e| WalletError::Provider(format!("failed to get chain ID: {e}")))?
        };

        info!(
            address = %address,
            chain_id = chain_id,
            "EVM wallet initialized",
        );

        Ok(EvmWallet {
            signer,
            provider: Arc::new(provider),
            address,
            chain_id,
        })
    }

    /// Derive a signer from a BIP39 mnemonic using kobe.
    fn signer_from_mnemonic(&self, mnemonic: &str) -> Result<PrivateKeySigner, WalletError> {
        let wallet = kobe::Wallet::from_mnemonic(mnemonic, self.passphrase.as_deref())
            .map_err(|e| WalletError::Derivation(format!("invalid mnemonic: {e}")))?;

        let deriver = kobe_eth::Deriver::new(&wallet);
        let derived = deriver
            .derive(self.index)
            .map_err(|e| WalletError::Derivation(format!("key derivation failed: {e}")))?;

        let key_hex = &*derived.private_key_hex;
        key_hex
            .parse::<PrivateKeySigner>()
            .map_err(|e| WalletError::Derivation(format!("signer creation failed: {e}")))
    }

    /// Create a signer from a raw private key hex string.
    fn signer_from_private_key(key: &str) -> Result<PrivateKeySigner, WalletError> {
        let key = key.strip_prefix("0x").unwrap_or(key);
        key.parse::<PrivateKeySigner>()
            .map_err(|e| WalletError::Config(format!("invalid private key: {e}")))
    }
}

/// An EVM-compatible wallet for AI agent blockchain interactions.
///
/// `EvmWallet` combines HD key derivation ([`kobe`]), local signing
/// ([`alloy`] `PrivateKeySigner`), and RPC communication to provide a
/// complete wallet that agents can use to read on-chain data and submit
/// transactions.
///
/// # Construction
///
/// Use [`EvmWallet::builder`] with method chaining:
///
/// ```rust,ignore
/// let wallet = EvmWallet::builder()
///     .mnemonic("abandon abandon abandon ...")
///     .rpc_url("https://eth-mainnet.g.alchemy.com/v2/xxx")
///     .build()
///     .await?;
/// ```
///
/// # As Agent Tools
///
/// Convert the wallet into agent-callable tools:
///
/// ```rust,ignore
/// let tools = wallet.tools();
/// let agent = Agent::new("defi-agent")
///     .tools(tools)
///     .provider(llm_provider);
/// ```
pub struct EvmWallet {
    /// Local signer for transaction and message signing.
    signer: PrivateKeySigner,
    /// Type-erased provider for RPC calls.
    provider: Arc<DynProvider<Ethereum>>,
    /// The wallet's Ethereum address.
    address: Address,
    /// The chain ID this wallet is connected to.
    chain_id: u64,
}

impl std::fmt::Debug for EvmWallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvmWallet")
            .field("address", &self.address)
            .field("chain_id", &self.chain_id)
            .finish_non_exhaustive()
    }
}

impl EvmWallet {
    /// Create a builder for constructing an [`EvmWallet`].
    #[must_use]
    pub fn builder() -> EvmWalletBuilder {
        EvmWalletBuilder::default()
    }

    /// Get the wallet's Ethereum address.
    #[must_use]
    pub const fn address(&self) -> Address {
        self.address
    }

    /// Get the checksummed address string.
    #[must_use]
    pub fn address_string(&self) -> String {
        self.address.to_checksum(None)
    }

    /// Get the chain ID.
    #[must_use]
    pub const fn chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Get the native token (ETH) balance for the wallet's address.
    pub async fn balance(&self) -> Result<U256, WalletError> {
        self.balance_of(self.address).await
    }

    /// Get the native token (ETH) balance for any address.
    pub async fn balance_of(&self, address: Address) -> Result<U256, WalletError> {
        self.provider
            .get_balance(address)
            .await
            .map_err(|e| WalletError::Provider(format!("failed to get balance: {e}")))
    }

    /// Get the current block number.
    pub async fn block_number(&self) -> Result<u64, WalletError> {
        self.provider
            .get_block_number()
            .await
            .map_err(|e| WalletError::Provider(format!("failed to get block number: {e}")))
    }

    /// Sign an arbitrary message (EIP-191 personal_sign).
    pub async fn sign_message(&self, message: &[u8]) -> Result<String, WalletError> {
        let sig = self
            .signer
            .sign_message(message)
            .await
            .map_err(|e| WalletError::Signing(format!("message signing failed: {e}")))?;
        Ok(format!(
            "0x{}",
            alloy::primitives::hex::encode(sig.as_bytes())
        ))
    }

    /// Sign an arbitrary message synchronously.
    pub fn sign_message_sync(&self, message: &[u8]) -> Result<String, WalletError> {
        let sig = self
            .signer
            .sign_message_sync(message)
            .map_err(|e| WalletError::Signing(format!("message signing failed: {e}")))?;
        Ok(format!(
            "0x{}",
            alloy::primitives::hex::encode(sig.as_bytes())
        ))
    }

    /// Send native token (ETH) to an address.
    ///
    /// Returns the transaction hash.
    pub async fn transfer(&self, to: Address, value: U256) -> Result<String, WalletError> {
        use alloy::network::TransactionBuilder;
        use alloy::rpc::types::TransactionRequest;

        let tx = TransactionRequest::default().with_to(to).with_value(value);

        let receipt = self
            .provider
            .send_transaction(tx)
            .await
            .map_err(|e| WalletError::Transaction(format!("send failed: {e}")))?
            .get_receipt()
            .await
            .map_err(|e| WalletError::Transaction(format!("receipt failed: {e}")))?;

        Ok(format!("{:#x}", receipt.transaction_hash))
    }

    /// Get a reference to the underlying signer.
    #[must_use]
    pub const fn signer(&self) -> &PrivateKeySigner {
        &self.signer
    }

    /// Get a reference to the underlying provider.
    #[must_use]
    pub fn provider(&self) -> &DynProvider<Ethereum> {
        &self.provider
    }

    /// Convert this wallet into a set of agent-callable [`BoxedTool`]s.
    ///
    /// Returns tools for:
    /// - `get_wallet_address` — returns the wallet address
    /// - `get_eth_balance` — query ETH balance
    /// - `sign_message` — EIP-191 personal sign
    /// - `transfer_eth` — send ETH to an address
    pub fn tools(self) -> Vec<BoxedTool> {
        let wallet = Arc::new(self);
        vec![
            Box::new(super::tools::GetWalletAddressTool::new(Arc::clone(&wallet))),
            Box::new(super::tools::GetEthBalanceTool::new(Arc::clone(&wallet))),
            Box::new(super::tools::SignMessageTool::new(Arc::clone(&wallet))),
            Box::new(super::tools::TransferEthTool::new(wallet)),
        ]
    }
}
