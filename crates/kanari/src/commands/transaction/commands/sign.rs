// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use super::{FileOutput, FileOutputData};
use crate::cli_types::{CommandAction, FileOrHexInput, WalletContextOptions};
use crate::utils::prompt_yes_no;
use async_trait::async_trait;
use kanari_key::keystore::account_keystore::AccountKeystore;
use kanari_types::{
    address::{KanariAddress, ParsedAddress},
    bitcoin::multisign_account::MultisignAccountModule,
    error::KanariResult,
    transaction::{
        KanariTransaction, KanariTransactionData, authenticator::BitcoinAuthenticator,
        kanari::PartiallySignedKanariTransaction,
    },
};
use moveos_types::module_binding::MoveFunctionCaller;

#[derive(Debug, Clone)]
pub enum SignInput {
    KanariTransactionData(KanariTransactionData),
    PartiallySignedKanariTransaction(PartiallySignedKanariTransaction),
}

impl TryFrom<FileOrHexInput> for SignInput {
    type Error = anyhow::Error;

    fn try_from(value: FileOrHexInput) -> Result<Self, Self::Error> {
        let input = match bcs::from_bytes::<KanariTransactionData>(&value.data) {
            Ok(tx_data) => SignInput::KanariTransactionData(tx_data),
            Err(_) => {
                let psrt: PartiallySignedKanariTransaction = match bcs::from_bytes(&value.data) {
                    Ok(psrt) => psrt,
                    Err(_) => {
                        return Err(anyhow::anyhow!("Invalid tx data or psrt data"));
                    }
                };
                SignInput::PartiallySignedKanariTransaction(psrt)
            }
        };
        Ok(input)
    }
}

impl SignInput {
    pub fn sender(&self) -> KanariAddress {
        match self {
            SignInput::KanariTransactionData(tx_data) => tx_data.sender,
            SignInput::PartiallySignedKanariTransaction(psrt) => psrt.sender(),
        }
    }
}
pub enum SignOutput {
    SignedKanariTransaction(KanariTransaction),
    PartiallySignedKanariTransaction(PartiallySignedKanariTransaction),
}

impl SignOutput {
    pub fn is_finished(&self) -> bool {
        matches!(self, SignOutput::SignedKanariTransaction(_))
    }
}

impl From<SignOutput> for FileOutputData {
    fn from(val: SignOutput) -> Self {
        match val {
            SignOutput::SignedKanariTransaction(tx) => FileOutputData::SignedKanariTransaction(tx),
            SignOutput::PartiallySignedKanariTransaction(psrt) => {
                FileOutputData::PartiallySignedKanariTransaction(psrt)
            }
        }
    }
}

/// Get transactions by order
#[derive(Debug, clap::Parser)]
pub struct SignCommand {
    /// Input data to be used for signing
    /// Input can be a transaction data hex or a partially signed transaction data hex
    /// or a file path which contains transaction data or partially signed transaction data
    input: FileOrHexInput,

    /// The address of the signer when the transaction is a multisign account transaction
    /// If not specified, we will auto find the existing participants in the multisign account from the keystore
    #[clap(short = 's', long, value_parser=ParsedAddress::parse)]
    signer: Option<ParsedAddress>,

    /// The output file path for the signed transaction
    /// If not specified, the signed output will write to temp directory.
    #[clap(long, short = 'o')]
    output: Option<String>,

    /// Automatically answer 'yes' to all prompts
    #[clap(long = "yes", short = 'y')]
    answer_yes: bool,

    /// Return command outputs in json format
    #[clap(long, default_value = "false")]
    json: bool,

    #[clap(flatten)]
    context: WalletContextOptions,
}

impl SignCommand {
    async fn sign(self) -> anyhow::Result<SignOutput> {
        let context = self.context.build_require_password()?;
        let client = context.get_client().await?;
        let multisign_account_module = client.as_module_binding::<MultisignAccountModule>();
        let sign_input = SignInput::try_from(self.input)?;
        let sender = sign_input.sender();
        let output = if multisign_account_module.is_multisign_account(sender.into())? {
            let threshold = multisign_account_module.threshold(sender.into())?;

            let mut psrt = match sign_input {
                SignInput::KanariTransactionData(tx_data) => {
                    PartiallySignedKanariTransaction::new(tx_data, threshold)
                }
                SignInput::PartiallySignedKanariTransaction(psrt) => psrt,
            };
            match self.signer {
                Some(signer) => {
                    let signer = context.resolve_address(signer)?;
                    if !multisign_account_module.is_participant(sender.into(), signer)? {
                        return Err(anyhow::anyhow!(
                            "The signer address {} is not a participant in the multisign account",
                            signer
                        ));
                    }
                    let kp = context.get_key_pair(&signer.into())?;
                    let authenticator = BitcoinAuthenticator::sign(&kp, &psrt.data);
                    if psrt.contains_authenticator(&authenticator) {
                        return Err(anyhow::anyhow!(
                            "The signer has already signed the transaction"
                        ));
                    }
                    psrt.add_authenticator(authenticator)?;
                }
                None => {
                    let participants = multisign_account_module.participants(sender.into())?;
                    let mut has_participant = false;
                    for participant in participants.iter() {
                        if context
                            .keystore
                            .contains_address(&participant.participant_address.into())
                        {
                            has_participant = true;
                            let kp =
                                context.get_key_pair(&participant.participant_address.into())?;
                            let authenticator = BitcoinAuthenticator::sign(&kp, &psrt.data);
                            if psrt.contains_authenticator(&authenticator) {
                                continue;
                            }
                            psrt.add_authenticator(authenticator)?;
                        }
                    }
                    if !has_participant {
                        return Err(anyhow::anyhow!(
                            "No participant found in the multisign account from the keystore, participants: {:?}",
                            participants
                        ));
                    }
                }
            }

            if psrt.is_fully_signed() {
                SignOutput::SignedKanariTransaction(psrt.try_into_kanari_transaction()?)
            } else {
                SignOutput::PartiallySignedKanariTransaction(psrt)
            }
        } else {
            let tx_data = match sign_input {
                SignInput::KanariTransactionData(tx_data) => tx_data,
                SignInput::PartiallySignedKanariTransaction(_) => {
                    return Err(anyhow::anyhow!(
                        "Cannot sign a partially signed transaction with a single signer"
                    ));
                }
            };
            SignOutput::SignedKanariTransaction(context.sign_transaction(sender, tx_data)?)
        };
        Ok(output)
    }

    fn print_tx_details(input: &SignInput) {
        let tx_data = |tx_data: &KanariTransactionData| -> String {
            format!(
                " Sender: {}\n Sequence number: {}\n Chain id: {}\n Max gas amount: {}\n Action: {}\n Transaction hash: {:?}\n",
                tx_data.sender,
                tx_data.sequence_number,
                tx_data.chain_id,
                tx_data.max_gas_amount,
                tx_data.action,
                tx_data.tx_hash()
            )
        };

        match input {
            SignInput::KanariTransactionData(tx) => {
                println!("Transaction data:\n{}", tx_data(tx));
            }
            SignInput::PartiallySignedKanariTransaction(pstx) => {
                println!(
                    "Partially signed transaction data:\n{}",
                    tx_data(&pstx.data)
                );
                println!(
                    " Collected signatures: {}/{}",
                    pstx.authenticators.len(),
                    pstx.threshold
                );
            }
        }
    }
}

#[async_trait]
impl CommandAction<Option<FileOutput>> for SignCommand {
    async fn execute(self) -> KanariResult<Option<FileOutput>> {
        let sign_input = SignInput::try_from(self.input.clone())?;
        SignCommand::print_tx_details(&sign_input);
        if !self.answer_yes && !prompt_yes_no("Do you want to sign this transaction?") {
            return Ok(None);
        }
        let json = self.json;
        let output = self.output.clone();
        let sign_output = self.sign().await?;
        let is_finished = sign_output.is_finished();

        let file_output_data = sign_output.into();
        let file_output = FileOutput::write_to_file(file_output_data, output)?;

        if !json {
            if is_finished {
                println!("Signed transaction is written to {:?}", file_output.path);
                println!(
                    "You can submit the transaction with `kanari tx submit {}`",
                    file_output.path
                );
            } else {
                println!(
                    "Partially signed transaction is written to {:?}",
                    file_output.path
                );
                println!(
                    "You can send the partially signed transaction to other signers, and sign it later with `kanari tx sign {}`",
                    file_output.path
                );
            }
            Ok(None)
        } else {
            Ok(Some(file_output))
        }
    }
}
