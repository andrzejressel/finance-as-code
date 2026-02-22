use crate::queries::{add_transactions, clean_account, regenerate_balances};
use finance_as_code_budget_core::Transaction;
use finance_as_code_budget_core::sink::Sink;
use log::info;
use rootcause::compat::IntoRootcause;
use rootcause::option_ext::OptionExt;
use rootcause::prelude::ResultExt;
use rootcause::*;

mod queries;
mod utils;

#[derive(bon::Builder)]
pub struct SinkTreelineOptions {
    #[builder(into)]
    /// Account name in Treeline whose data will be fully replaced: existing
    /// transactions and balances will be cleared and replaced with the
    /// provided transactions before balances are regenerated.
    account_name: String,
}

pub fn create_treeline_sink(options: SinkTreelineOptions) -> impl Sink {
    SinkTreeline { options }
}

struct SinkTreeline {
    options: SinkTreelineOptions,
}

impl Sink for SinkTreeline {
    fn name(&self) -> &str {
        "Treeline"
    }

    fn write(&self, transactions: &[Transaction]) -> Result<()> {
        let context = utils::get_context()
            .into_rootcause()
            .context("Failed to create Treeline client")?;

        let accounts = context
            .repository
            .get_accounts()
            .into_rootcause()
            .context("Failed to retrieve accounts")?;

        let account = accounts
            .into_iter()
            .find(|acc| acc.name == self.options.account_name)
            .context_with(|| {
                format!(
                    "Account with name '{}' not found",
                    self.options.account_name
                )
            })?;

        info!("Account found: [{}] with id [{}]", account.name, account.id);

        info!("Removing existing data");
        clean_account(&context, account.id)
            .context_with(|| format!("Failed to clean account [{}]", account.id))?;

        info!("Adding new transactions");
        add_transactions(&context, account.id, transactions)
            .context_with(|| format!("Failed to add transactions to account [{}]", account.id))?;

        info!("Regenerating balances");
        regenerate_balances(&context, account.id).context_with(|| {
            format!("Failed to regenerate balances for account [{}]", account.id)
        })?;

        info!("Treeline sink completed successfully");
        Ok(())
    }
}
