// Copyright (c) // Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::cli_types::{CommandAction, WalletContextOptions};
use async_trait::async_trait;
use clap::Parser;
use kanari_types::error::KanariResult;

use kanari_rpc_api::jsonrpc_types::{
    HumanReadableDisplay, IndexerObjectStatePageView, ObjectStateView,
};

/// Send a RPC request
#[derive(Debug, Parser)]
pub struct RequestCommand {
    /// The RPC method name
    /// --method kanari_getStates
    #[clap(long)]
    pub method: String,

    /// The RPC method params, json value.
    /// --params '"/resource/0x3/0x3::account::Account"'
    /// or
    /// --params '["/resource/0x3/0x3::account::Account", {"decode": true}]'
    #[clap(long)]
    pub params: Option<serde_json::Value>,

    #[clap(flatten)]
    pub(crate) context_options: WalletContextOptions,

    /// Return command outputs in json format
    #[clap(long, default_value = "false")]
    json: bool,
}

#[async_trait]
impl CommandAction<serde_json::Value> for RequestCommand {
    async fn execute(self) -> KanariResult<serde_json::Value> {
        let client = self.context_options.build()?.get_client().await?;
        let params = match self.params {
            Some(serde_json::Value::Array(array)) => array,
            Some(value) => {
                let s = value.as_str().unwrap();
                let ret = serde_json::from_str(s);
                match ret {
                    Ok(value) => value,
                    Err(_) => {
                        vec![serde_json::value::Value::String(s.to_string())]
                    }
                }
            }
            None => vec![],
        };
        Ok(client.request(self.method.as_str(), params).await?)
    }

    /// Executes the command, and serializes it to the common JSON output type
    async fn execute_serialized(self) -> KanariResult<String> {
        let method = self.method.clone();
        let json = self.json;
        let result = self.execute().await?;

        if json {
            let output = serde_json::to_string_pretty(&result).unwrap();
            if output == "null" {
                return Ok("".to_string());
            }
            Ok(output)
        } else if method == "kanari_getObjectStates" {
            let view = serde_json::from_value::<Vec<Option<ObjectStateView>>>(result.clone())?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            Ok(view.to_human_readable_string(true, 0))
        } else if method == "kanari_queryObjectStates" {
            Ok(
                serde_json::from_value::<IndexerObjectStatePageView>(result.clone())?
                    .to_human_readable_string(true, 0),
            )
        } else {
            // TODO: handle other rpc methods.
            let output = serde_json::to_string_pretty(&result).unwrap();
            if output == "null" {
                return Ok("".to_string());
            }
            Ok(output)
        }
    }
}
