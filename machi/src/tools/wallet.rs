//! Built-in wallet tools for agent operations.
//!
//! These tools allow agents to interact with their embedded wallet,
//! including address derivation, balance queries, and transaction sending.

use std::future::Future;
use std::pin::Pin;

use serde_json::Value;

use super::{Tool, ToolContext, ToolDefinition, ToolResult};
use crate::chain::{Chain, TransactionRequest};
use crate::get_optional_u32_arg;

/// Tool to get the agent's wallet address.
pub struct GetAddress;

impl GetAddress {
    /// Get the tool definition.
    pub fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "get_address",
            "Get the agent's wallet address on the current blockchain",
        )
        .optional_param(
            "index",
            "number",
            "The address derivation index (default: 0)",
        )
    }
}

impl<C: Chain + Send + Sync> Tool<C> for GetAddress {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "get_address",
            "Get the agent's wallet address on the current blockchain",
        )
        .optional_param(
            "index",
            "number",
            "The address derivation index (default: 0)",
        )
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a ToolContext<'a, C>,
        args: Value,
    ) -> Pin<Box<dyn Future<Output = ToolResult> + Send + 'a>> {
        Box::pin(async move {
            let index = get_optional_u32_arg!(args, "index", ctx.wallet.default_index());

            match ctx.chain.derive_address(ctx.wallet.inner(), index) {
                Ok(addr) => ToolResult::ok(serde_json::json!({
                    "address": addr.as_ref(),
                    "chain": ctx.chain.name(),
                    "index": index
                })),
                Err(e) => ToolResult::err(format!("Failed to derive address: {e}")),
            }
        })
    }
}

/// Tool to get the balance of an address.
pub struct GetBalance;

impl GetBalance {
    /// Get the tool definition.
    pub fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "get_balance",
            "Get the native token balance of an address on the current blockchain",
        )
        .optional_param(
            "address",
            "string",
            "The address to check (default: agent's own address)",
        )
    }
}

impl<C: Chain + Send + Sync> Tool<C> for GetBalance {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "get_balance",
            "Get the native token balance of an address on the current blockchain",
        )
        .optional_param(
            "address",
            "string",
            "The address to check (default: agent's own address)",
        )
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a ToolContext<'a, C>,
        args: Value,
    ) -> Pin<Box<dyn Future<Output = ToolResult> + Send + 'a>> {
        Box::pin(async move {
            // Get address - use provided or derive agent's address
            let address = match args.get("address").and_then(|v| v.as_str()) {
                Some(addr) => addr.to_string(),
                None => {
                    match ctx
                        .chain
                        .derive_address(ctx.wallet.inner(), ctx.wallet.default_index())
                    {
                        Ok(addr) => addr.as_ref().to_string(),
                        Err(e) => return ToolResult::err(format!("Failed to derive address: {e}")),
                    }
                }
            };

            match ctx.chain.balance(&address).await {
                Ok(balance) => ToolResult::ok(serde_json::json!({
                    "address": address,
                    "balance": balance.to_string(),
                    "chain": ctx.chain.name()
                })),
                Err(e) => ToolResult::err(format!("Failed to get balance: {e}")),
            }
        })
    }
}

/// Tool to send a transaction.
pub struct SendTransaction;

impl SendTransaction {
    /// Get the tool definition.
    pub fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "send_transaction",
            "Send native tokens to a recipient address. Returns the transaction hash on success.",
        )
        .param("to", "string", "The recipient address")
        .param(
            "value",
            "string",
            "The amount to send in the smallest unit (e.g., wei for ETH)",
        )
        .optional_param("data", "string", "Optional hex-encoded calldata for contract calls")
    }
}

impl<C: Chain + Send + Sync> Tool<C> for SendTransaction {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "send_transaction",
            "Send native tokens to a recipient address. Returns the transaction hash on success.",
        )
        .param("to", "string", "The recipient address")
        .param(
            "value",
            "string",
            "The amount to send in the smallest unit (e.g., wei for ETH)",
        )
        .optional_param("data", "string", "Optional hex-encoded calldata for contract calls")
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a ToolContext<'a, C>,
        args: Value,
    ) -> Pin<Box<dyn Future<Output = ToolResult> + Send + 'a>> {
        Box::pin(async move {
            // Extract required arguments
            let to = match args.get("to").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return ToolResult::err("Missing required argument: to"),
            };

            let value_str = match args.get("value").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return ToolResult::err("Missing required argument: value"),
            };

            let value: u128 = match value_str.parse() {
                Ok(v) => v,
                Err(_) => return ToolResult::err("Invalid value: must be a valid number"),
            };

            // Optional calldata
            let data = args
                .get("data")
                .and_then(|v| v.as_str())
                .and_then(|s| {
                    let s = s.strip_prefix("0x").unwrap_or(s);
                    hex::decode(s).ok()
                });

            let tx = TransactionRequest { to, value, data };

            match ctx
                .chain
                .send_transaction(ctx.wallet.inner(), ctx.wallet.default_index(), tx)
                .await
            {
                Ok(tx_hash) => ToolResult::ok(serde_json::json!({
                    "success": true,
                    "tx_hash": tx_hash.to_string(),
                    "chain": ctx.chain.name()
                })),
                Err(e) => ToolResult::err(format!("Transaction failed: {e}")),
            }
        })
    }
}

/// Tool to get the agent's mnemonic phrase (sensitive operation).
pub struct GetMnemonic;

impl GetMnemonic {
    /// Get the tool definition.
    pub fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "get_mnemonic",
            "Get the agent's wallet mnemonic phrase. WARNING: This is a sensitive operation!",
        )
    }
}

impl<C: Chain + Send + Sync> Tool<C> for GetMnemonic {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "get_mnemonic",
            "Get the agent's wallet mnemonic phrase. WARNING: This is a sensitive operation!",
        )
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a ToolContext<'a, C>,
        _args: Value,
    ) -> Pin<Box<dyn Future<Output = ToolResult> + Send + 'a>> {
        Box::pin(async move {
            ToolResult::ok(serde_json::json!({
                "mnemonic": ctx.wallet.mnemonic(),
                "warning": "Keep this secret! Anyone with this phrase can access your funds."
            }))
        })
    }
}

/// Create a tool registry with all built-in wallet tools.
pub fn create_wallet_tools<C: Chain + Send + Sync + 'static>() -> super::ToolRegistry<C> {
    let mut registry = super::ToolRegistry::new();
    registry.register(GetAddress);
    registry.register(GetBalance);
    registry.register(SendTransaction);
    // Note: GetMnemonic is intentionally excluded by default for security
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_address_definition() {
        let tool = GetAddress;
        let def = tool.definition();
        assert_eq!(def.name, "get_address");
        assert_eq!(def.parameters.len(), 1);
    }

    #[test]
    fn test_get_balance_definition() {
        let tool = GetBalance;
        let def = tool.definition();
        assert_eq!(def.name, "get_balance");
    }

    #[test]
    fn test_send_transaction_definition() {
        let tool = SendTransaction;
        let def = tool.definition();
        assert_eq!(def.name, "send_transaction");
        assert_eq!(def.parameters.len(), 3);
    }
}
