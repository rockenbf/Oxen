use polars::frame::DataFrame;
use serde::{Deserialize, Serialize};

use crate::{model::{Schema, LocalRepository}, view::{JsonDataFrameView, JsonDataFrame}, opts::DFOpts, error::OxenError};

use super::tabular_compare_summary::TabularCompareSummary;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TabularCompare {
    pub summary: TabularCompareSummary,

    pub schema_left: Option<Schema>,
    pub schema_right: Option<Schema>,

    pub keys: Vec<String>,
    pub targets: Vec<String>,

    // Send the hash column back but don't display it 
    pub match_rows: Option<JsonDataFrame>,
    // pub match_rows_view: Option<JsonDataFrameView>,

    pub diff_rows: Option<JsonDataFrame>,
    // pub diff_rows_view: Option<JsonDataFrameView>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TabularCompareBody {
    pub path_1: String,
    pub path_2: String,
    pub keys: Vec<String>,
    pub targets: Vec<String>,
}

// impl TabularCompare {
//     // TODONOW: get straight to the source, this is duplicative
//     pub fn from_data_frames(
//         repo: &LocalRepository, 
//         df_1: DataFrame,
//         df_2: DataFrame,
//         only_df1: DataFrame,
//         only_df2: DataFrame,
//         different_targets: DataFrame,
//         same_targets: DataFrame,
//         df_opts: DFOpts,
//     ) -> Result<Self, OxenError> {
//         let only_df1_json = JsonDataFrame::from_df_opts(only_df1, df_opts.clone());
//         let only_df2_json = JsonDataFrame::from_df_opts(only_df2, df_opts.clone());
//         let different_targets_json = JsonDataFrame::from_df_opts(different_targets, df_opts.clone());
//         let same_targets_json = JsonDataFrame::from_df_opts(same_targets, df_opts.clone());


//     }
    
// }