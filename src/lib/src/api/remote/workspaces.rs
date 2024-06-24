pub mod commits;
pub mod data_frames;
pub mod files;

use std::path::Path;

pub use commits::commit;
pub use commits::commit_file;

use crate::api;
use crate::api::remote::client;
use crate::error::OxenError;
use crate::model::RemoteRepository;
use crate::view::workspaces::{NewWorkspace, WorkspaceResponse};
use crate::view::WorkspaceResponseView;
use crate::view::{RemoteStagedStatus, RemoteStagedStatusResponse};

pub async fn create(
    remote_repo: &RemoteRepository,
    branch_name: impl AsRef<str>,
    workspace_id: impl AsRef<str>,
) -> Result<WorkspaceResponse, OxenError> {
    let branch_name = branch_name.as_ref();
    let workspace_id = workspace_id.as_ref();
    let url = api::endpoint::url_from_repo(remote_repo, "/workspaces")?;
    log::debug!("create workspace {}\n", url);

    let body = serde_json::to_string(&NewWorkspace {
        branch_name: branch_name.to_string(),
        workspace_id: workspace_id.to_string(),
    })?;

    let client = client::new_for_url(&url)?;
    let res = client
        .put(&url)
        .body(reqwest::Body::from(body))
        .send()
        .await?;

    let body = client::parse_json_body(&url, res).await?;
    log::debug!("create workspace got body: {}", body);
    let response: Result<WorkspaceResponseView, serde_json::Error> = serde_json::from_str(&body);
    match response {
        Ok(val) => Ok(val.workspace),
        Err(err) => Err(OxenError::basic_str(format!(
            "error parsing response from {url}\n\nErr {err:?} \n\n{body}"
        ))),
    }
}

pub async fn status(
    remote_repo: &RemoteRepository,
    workspace_id: &str,
    path: &Path,
    page: usize,
    page_size: usize,
) -> Result<RemoteStagedStatus, OxenError> {
    let path_str = path.to_str().unwrap();
    let uri =
        format!("/workspaces/{workspace_id}/status/{path_str}?page={page}&page_size={page_size}");
    let url = api::endpoint::url_from_repo(remote_repo, &uri)?;
    log::debug!("status url: {url}");

    let client = client::new_for_url(&url)?;
    match client.get(&url).send().await {
        Ok(res) => {
            let body = client::parse_json_body(&url, res).await?;
            log::debug!("status got body: {}", body);
            let response: Result<RemoteStagedStatusResponse, serde_json::Error> =
                serde_json::from_str(&body);
            match response {
                Ok(val) => Ok(val.staged),
                Err(err) => Err(OxenError::basic_str(format!(
                    "api::staging::status error parsing response from {url}\n\nErr {err:?} \n\n{body}"
                ))),
            }
        }
        Err(err) => {
            let err = format!("api::staging::status Request failed: {url}\nErr {err:?}");
            Err(OxenError::basic_str(err))
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    use crate::config::UserConfig;
    use crate::constants::DEFAULT_BRANCH_NAME;
    use crate::error::OxenError;
    use crate::test;
    use crate::{api, command, constants};

    use std::path::Path;

    #[tokio::test]
    async fn test_create_workspace() -> Result<(), OxenError> {
        test::run_remote_repo_test_bounding_box_csv_pushed(|remote_repo| async move {
            let branch_name = "main";
            let workspace_id = "test_workspace_id";
            let workspace = create(&remote_repo, branch_name, workspace_id).await?;

            assert_eq!(workspace.branch_name, branch_name);
            assert_eq!(workspace.workspace_id, workspace_id);

            Ok(remote_repo)
        })
        .await
    }

    #[tokio::test]
    async fn test_list_empty_staging_dir_empty_remote() -> Result<(), OxenError> {
        test::run_empty_remote_repo_test(|mut local_repo, remote_repo| async move {
            let branch_name = "add-images";
            api::local::branches::create_checkout(&local_repo, branch_name)?;
            let remote = test::repo_remote_url_from(&local_repo.dirname());
            command::config::set_remote(&mut local_repo, constants::DEFAULT_REMOTE_NAME, &remote)?;
            command::push(&local_repo).await?;

            // client can decide what to use for id
            let identifier = UserConfig::identifier()?;
            let branch = api::remote::branches::create_from_or_get(
                &remote_repo,
                branch_name,
                DEFAULT_BRANCH_NAME,
            )
            .await?;
            assert_eq!(branch.name, branch_name);

            let workspace =
                api::remote::workspaces::create(&remote_repo, &branch_name, &identifier).await;
            assert!(workspace.is_ok());

            let page_num = constants::DEFAULT_PAGE_NUM;
            let page_size = constants::DEFAULT_PAGE_SIZE;
            let path = Path::new("images");
            let entries = api::remote::workspaces::status(
                &remote_repo,
                &identifier,
                path,
                page_num,
                page_size,
            )
            .await?;
            assert_eq!(entries.added_files.entries.len(), 0);
            assert_eq!(entries.added_files.total_entries, 0);

            Ok(remote_repo)
        })
        .await
    }

    #[tokio::test]
    async fn test_list_empty_staging_dir_all_data_pushed() -> Result<(), OxenError> {
        test::run_remote_repo_test_bounding_box_csv_pushed(|remote_repo| async move {
            let branch_name = "add-images";
            let branch = api::remote::branches::create_from_or_get(
                &remote_repo,
                branch_name,
                DEFAULT_BRANCH_NAME,
            )
            .await?;
            assert_eq!(branch.name, branch_name);

            let workspace_id = UserConfig::identifier()?;
            let workspace =
                api::remote::workspaces::create(&remote_repo, &branch_name, &workspace_id).await?;
            assert_eq!(workspace.workspace_id, workspace_id);

            let page_num = constants::DEFAULT_PAGE_NUM;
            let page_size = constants::DEFAULT_PAGE_SIZE;
            let path = Path::new("images");
            let entries = api::remote::workspaces::status(
                &remote_repo,
                &workspace_id,
                path,
                page_num,
                page_size,
            )
            .await?;
            assert_eq!(entries.added_files.entries.len(), 0);
            assert_eq!(entries.added_files.total_entries, 0);

            Ok(remote_repo)
        })
        .await
    }
}
