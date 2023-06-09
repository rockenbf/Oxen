use std::path::Path;

use polars::prelude::DataFrame;

use crate::api;
use crate::error::OxenError;
use crate::model::RemoteRepository;
use crate::opts::DFOpts;
use crate::view::json_data_frame::JsonDataSize;
use crate::view::JsonDataFrameSliceResponse;

use super::client;

pub async fn show(
    remote_repo: &RemoteRepository,
    commit_or_branch: &str,
    path: impl AsRef<Path>,
    opts: DFOpts,
) -> Result<(DataFrame, JsonDataSize), OxenError> {
    let path_str = path.as_ref().to_str().unwrap();
    let query_str = opts.to_http_query_params();
    let uri = format!("/df/{commit_or_branch}/{path_str}?{query_str}");
    let url = api::endpoint::url_from_repo(remote_repo, &uri)?;

    let client = client::new_for_url(&url)?;
    match client.get(&url).send().await {
        Ok(res) => {
            let body = client::parse_json_body(&url, res).await?;
            log::debug!("got body: {}", body);
            let response: Result<JsonDataFrameSliceResponse, serde_json::Error> =
                serde_json::from_str(&body);
            match response {
                Ok(val) => {
                    log::debug!("got JsonDataFrameSliceResponse: {:?}", val);
                    let df = val.df.to_df();
                    Ok((df, val.full_size))
                }
                Err(err) => Err(OxenError::basic_str(format!(
                    "error parsing response from {url}\n\nErr {err:?} \n\n{body}"
                ))),
            }
        }
        Err(err) => {
            let err = format!("Request failed: {url}\nErr {err:?}");
            Err(OxenError::basic_str(err))
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::api;
    use crate::command;
    use crate::constants::DEFAULT_BRANCH_NAME;
    use crate::constants::DEFAULT_REMOTE_NAME;
    use crate::error::OxenError;

    use crate::opts::DFOpts;
    use crate::test;
    use crate::util;

    #[tokio::test]
    async fn test_paginate_df() -> Result<(), OxenError> {
        test::run_empty_local_repo_test_async(|mut local_repo| async move {
            let repo_dir = &local_repo.path;
            let large_dir = repo_dir.join("large_files");
            std::fs::create_dir_all(&large_dir)?;
            let csv_file = large_dir.join("test.csv");
            let from_file = test::test_200k_csv();
            util::fs::copy(&from_file, &csv_file)?;

            command::add(&local_repo, &csv_file)?;
            command::commit(&local_repo, "add test.csv")?;

            // Set the proper remote
            let remote = test::repo_remote_url_from(&local_repo.dirname());
            command::config::set_remote(&mut local_repo, DEFAULT_REMOTE_NAME, &remote)?;

            // Create the repo
            let remote_repo = test::create_remote_repo(&local_repo).await?;

            // Push the repo
            command::push(&local_repo).await?;

            // Get the df
            let opts = DFOpts::empty();
            let df = api::remote::df::show(
                &remote_repo,
                DEFAULT_BRANCH_NAME,
                "large_files/test.csv",
                opts,
            )
            .await?;
            assert_eq!(df.1.height, 200_000);
            assert_eq!(df.1.width, 11);

            assert_eq!(df.0.height(), 10);
            assert_eq!(df.0.width(), 11);

            Ok(())
        })
        .await
    }
}
