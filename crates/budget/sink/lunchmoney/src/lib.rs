mod tags;

use finance_as_code_api_lunchmoney::api::LunchMoneyApi;
pub use finance_as_code_api_lunchmoney::category_hierarchy_service::CategoryHierarchyItem;
pub use finance_as_code_api_lunchmoney::category_hierarchy_service::ChildCategoryHierarchyItem;
use finance_as_code_api_lunchmoney::category_hierarchy_service::{
    DefaultLunchMoneyCategoryHierarchyCreationService, LunchMoneyCategoryHierarchyCreationService,
};
use finance_as_code_api_lunchmoney::category_service::{
    CategoryNameToIdMaps, DefaultLunchMoneyCategoriesService, LunchMoneyCategoriesService,
};
use finance_as_code_api_lunchmoney::deletion_service::{
    DefaultLunchMoneyTransactionsDeletionService, LunchMoneyTransactionsDeletionService,
};
use finance_as_code_api_lunchmoney::dto::InsertTransactionDto;
use finance_as_code_api_lunchmoney::upload_service::{
    DefaultLunchMoneyTransactionsUploadService, LunchMoneyTransactionsUploadService,
};
use finance_as_code_budget_core::Transaction;
use finance_as_code_budget_core::setup::Setup;
use finance_as_code_budget_core::sink::Sink;
use log::info;
use rootcause::Result;
use rootcause::option_ext::OptionExt;
use rootcause::prelude::ResultExt;
use std::collections::{BTreeSet, HashMap};

pub use tags::LunchMoneyTags;

#[derive(Clone, Debug)]
pub struct LunchMoneyApiKey(String);

impl LunchMoneyApiKey {
    pub fn new(key: String) -> Self {
        Self(key)
    }

    pub(crate) fn value(&self) -> &str {
        &self.0
    }
}

impl From<String> for LunchMoneyApiKey {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for LunchMoneyApiKey {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Clone, Debug)]
pub struct LunchMoneyAccountName(String);

impl LunchMoneyAccountName {
    pub fn new(name: String) -> Self {
        Self(name)
    }

    pub(crate) fn value(&self) -> &str {
        &self.0
    }
}

impl From<String> for LunchMoneyAccountName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for LunchMoneyAccountName {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(bon::Builder, Clone, Debug)]
pub struct LunchMoneySinkConfig {
    #[builder(into)]
    /// Lunch Money API key generated in the Lunch Money app <https://my.lunchmoney.app/developers>
    pub(crate) api_key: LunchMoneyApiKey,
    #[builder(into)]
    /// Account name in Lunch Money. Can be found in [`Setup -> Accounts`](https://my.lunchmoney.app/accounts)
    pub(crate) account_name: LunchMoneyAccountName,
}

#[derive(bon::Builder, Clone, Debug)]
pub struct LunchMoneyCategorySetupConfig {
    #[builder(into)]
    /// Lunch Money API key generated in the Lunch Money app <https://my.lunchmoney.app/developers>
    pub(crate) api_key: LunchMoneyApiKey,
    pub(crate) categories: Vec<CategoryHierarchyItem>,
}

pub struct LunchMoneySink {
    config: LunchMoneySinkConfig,
    categories_service: Box<dyn LunchMoneyCategoriesService>,
    transactions_deletion_service: Box<dyn LunchMoneyTransactionsDeletionService>,
    transactions_upload_service: Box<dyn LunchMoneyTransactionsUploadService>,
}

pub struct LunchMoneyCategorySetup {
    config: LunchMoneyCategorySetupConfig,
    category_hierarchy_creation_service: Box<dyn LunchMoneyCategoryHierarchyCreationService>,
}

pub fn create_lunchmoney_sink(config: LunchMoneySinkConfig) -> impl Sink {
    LunchMoneySink {
        config,
        categories_service: Box::new(DefaultLunchMoneyCategoriesService),
        transactions_deletion_service: Box::new(DefaultLunchMoneyTransactionsDeletionService),
        transactions_upload_service: Box::new(DefaultLunchMoneyTransactionsUploadService),
    }
}

/// [Setup] that replaces Lunch Money category hierarchy with the requested
/// structure.
pub fn create_lunchmoney_category_setup(config: LunchMoneyCategorySetupConfig) -> impl Setup {
    LunchMoneyCategorySetup {
        config,
        category_hierarchy_creation_service: Box::new(
            DefaultLunchMoneyCategoryHierarchyCreationService,
        ),
    }
}

impl Sink for LunchMoneySink {
    fn name(&self) -> &str {
        "Lunch Money"
    }

    fn write(&self, transactions: &[Transaction]) -> Result<()> {
        info!("Creating Lunch Money API client");

        let client = finance_as_code_api_lunchmoney::api::LunchMoneyClient::new(
            "https://api.lunchmoney.dev/v2".to_string(),
            self.config.api_key.value().into(),
        );

        info!(
            "Retrieving account ID for account name '{}'",
            self.config.account_name.value()
        );
        let account_id = Self::get_account_id_for_account_name(&self.config.account_name, &client)
            .context("failed to get account ID")?;
        info!(
            "Account ID for account name '{}' is {}",
            self.config.account_name.value(),
            account_id
        );

        let transaction_category_names = get_all_transaction_category_names(transactions);
        info!(
            "Resolving {} distinct Lunch Money category names from transactions",
            transaction_category_names.len()
        );

        let category_name_to_id_maps = self
            .categories_service
            .get_category_name_to_id_map(&client)
            .context("failed to get Lunch Money category name to id map")?;

        let transaction_category_name_to_id =
            map_category_names_to_ids(&transaction_category_names, &category_name_to_id_maps)
                .context("failed to map transaction category names to ids")?;

        info!(
            "Getting all existing transactions for account '{}'",
            self.config.account_name.value()
        );
        let all_transactions = client
            .get_all_transactions(
                &finance_as_code_api_lunchmoney::dto::GetTransactionsParams {
                    manual_account_id: Some(account_id),
                    limit: Some(500),
                    ..Default::default()
                },
            )
            .context("failed to get existing transactions for account")?;

        info!(
            "Deleting {} existing transactions for account '{}'",
            all_transactions.len(),
            self.config.account_name.value()
        );

        self.transactions_deletion_service
            .delete_transactions(&client, self.config.account_name.value(), &all_transactions)
            .context("failed to delete existing transactions")?;

        let insert_transactions: Vec<_> = transactions
            .iter()
            .map(|transaction| {
                to_insert_transaction(transaction, account_id, &transaction_category_name_to_id)
            })
            .collect::<Result<Vec<_>>>()
            .context("failed to convert transactions to Lunch Money DTOs")?;

        info!(
            "Uploading {} transactions to account '{}'",
            insert_transactions.len(),
            self.config.account_name.value()
        );

        self.transactions_upload_service
            .upload_transactions(
                &client,
                self.config.account_name.value(),
                &insert_transactions,
            )
            .context("failed to upload transactions to Lunch Money")?;

        Ok(())
    }
}

impl Setup for LunchMoneyCategorySetup {
    fn name(&self) -> &str {
        "Lunch Money Category Setup"
    }

    fn run(&self) -> Result<()> {
        info!("Creating Lunch Money API client for category setup");

        let client = finance_as_code_api_lunchmoney::api::LunchMoneyClient::new(
            "https://api.lunchmoney.dev/v2".to_string(),
            self.config.api_key.value().into(),
        );

        info!(
            "Replacing Lunch Money category hierarchy with {} top-level categories",
            self.config.categories.len()
        );

        self.category_hierarchy_creation_service
            .replace_category_hierarchy(&client, &self.config.categories)
            .context("failed to replace Lunch Money category hierarchy")?;

        Ok(())
    }
}

impl LunchMoneySink {
    fn get_account_id_for_account_name(
        account_name: &LunchMoneyAccountName,
        api_client: &impl LunchMoneyApi,
    ) -> Result<i64> {
        let manual_accounts = api_client
            .get_all_manual_accounts()
            .context("Failed to retrieve Lunch Money manual accounts")?;

        Ok(manual_accounts
            .into_iter()
            .find(|manual_account| manual_account.name == account_name.value())
            .map(|manual_account| manual_account.id)
            .context_with(|| format!("Account with name '{}' not found", account_name.value()))?)
    }
}

fn to_insert_transaction(
    transaction: &Transaction,
    account_id: i64,
    category_name_to_id: &HashMap<String, i64>,
) -> Result<InsertTransactionDto> {
    let category_id = transaction
        .tags
        .get_category_name()
        .map(|category_name| {
            category_name_to_id
                .get(category_name)
                .copied()
                .context_with(|| {
                    format!(
                        "Lunch Money category '{}' for transaction '{}' not found",
                        category_name, transaction.id
                    )
                })
        })
        .transpose()?;

    Ok(InsertTransactionDto {
        date: transaction.date,
        amount: -*transaction.amount.amount(),
        currency: Some(transaction.amount.currency().iso_alpha_code.to_lowercase()),
        category_id,
        notes: Some(transaction.description.clone()),
        payee: Some(transaction.counterparty.clone()),
        manual_account_id: Some(account_id),
        external_id: Some(transaction.id.to_string()),
    })
}

fn get_all_transaction_category_names(transactions: &[Transaction]) -> BTreeSet<String> {
    transactions
        .iter()
        .filter_map(|transaction| transaction.tags.get_category_name().cloned())
        .collect()
}

fn map_category_names_to_ids(
    category_names: &BTreeSet<String>,
    category_name_to_id_maps: &CategoryNameToIdMaps,
) -> Result<HashMap<String, i64>> {
    let non_assignable_category_names: Vec<String> = category_names
        .iter()
        .filter(|category_name| {
            category_name_to_id_maps
                .non_assignable
                .contains_key(category_name.as_str())
        })
        .cloned()
        .collect();

    if !non_assignable_category_names.is_empty() {
        let non_assignable_categories = non_assignable_category_names
            .iter()
            .map(|category_name| format!("'{}'", category_name))
            .collect::<Vec<_>>()
            .join(", ");

        rootcause::bail!(
            "Lunch Money category {} is a category group and cannot be used as transaction category. Use child categories instead.",
            non_assignable_categories
        );
    }

    let unknown_category_names: Vec<String> = category_names
        .iter()
        .filter(|category_name| {
            !category_name_to_id_maps
                .assignable
                .contains_key(category_name.as_str())
                && !category_name_to_id_maps
                    .non_assignable
                    .contains_key(category_name.as_str())
        })
        .cloned()
        .collect();

    if !unknown_category_names.is_empty() {
        rootcause::bail!(
            "Unknown Lunch Money category names in transactions: {}",
            unknown_category_names.join(", ")
        );
    }

    Ok(category_names
        .iter()
        .filter_map(|category_name| {
            category_name_to_id_maps
                .assignable
                .get(category_name)
                .copied()
                .map(|category_id| (category_name.clone(), category_id))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finance_as_code_api_lunchmoney::api::MockLunchMoneyApi;
    use finance_as_code_api_lunchmoney::category_hierarchy_service::MockLunchMoneyCategoryHierarchyCreationService;
    use finance_as_code_api_lunchmoney::dto::ManualAccountDto;
    use googletest::prelude::*;
    use std::collections::{BTreeSet, HashMap};

    fn sample_categories() -> Vec<CategoryHierarchyItem> {
        vec![CategoryHierarchyItem {
            name: "Transport".to_string(),
            description: Some("Transportation costs".to_string()),
            is_income: Some(false),
            exclude_from_budget: Some(false),
            exclude_from_totals: Some(false),
            children: vec![ChildCategoryHierarchyItem {
                name: "Fuel".to_string(),
                description: None,
            }],
        }]
    }

    #[test]
    fn get_account_id_for_account_name_returns_account_id() {
        let mut api_client = MockLunchMoneyApi::new();
        api_client
            .expect_get_all_manual_accounts()
            .times(1)
            .return_once(|| {
                Ok(vec![
                    ManualAccountDto {
                        id: 1,
                        name: "Cash Wallet".to_string(),
                    },
                    ManualAccountDto {
                        id: 2,
                        name: "Savings Jar".to_string(),
                    },
                ])
            });

        let account_id = LunchMoneySink::get_account_id_for_account_name(
            &LunchMoneyAccountName::from("Savings Jar"),
            &api_client,
        )
        .unwrap();

        assert_that!(account_id, eq(2));
    }

    #[test]
    fn get_account_id_for_account_name_returns_error_when_account_is_missing() {
        let mut api_client = MockLunchMoneyApi::new();
        api_client
            .expect_get_all_manual_accounts()
            .times(1)
            .return_once(|| {
                Ok(vec![ManualAccountDto {
                    id: 1,
                    name: "Cash Wallet".to_string(),
                }])
            });

        let error = LunchMoneySink::get_account_id_for_account_name(
            &LunchMoneyAccountName::from("Savings Jar"),
            &api_client,
        )
        .unwrap_err();

        assert_that!(
            error.to_string(),
            contains_substring("Account with name 'Savings Jar' not found")
        );
    }

    #[test]
    fn map_category_names_to_ids_returns_ids_for_all_requested_names() {
        let category_names = BTreeSet::from(["Fuel".to_string(), "Rent".to_string()]);
        let category_name_to_id_maps = CategoryNameToIdMaps {
            assignable: HashMap::from([
                ("Fuel".to_string(), 315174),
                ("Rent".to_string(), 83),
                ("Groceries".to_string(), 999),
            ]),
            non_assignable: HashMap::from([("Automobile".to_string(), 86)]),
        };

        let mapped = map_category_names_to_ids(&category_names, &category_name_to_id_maps).unwrap();

        assert_that!(
            mapped,
            eq(&HashMap::from([
                ("Fuel".to_string(), 315174),
                ("Rent".to_string(), 83),
            ]))
        );
    }

    #[test]
    fn map_category_names_to_ids_returns_error_when_name_is_missing() {
        let category_names = BTreeSet::from(["Fuel".to_string(), "Nonexistent".to_string()]);
        let category_name_to_id_maps = CategoryNameToIdMaps {
            assignable: HashMap::from([("Fuel".to_string(), 315174)]),
            non_assignable: HashMap::new(),
        };

        let error =
            map_category_names_to_ids(&category_names, &category_name_to_id_maps).unwrap_err();

        assert_that!(
            error.to_string(),
            contains_substring("Unknown Lunch Money category names in transactions: Nonexistent")
        );
    }

    #[test]
    fn map_category_names_to_ids_returns_error_when_name_is_non_assignable_group() {
        let category_names = BTreeSet::from(["Automobile".to_string()]);
        let category_name_to_id_maps = CategoryNameToIdMaps {
            assignable: HashMap::from([("Fuel".to_string(), 315174)]),
            non_assignable: HashMap::from([("Automobile".to_string(), 86)]),
        };

        let error =
            map_category_names_to_ids(&category_names, &category_name_to_id_maps).unwrap_err();

        assert_that!(
            error.to_string(),
            contains_substring(
                "Lunch Money category 'Automobile' is a category group and cannot be used as transaction category"
            )
        );
    }

    #[test]
    fn lunchmoney_category_setup_replaces_category_hierarchy() {
        let categories = sample_categories();
        let mut mock_service = MockLunchMoneyCategoryHierarchyCreationService::new();
        let expected_categories = categories.clone();

        mock_service
            .expect_replace_category_hierarchy()
            .times(1)
            .withf(move |_, actual_categories| actual_categories == expected_categories.as_slice())
            .return_once(|_, _| Ok(()));

        let setup = LunchMoneyCategorySetup {
            config: LunchMoneyCategorySetupConfig {
                api_key: LunchMoneyApiKey::from("api_key"),
                categories,
            },
            category_hierarchy_creation_service: Box::new(mock_service),
        };

        setup.run().unwrap();
    }

    #[test]
    fn lunchmoney_category_setup_adds_context_when_replacement_fails() {
        let mut mock_service = MockLunchMoneyCategoryHierarchyCreationService::new();

        mock_service
            .expect_replace_category_hierarchy()
            .times(1)
            .return_once(|_, _| rootcause::bail!("replacement failed"));

        let setup = LunchMoneyCategorySetup {
            config: LunchMoneyCategorySetupConfig {
                api_key: LunchMoneyApiKey::from("api_key"),
                categories: sample_categories(),
            },
            category_hierarchy_creation_service: Box::new(mock_service),
        };

        let error = setup.run().unwrap_err();
        assert_that!(
            error.to_string(),
            contains_substring("failed to replace Lunch Money category hierarchy")
        );
    }
}
