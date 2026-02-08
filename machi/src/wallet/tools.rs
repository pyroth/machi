//! `DynTool` implementations for wallet operations.
//!
//! Each tool wraps an `Arc<EvmWallet>` and exposes a specific wallet
//! capability to the agent via the [`DynTool`] interface.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use super::evm::EvmWallet;
use crate::error::ToolError;
use crate::tool::{DynTool, ToolDefinition};

/// Returns the wallet's Ethereum address.
#[derive(Debug)]
pub struct GetWalletAddressTool {
    wallet: Arc<EvmWallet>,
}

impl GetWalletAddressTool {
    pub const fn new(wallet: Arc<EvmWallet>) -> Self {
        Self { wallet }
    }
}

#[async_trait]
impl DynTool for GetWalletAddressTool {
    fn name(&self) -> &str {
        "get_wallet_address"
    }

    fn description(&self) -> String {
        "Get the agent's own Ethereum wallet address".into()
    }

    fn definition(&self) -> ToolDefinition {
        let params = serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        });
        ToolDefinition::new(self.name(), self.description(), params)
    }

    async fn call_json(&self, _args: Value) -> Result<Value, ToolError> {
        Ok(Value::String(self.wallet.address_string()))
    }
}

/// Query the native token (ETH) balance for any address.
#[derive(Debug)]
pub struct GetEthBalanceTool {
    wallet: Arc<EvmWallet>,
}

impl GetEthBalanceTool {
    pub const fn new(wallet: Arc<EvmWallet>) -> Self {
        Self { wallet }
    }
}

#[async_trait]
impl DynTool for GetEthBalanceTool {
    fn name(&self) -> &str {
        "get_eth_balance"
    }

    fn description(&self) -> String {
        "Get the ETH (native token) balance of an address. \
         If no address is provided, returns the agent's own balance."
            .into()
    }

    fn definition(&self) -> ToolDefinition {
        let params = serde_json::json!({
            "type": "object",
            "properties": {
                "address": {
                    "type": "string",
                    "description": "The Ethereum address to check (hex, 0x-prefixed). Omit to check the agent's own balance."
                }
            },
            "required": []
        });
        ToolDefinition::new(self.name(), self.description(), params)
    }

    async fn call_json(&self, args: Value) -> Result<Value, ToolError> {
        let balance = if let Some(addr_str) = args.get("address").and_then(|v| v.as_str()) {
            let address: alloy::primitives::Address = addr_str
                .parse()
                .map_err(|e| ToolError::invalid_args(format!("invalid address: {e}")))?;
            self.wallet.balance_of(address).await?
        } else {
            self.wallet.balance().await?
        };

        // Return balance in wei as a string to preserve precision.
        Ok(Value::String(balance.to_string()))
    }
}

/// Sign an arbitrary message using EIP-191 `personal_sign`.
#[derive(Debug)]
pub struct SignMessageTool {
    wallet: Arc<EvmWallet>,
}

impl SignMessageTool {
    pub const fn new(wallet: Arc<EvmWallet>) -> Self {
        Self { wallet }
    }
}

#[async_trait]
impl DynTool for SignMessageTool {
    fn name(&self) -> &str {
        "sign_message"
    }

    fn description(&self) -> String {
        "Sign an arbitrary message using EIP-191 personal_sign. \
         Returns the hex-encoded signature."
            .into()
    }

    fn definition(&self) -> ToolDefinition {
        let params = serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The message to sign"
                }
            },
            "required": ["message"]
        });
        ToolDefinition::new(self.name(), self.description(), params)
    }

    async fn call_json(&self, args: Value) -> Result<Value, ToolError> {
        let message = args
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::invalid_args("missing required field 'message'"))?;

        let signature = self.wallet.sign_message(message.as_bytes()).await?;
        Ok(Value::String(signature))
    }
}

/// Send native token (ETH) to an address.
#[derive(Debug)]
pub struct TransferEthTool {
    wallet: Arc<EvmWallet>,
}

impl TransferEthTool {
    pub const fn new(wallet: Arc<EvmWallet>) -> Self {
        Self { wallet }
    }
}

#[async_trait]
impl DynTool for TransferEthTool {
    fn name(&self) -> &str {
        "transfer_eth"
    }

    fn description(&self) -> String {
        "Transfer ETH (native token) to a specified address. \
         Amount is in wei (1 ETH = 10^18 wei). Returns the transaction hash."
            .into()
    }

    fn definition(&self) -> ToolDefinition {
        let params = serde_json::json!({
            "type": "object",
            "properties": {
                "to": {
                    "type": "string",
                    "description": "The recipient Ethereum address (hex, 0x-prefixed)"
                },
                "amount": {
                    "type": "string",
                    "description": "The amount to transfer in wei (e.g. \"1000000000000000000\" for 1 ETH)"
                }
            },
            "required": ["to", "amount"]
        });
        ToolDefinition::new(self.name(), self.description(), params)
    }

    async fn call_json(&self, args: Value) -> Result<Value, ToolError> {
        let to_str = args
            .get("to")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::invalid_args("missing required field 'to'"))?;
        let amount_str = args
            .get("amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::invalid_args("missing required field 'amount'"))?;

        let to: alloy::primitives::Address = to_str
            .parse()
            .map_err(|e| ToolError::invalid_args(format!("invalid address: {e}")))?;
        let amount = alloy::primitives::U256::from_str_radix(amount_str, 10)
            .map_err(|e| ToolError::invalid_args(format!("invalid amount: {e}")))?;

        let tx_hash = self.wallet.transfer(to, amount).await?;

        Ok(serde_json::json!({
            "tx_hash": tx_hash,
            "from": self.wallet.address_string(),
            "to": to_str,
            "amount": amount_str
        }))
    }
}
