use crate::api::LunchMoneyApi;
use log::info;
use rootcause::Result;
use rootcause::prelude::ResultExt;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub struct CategoryNameToIdMaps {
    pub assignable: HashMap<String, i64>,
    pub non_assignable: HashMap<String, i64>,
}

#[cfg_attr(any(test, feature = "mock"), mockall::automock)]
pub trait LunchMoneyCategoriesService {
    fn get_category_name_to_id_map(
        &self,
        api_client: &dyn LunchMoneyApi,
    ) -> Result<CategoryNameToIdMaps>;
}

pub struct DefaultLunchMoneyCategoriesService;

impl LunchMoneyCategoriesService for DefaultLunchMoneyCategoriesService {
    fn get_category_name_to_id_map(
        &self,
        api_client: &dyn LunchMoneyApi,
    ) -> Result<CategoryNameToIdMaps> {
        info!("Fetching Lunch Money categories");

        let categories = api_client
            .get_all_categories()
            .context("failed to fetch Lunch Money categories")?;

        let mut assignable_category_name_to_id = HashMap::new();
        let mut non_assignable_category_name_to_id = HashMap::new();
        let mut existing_category_names = HashSet::new();

        for category in categories {
            if category.children.is_empty() {
                insert_unique_category_name(
                    &mut assignable_category_name_to_id,
                    &mut existing_category_names,
                    category.name,
                    category.id,
                )?;
            } else {
                insert_unique_category_name(
                    &mut non_assignable_category_name_to_id,
                    &mut existing_category_names,
                    category.name,
                    category.id,
                )?;

                for child in category.children {
                    insert_unique_category_name(
                        &mut assignable_category_name_to_id,
                        &mut existing_category_names,
                        child.name,
                        child.id,
                    )?;
                }
            }
        }

        Ok(CategoryNameToIdMaps {
            assignable: assignable_category_name_to_id,
            non_assignable: non_assignable_category_name_to_id,
        })
    }
}

fn insert_unique_category_name(
    destination_map: &mut HashMap<String, i64>,
    existing_category_names: &mut HashSet<String>,
    category_name: String,
    category_id: i64,
) -> Result<()> {
    if existing_category_names.contains(&category_name) {
        rootcause::bail!("duplicate Lunch Money category name '{}'", category_name);
    }

    existing_category_names.insert(category_name.clone());
    destination_map.insert(category_name, category_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::MockLunchMoneyApi;
    use crate::dto::{CategoryDto, ChildCategoryDto};
    use chrono::{DateTime, FixedOffset};
    use googletest::prelude::*;

    fn dt(value: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(value).unwrap()
    }

    #[test]
    fn get_category_name_to_id_map_splits_assignable_and_non_assignable_categories() {
        let mut api_client = MockLunchMoneyApi::new();
        api_client
            .expect_get_all_categories()
            .times(1)
            .return_once(|| {
                Ok(vec![
                    CategoryDto {
                        id: 86,
                        name: "Automobile".to_string(),
                        description: Some("Auto related categories".to_string()),
                        is_income: false,
                        exclude_from_budget: false,
                        exclude_from_totals: false,
                        updated_at: dt("2025-02-28T09:49:03.238Z"),
                        created_at: dt("2025-01-28T09:49:03.238Z"),
                        is_group: true,
                        group_id: None,
                        children: vec![ChildCategoryDto {
                            id: 315174,
                            name: "Fuel".to_string(),
                            description: Some("Fuel and gas expenses".to_string()),
                            is_income: false,
                            exclude_from_budget: false,
                            exclude_from_totals: false,
                            updated_at: dt("2025-02-28T09:49:03.238Z"),
                            created_at: dt("2025-01-28T09:49:03.238Z"),
                            is_group: false,
                            group_id: Some(86),
                            archived: false,
                            archived_at: None,
                            order: Some(1),
                            collapsed: false,
                        }],
                        archived: false,
                        archived_at: None,
                        order: Some(2),
                        collapsed: false,
                    },
                    CategoryDto {
                        id: 83,
                        name: "Rent".to_string(),
                        description: Some("Monthly Rent".to_string()),
                        is_income: false,
                        exclude_from_budget: false,
                        exclude_from_totals: false,
                        updated_at: dt("2025-02-28T09:49:03.225Z"),
                        created_at: dt("2025-01-28T09:49:03.225Z"),
                        is_group: false,
                        group_id: None,
                        children: vec![],
                        archived: false,
                        archived_at: None,
                        order: Some(0),
                        collapsed: false,
                    },
                ])
            });

        let service = DefaultLunchMoneyCategoriesService;
        let category_maps = service.get_category_name_to_id_map(&api_client).unwrap();

        assert_that!(
            category_maps.assignable,
            eq(&HashMap::from([
                ("Fuel".to_string(), 315174),
                ("Rent".to_string(), 83),
            ]))
        );
        assert_that!(
            category_maps.non_assignable,
            eq(&HashMap::from([("Automobile".to_string(), 86)]))
        );
    }

    #[test]
    fn get_category_name_to_id_map_returns_error_when_api_fails() {
        let mut api_client = MockLunchMoneyApi::new();
        api_client
            .expect_get_all_categories()
            .times(1)
            .return_once(|| rootcause::bail!("request failed"));

        let service = DefaultLunchMoneyCategoriesService;
        let error = service
            .get_category_name_to_id_map(&api_client)
            .unwrap_err();

        assert_that!(
            error.to_string(),
            contains_substring("failed to fetch Lunch Money categories")
        );
    }

    #[test]
    fn get_category_name_to_id_map_returns_error_when_category_names_are_duplicated() {
        let mut api_client = MockLunchMoneyApi::new();
        api_client
            .expect_get_all_categories()
            .times(1)
            .return_once(|| {
                Ok(vec![
                    CategoryDto {
                        id: 83,
                        name: "Rent".to_string(),
                        description: Some("Monthly Rent".to_string()),
                        is_income: false,
                        exclude_from_budget: false,
                        exclude_from_totals: false,
                        updated_at: dt("2025-02-28T09:49:03.225Z"),
                        created_at: dt("2025-01-28T09:49:03.225Z"),
                        is_group: false,
                        group_id: None,
                        children: vec![],
                        archived: false,
                        archived_at: None,
                        order: Some(0),
                        collapsed: false,
                    },
                    CategoryDto {
                        id: 84,
                        name: "Rent".to_string(),
                        description: Some("Second Rent".to_string()),
                        is_income: false,
                        exclude_from_budget: false,
                        exclude_from_totals: false,
                        updated_at: dt("2025-02-28T09:49:03.225Z"),
                        created_at: dt("2025-01-28T09:49:03.225Z"),
                        is_group: false,
                        group_id: None,
                        children: vec![],
                        archived: false,
                        archived_at: None,
                        order: Some(1),
                        collapsed: false,
                    },
                ])
            });

        let service = DefaultLunchMoneyCategoriesService;
        let error = service
            .get_category_name_to_id_map(&api_client)
            .unwrap_err();

        assert_that!(
            error.to_string(),
            contains_substring("duplicate Lunch Money category name 'Rent'")
        );
    }
}
