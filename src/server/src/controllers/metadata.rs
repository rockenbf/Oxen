use crate::errors::OxenHttpError;
use crate::helpers::get_repo;
use crate::params::{app_data, parse_resource, path_param, AggregateQuery};

use liboxen::core;
use liboxen::core::index::CommitEntryReader;
use liboxen::error::OxenError;
use liboxen::model::DataFrameSize;
use liboxen::opts::DFOpts;
use liboxen::view::entry::ResourceVersion;
use liboxen::view::{
    JsonDataFrame, JsonDataFrameSliceResponse, MetadataEntryResponse, StatusMessage,
};
use liboxen::{api, current_function};

use actix_web::{web, HttpRequest, HttpResponse};

pub async fn file(req: HttpRequest) -> actix_web::Result<HttpResponse, OxenHttpError> {
    log::debug!("deleteme metadata file controller");
    let app_data = app_data(&req)?;
    let namespace = path_param(&req, "namespace")?;
    let repo_name = path_param(&req, "repo_name")?;
    let repo = get_repo(&app_data.path, namespace, &repo_name)?;
    let resource = parse_resource(&req, &repo)?;
    
    log::debug!(
        "{} resource {}/{}",
        current_function!(),
        repo_name,
        resource
    );

    // init a commit entry

    // TODONOW remove all this 
    let head = api::local::commits::head_commit(&repo)?;
            
    let commit_entry_reader = CommitEntryReader::new(&repo, &head)?;

    // Try to get the entry from the local repo
    log::debug!("on the remote repo trying to get file path {:?}", &resource.file_path);
    let entry = commit_entry_reader.get_entry(&resource.file_path)?;

    log::debug!("we got the entry from the remote repo: {:?}", entry);

    let latest_commit = api::local::commits::get_by_id(&repo, &resource.commit.id)?.ok_or(
        OxenError::revision_not_found(resource.commit.id.clone().into()),
    )?;

    log::debug!(
        "{} resolve commit {} -> '{}'",
        current_function!(),
        latest_commit.id,
        latest_commit.message
    );

    // Check if annotations/README.md exists in this commit 
    // init a commitentryreader 

    let entry = api::local::entries::get_meta_entry(&repo, &resource.commit, &resource.file_path)?;
    let meta = MetadataEntryResponse {
        status: StatusMessage::resource_found(),
        entry,
    };
    Ok(HttpResponse::Ok().json(meta))
}

pub async fn dir(req: HttpRequest) -> actix_web::Result<HttpResponse, OxenHttpError> {
    let app_data = app_data(&req)?;
    let namespace = path_param(&req, "namespace")?;
    let repo_name = path_param(&req, "repo_name")?;
    let repo = get_repo(&app_data.path, namespace, &repo_name)?;
    let resource = parse_resource(&req, &repo)?;

    log::debug!(
        "{} resource {}/{}",
        current_function!(),
        repo_name,
        resource
    );

    let latest_commit = api::local::commits::get_by_id(&repo, &resource.commit.id)?.ok_or(
        OxenError::revision_not_found(resource.commit.id.clone().into()),
    )?;

    log::debug!(
        "{} resolve commit {} -> '{}'",
        current_function!(),
        latest_commit.id,
        latest_commit.message
    );

    let resource_version = ResourceVersion {
        path: resource.file_path.to_string_lossy().into(),
        version: resource.version().to_owned(),
    };

    let directory = resource.file_path;
    let offset = 0;
    let limit = 100;
    let mut sliced_df =
        core::index::commit_metadata_db::select(&repo, &latest_commit, &directory, offset, limit)?;
    let (num_rows, num_cols) =
        core::index::commit_metadata_db::full_size(&repo, &latest_commit, &directory)?;
    let response = JsonDataFrameSliceResponse {
        status: StatusMessage::resource_found(),
        full_size: DataFrameSize {
            width: num_cols,
            height: num_rows,
        },
        slice_size: DataFrameSize {
            width: num_cols,
            height: num_rows,
        },
        df: JsonDataFrame::from_df(&mut sliced_df),
        commit: Some(resource.commit.clone()),
        resource: Some(resource_version),
        page_number: 0,
        page_size: limit,
        total_pages: 0,
        total_entries: limit,
    };
    Ok(HttpResponse::Ok().json(response))
}

pub async fn agg_dir(
    req: HttpRequest,
    query: web::Query<AggregateQuery>,
) -> actix_web::Result<HttpResponse, OxenHttpError> {
    let app_data = app_data(&req)?;
    let namespace = path_param(&req, "namespace")?;
    let repo_name = path_param(&req, "repo_name")?;
    let repo = get_repo(&app_data.path, namespace, &repo_name)?;
    let resource = parse_resource(&req, &repo)?;

    let column = query.column.clone().ok_or(OxenHttpError::BadRequest(
        "Must supply column query parameter".into(),
    ))?;

    log::debug!(
        "{} resource {}/{}",
        current_function!(),
        repo_name,
        resource
    );

    let latest_commit = api::local::commits::get_by_id(&repo, &resource.commit.id)?.ok_or(
        OxenError::revision_not_found(resource.commit.id.clone().into()),
    )?;

    log::debug!(
        "{} resolve commit {} -> '{}'",
        current_function!(),
        latest_commit.id,
        latest_commit.message
    );

    let directory = &resource.file_path;

    let cached_path = core::cache::cachers::content_stats::dir_column_path(
        &repo,
        &latest_commit,
        directory,
        &column,
    );
    log::debug!("Reading aggregation from cached path: {:?}", cached_path);

    if cached_path.exists() {
        let mut df = core::df::tabular::read_df(&cached_path, DFOpts::empty())?;

        let resource_version = ResourceVersion {
            path: resource.file_path.to_string_lossy().into(),
            version: resource.version().to_owned(),
        };

        let response = JsonDataFrameSliceResponse {
            status: StatusMessage::resource_found(),
            full_size: DataFrameSize {
                width: df.width(),
                height: df.height(),
            },
            slice_size: DataFrameSize {
                width: df.width(),
                height: df.height(),
            },
            df: JsonDataFrame::from_df(&mut df),
            commit: Some(resource.commit.clone()),
            resource: Some(resource_version),
            page_number: 1,
            page_size: df.height(),
            total_pages: 1,
            total_entries: df.height(),
        };
        Ok(HttpResponse::Ok().json(response))
    } else {
        log::error!("Metadata cache not computed for column {}", column);
        Ok(HttpResponse::BadRequest().json(StatusMessage::resource_not_found()))
    }
}

pub async fn images(req: HttpRequest) -> actix_web::Result<HttpResponse, OxenHttpError> {
    let app_data = app_data(&req)?;
    let namespace = path_param(&req, "namespace")?;
    let repo_name = path_param(&req, "repo_name")?;
    let repo = get_repo(&app_data.path, namespace, &repo_name)?;
    let resource = parse_resource(&req, &repo)?;

    log::debug!(
        "{} resource {}/{}",
        current_function!(),
        repo_name,
        resource
    );

    let latest_commit = api::local::commits::get_by_id(&repo, &resource.commit.id)?.ok_or(
        OxenError::revision_not_found(resource.commit.id.clone().into()),
    )?;

    log::debug!(
        "{} resolve commit {} -> '{}'",
        current_function!(),
        latest_commit.id,
        latest_commit.message
    );

    // TODO: get stats dataframe given the directory...figure out what the best API and response is for this...
    let entry = api::local::entries::get_meta_entry(&repo, &resource.commit, &resource.file_path)?;
    let meta = MetadataEntryResponse {
        status: StatusMessage::resource_found(),
        entry,
    };
    Ok(HttpResponse::Ok().json(meta))
}
