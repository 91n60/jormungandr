mod tx_only;

use crate::mjolnir_app::MjolnirError;
use structopt::StructOpt;
pub use tx_only::TxOnly;
#[derive(StructOpt, Debug)]
pub enum Batch {
    /// Prints nodes related data, like stats,fragments etc.
    TxOnly(tx_only::TxOnly),
}

impl Batch {
    pub fn exec(&self) -> Result<(), MjolnirError> {
        match self {
            Batch::TxOnly(tx_only_command) => tx_only_command.exec(),
        }
    }
}
