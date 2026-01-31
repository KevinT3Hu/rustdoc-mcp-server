use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, JsonSchema)]
pub struct GetDocsArgs {
    pub path: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct SearchDocsArgs {
    pub query: String,
    pub crate_name: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct GetModuleArgs {
    pub path: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct ListCrateItemsArgs {
    pub crate_name: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ListDepsResult {
    pub dependencies: Vec<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct SearchDocsResult {
    pub matches: Vec<ItemSummary>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ItemSummary {
    pub name: String,
    pub kind: String,
}

#[derive(Serialize, JsonSchema)]
pub struct GetModuleResult {
    pub items: Vec<ItemSummary>,
}

#[derive(Serialize, JsonSchema)]
pub struct ListCrateItemsResult {
    pub items: Vec<ItemSummary>,
}
