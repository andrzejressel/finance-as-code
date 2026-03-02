use crate::api::LunchMoneyApi;
use crate::dto::CreateCategoryRequest;
use log::info;
use rootcause::Result;
use rootcause::prelude::ResultExt;

#[derive(bon::Builder, Clone, Debug, PartialEq)]
pub struct CategoryHierarchyItem {
    #[builder(into)]
    pub name: String,
    #[builder(into)]
    pub description: Option<String>,
    pub is_income: Option<bool>,
    pub exclude_from_budget: Option<bool>,
    pub exclude_from_totals: Option<bool>,
    /// Category becomes a group only when this list is non-empty.
    ///
    /// To keep API simple, this model does not support creating an empty
    /// category group.
    #[builder(default)]
    pub children: Vec<ChildCategoryHierarchyItem>,
}

#[derive(bon::Builder, Clone, Debug, PartialEq)]
pub struct ChildCategoryHierarchyItem {
    #[builder(into)]
    pub name: String,
    #[builder(into)]
    pub description: Option<String>,
}

#[cfg_attr(any(test, feature = "mock"), mockall::automock)]
pub trait LunchMoneyCategoryHierarchyCreationService {
    fn replace_category_hierarchy(
        &self,
        api_client: &dyn LunchMoneyApi,
        categories: &[CategoryHierarchyItem],
    ) -> Result<()>;
}

pub struct DefaultLunchMoneyCategoryHierarchyCreationService;

impl LunchMoneyCategoryHierarchyCreationService
    for DefaultLunchMoneyCategoryHierarchyCreationService
{
    fn replace_category_hierarchy(
        &self,
        api_client: &dyn LunchMoneyApi,
        categories: &[CategoryHierarchyItem],
    ) -> Result<()> {
        info!("Starting Lunch Money category hierarchy replacement");
        let existing_categories = get_existing_categories(api_client)?;

        if category_hierarchy_matches_request(&existing_categories, categories) {
            info!(
                "Existing Lunch Money category hierarchy already matches requested hierarchy, skipping replacement"
            );
            return Ok(());
        }

        remove_existing_categories(api_client, &existing_categories)?;
        create_categories(api_client, categories)?;
        info!(
            "Finished Lunch Money category hierarchy replacement (top_level_count={})",
            categories.len()
        );

        Ok(())
    }
}

fn get_existing_categories(api_client: &dyn LunchMoneyApi) -> Result<Vec<crate::dto::CategoryDto>> {
    info!("Starting to fetch existing Lunch Money categories");
    let existing_categories = api_client
        .get_all_categories()
        .context("failed to fetch existing Lunch Money categories")?;
    info!(
        "Finished fetching existing Lunch Money categories (count={})",
        existing_categories.len()
    );
    Ok(existing_categories)
}

fn category_hierarchy_matches_request(
    existing_categories: &[crate::dto::CategoryDto],
    requested_categories: &[CategoryHierarchyItem],
) -> bool {
    if existing_categories.len() != requested_categories.len() {
        return false;
    }

    let mut existing_top_level_categories =
        existing_categories.iter().enumerate().collect::<Vec<_>>();
    existing_top_level_categories
        .sort_by_key(|(index, category)| (category.order.unwrap_or(*index as i64), *index));

    existing_top_level_categories
        .into_iter()
        .zip(requested_categories)
        .all(|((_, existing_category), requested_category)| {
            category_matches_request(existing_category, requested_category)
        })
}

fn category_matches_request(
    existing_category: &crate::dto::CategoryDto,
    requested_category: &CategoryHierarchyItem,
) -> bool {
    if existing_category.name != requested_category.name
        || existing_category.description != requested_category.description
        || !optional_bool_matches(requested_category.is_income, existing_category.is_income)
        || !optional_bool_matches(
            requested_category.exclude_from_budget,
            existing_category.exclude_from_budget,
        )
        || !optional_bool_matches(
            requested_category.exclude_from_totals,
            existing_category.exclude_from_totals,
        )
        || existing_category.is_group == requested_category.children.is_empty()
        || existing_category.group_id.is_some()
        || existing_category.children.len() != requested_category.children.len()
    {
        return false;
    }

    let mut existing_children = existing_category
        .children
        .iter()
        .enumerate()
        .collect::<Vec<_>>();
    existing_children.sort_by_key(|(index, child)| (child.order.unwrap_or(*index as i64), *index));

    existing_children
        .into_iter()
        .zip(&requested_category.children)
        .all(|((_, existing_child), requested_child)| {
            existing_child.name == requested_child.name
                && existing_child.description == requested_child.description
                && !existing_child.is_group
                && existing_child.group_id == Some(existing_category.id)
        })
}

fn optional_bool_matches(requested: Option<bool>, existing: bool) -> bool {
    requested.is_none_or(|requested_value| requested_value == existing)
}

fn remove_existing_categories(
    api_client: &dyn LunchMoneyApi,
    existing_categories: &[crate::dto::CategoryDto],
) -> Result<()> {
    info!(
        "Starting to remove existing Lunch Money categories (count={})",
        existing_categories.len()
    );
    for existing_category in existing_categories {
        info!(
            "Removing existing Lunch Money category '{}' (id={})",
            existing_category.name, existing_category.id
        );
        api_client
            .delete_category(existing_category.id)
            .context_with(|| {
                format!(
                    "failed to delete existing Lunch Money category '{}' (id={})",
                    existing_category.name, existing_category.id
                )
            })?;
        info!(
            "Finished removing existing Lunch Money category '{}' (id={})",
            existing_category.name, existing_category.id
        );
    }

    info!(
        "Finished removing existing Lunch Money categories (count={})",
        existing_categories.len()
    );

    Ok(())
}

fn create_categories(
    api_client: &dyn LunchMoneyApi,
    categories: &[CategoryHierarchyItem],
) -> Result<()> {
    info!(
        "Starting to create requested Lunch Money categories (count={})",
        categories.len()
    );
    for (order, category) in categories.iter().enumerate() {
        create_parent_category(api_client, category, order as i64).context_with(|| {
            format!(
                "failed to create Lunch Money category hierarchy item '{}'",
                category.name
            )
        })?;
    }

    info!(
        "Finished creating requested Lunch Money categories (count={})",
        categories.len()
    );

    Ok(())
}

fn create_parent_category(
    api_client: &dyn LunchMoneyApi,
    category: &CategoryHierarchyItem,
    order: i64,
) -> Result<()> {
    info!(
        "Starting to create top-level Lunch Money category '{}' (order={})",
        category.name, order
    );
    let is_group = !category.children.is_empty();

    let created_category = api_client
        .create_category(&CreateCategoryRequest {
            name: category.name.clone(),
            description: category.description.clone(),
            is_income: category.is_income,
            exclude_from_budget: category.exclude_from_budget,
            exclude_from_totals: category.exclude_from_totals,
            is_group: Some(is_group),
            group_id: None,
            order: Some(order),
        })
        .context_with(|| format!("failed to create Lunch Money category '{}'", category.name))?;

    info!(
        "Created Lunch Money category '{}' (id={})",
        category.name, created_category.id
    );

    for (child_order, child) in category.children.iter().enumerate() {
        create_child_category(api_client, child, created_category.id, child_order as i64)
            .context_with(|| {
                format!(
                    "failed to create child category '{}' under parent '{}' (id={})",
                    child.name, category.name, created_category.id
                )
            })?;
    }

    info!(
        "Finished creating top-level Lunch Money category '{}'",
        category.name
    );

    Ok(())
}

fn create_child_category(
    api_client: &dyn LunchMoneyApi,
    child: &ChildCategoryHierarchyItem,
    parent_id: i64,
    order: i64,
) -> Result<()> {
    info!(
        "Starting to create child Lunch Money category '{}' (parent_id={}, order={})",
        child.name, parent_id, order
    );
    api_client
        .create_category(&CreateCategoryRequest {
            name: child.name.clone(),
            description: child.description.clone(),
            is_income: None,
            exclude_from_budget: None,
            exclude_from_totals: None,
            is_group: Some(false),
            group_id: Some(parent_id),
            order: Some(order),
        })
        .context_with(|| {
            format!(
                "failed to create Lunch Money child category '{}'",
                child.name
            )
        })?;

    info!(
        "Finished creating child Lunch Money category '{}' (parent_id={})",
        child.name, parent_id
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::MockLunchMoneyApi;
    use crate::dto::{CategoryDto, ChildCategoryDto};
    use chrono::{DateTime, FixedOffset};
    use googletest::prelude::*;
    use mockall::Sequence;

    fn dt(value: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(value).unwrap()
    }

    fn category_dto(id: i64, name: &str, is_group: bool, group_id: Option<i64>) -> CategoryDto {
        CategoryDto {
            id,
            name: name.to_string(),
            description: None,
            is_income: false,
            exclude_from_budget: false,
            exclude_from_totals: false,
            updated_at: dt("2025-02-28T09:49:03.238Z"),
            created_at: dt("2025-01-28T09:49:03.238Z"),
            is_group,
            group_id,
            children: vec![],
            archived: false,
            archived_at: None,
            order: None,
            collapsed: false,
        }
    }

    fn child_category_dto(
        id: i64,
        name: &str,
        description: Option<&str>,
        parent_id: i64,
        order: i64,
    ) -> ChildCategoryDto {
        ChildCategoryDto {
            id,
            name: name.to_string(),
            description: description.map(ToString::to_string),
            is_income: false,
            exclude_from_budget: false,
            exclude_from_totals: false,
            updated_at: dt("2025-02-28T09:49:03.238Z"),
            created_at: dt("2025-01-28T09:49:03.238Z"),
            is_group: false,
            group_id: Some(parent_id),
            archived: false,
            archived_at: None,
            order: Some(order),
            collapsed: false,
        }
    }

    #[test]
    fn replace_category_hierarchy_skips_changes_when_existing_hierarchy_matches_request() {
        let mut api_client = MockLunchMoneyApi::new();

        api_client
            .expect_get_all_categories()
            .times(1)
            .return_once(|| {
                let mut transport = category_dto(10, "Transport", true, None);
                transport.description = Some("Transportation costs".to_string());
                transport.order = Some(0);
                transport.children = vec![
                    child_category_dto(11, "Fuel", None, 10, 0),
                    child_category_dto(12, "Maintenance", None, 10, 1),
                ];

                let mut rent = category_dto(13, "Rent", false, None);
                rent.order = Some(1);

                Ok(vec![transport, rent])
            });

        let service = DefaultLunchMoneyCategoryHierarchyCreationService;
        let categories = vec![
            CategoryHierarchyItem {
                name: "Transport".to_string(),
                description: Some("Transportation costs".to_string()),
                is_income: Some(false),
                exclude_from_budget: Some(false),
                exclude_from_totals: Some(false),
                children: vec![
                    ChildCategoryHierarchyItem {
                        name: "Fuel".to_string(),
                        description: None,
                    },
                    ChildCategoryHierarchyItem {
                        name: "Maintenance".to_string(),
                        description: None,
                    },
                ],
            },
            CategoryHierarchyItem {
                name: "Rent".to_string(),
                description: None,
                is_income: Some(false),
                exclude_from_budget: Some(false),
                exclude_from_totals: Some(false),
                children: vec![],
            },
        ];

        service
            .replace_category_hierarchy(&api_client, &categories)
            .unwrap();
    }

    #[test]
    fn replace_category_hierarchy_deletes_existing_then_creates_parent_and_children_with_order() {
        let mut api_client = MockLunchMoneyApi::new();
        let mut sequence = Sequence::new();

        api_client
            .expect_get_all_categories()
            .times(1)
            .in_sequence(&mut sequence)
            .return_once(|| {
                Ok(vec![
                    category_dto(10, "Old Group", true, None),
                    category_dto(11, "Old Category", false, None),
                ])
            });

        api_client
            .expect_delete_category()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(|id| *id == 10)
            .return_once(|_| Ok(()));

        api_client
            .expect_delete_category()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(|id| *id == 11)
            .return_once(|_| Ok(()));

        api_client
            .expect_create_category()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(|request| {
                request.name == "Transport"
                    && request.is_group == Some(true)
                    && request.group_id.is_none()
                    && request.order == Some(0)
            })
            .return_once(|_| Ok(category_dto(86, "Transport", true, None)));

        api_client
            .expect_create_category()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(|request| {
                request.name == "Fuel"
                    && request.group_id == Some(86)
                    && request.is_group == Some(false)
                    && request.order == Some(0)
            })
            .return_once(|_| Ok(category_dto(315174, "Fuel", false, Some(86))));

        api_client
            .expect_create_category()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(|request| {
                request.name == "Maintenance"
                    && request.group_id == Some(86)
                    && request.is_group == Some(false)
                    && request.order == Some(1)
            })
            .return_once(|_| Ok(category_dto(315175, "Maintenance", false, Some(86))));

        api_client
            .expect_create_category()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(|request| {
                request.name == "Rent"
                    && request.group_id.is_none()
                    && request.is_group == Some(false)
                    && request.order == Some(1)
            })
            .return_once(|_| Ok(category_dto(83, "Rent", false, None)));

        let service = DefaultLunchMoneyCategoryHierarchyCreationService;
        let categories = vec![
            CategoryHierarchyItem {
                name: "Transport".to_string(),
                description: Some("Transportation costs".to_string()),
                is_income: Some(false),
                exclude_from_budget: Some(false),
                exclude_from_totals: Some(false),
                children: vec![
                    ChildCategoryHierarchyItem {
                        name: "Fuel".to_string(),
                        description: None,
                    },
                    ChildCategoryHierarchyItem {
                        name: "Maintenance".to_string(),
                        description: None,
                    },
                ],
            },
            CategoryHierarchyItem {
                name: "Rent".to_string(),
                description: None,
                is_income: Some(false),
                exclude_from_budget: Some(false),
                exclude_from_totals: Some(false),
                children: vec![],
            },
        ];

        service
            .replace_category_hierarchy(&api_client, &categories)
            .unwrap();
    }

    #[test]
    fn replace_category_hierarchy_returns_error_with_category_name_when_delete_fails() {
        let mut api_client = MockLunchMoneyApi::new();
        let mut sequence = Sequence::new();

        api_client
            .expect_get_all_categories()
            .times(1)
            .in_sequence(&mut sequence)
            .return_once(|| Ok(vec![category_dto(10, "Old Group", true, None)]));

        api_client
            .expect_delete_category()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(|id| *id == 10)
            .return_once(|_| rootcause::bail!("delete failed"));

        let service = DefaultLunchMoneyCategoryHierarchyCreationService;
        let error = service
            .replace_category_hierarchy(&api_client, &[])
            .unwrap_err();

        assert_that!(
            error.to_string(),
            contains_substring(
                "failed to delete existing Lunch Money category 'Old Group' (id=10)"
            )
        );
    }
}
