use std::path::PathBuf;

use crate::errors::OxenHttpError;

use actix_web::{web, HttpRequest, HttpResponse};
use liboxen::core::index::{CommitReader, Merger};
use liboxen::error::OxenError;
use liboxen::model::compare::tabular_compare::TabularCompareBody;
use liboxen::model::{Commit, LocalRepository};
use liboxen::opts::DFOpts;
use liboxen::view::compare::{
    CompareCommits, CompareCommitsResponse, CompareEntries, CompareEntryResponse, CompareTabularResponse,
};
use liboxen::view::{CompareEntriesResponse, StatusMessage};
use liboxen::{api, constants, util};

use crate::helpers::get_repo;
use crate::params::{
    app_data, df_opts_query, parse_base_head, path_param, resolve_base_head, DFOptsQuery,
    PageNumQuery, self,
};

pub async fn commits(
    req: HttpRequest,
    query: web::Query<PageNumQuery>,
) -> actix_web::Result<HttpResponse, OxenHttpError> {
    let app_data = app_data(&req)?;
    let namespace = path_param(&req, "namespace")?;
    let name = path_param(&req, "repo_name")?;
    let base_head = path_param(&req, "base_head")?;

    // Get the repository or return error
    let repository = get_repo(&app_data.path, namespace, name)?;

    // Page size and number
    let page = query.page.unwrap_or(constants::DEFAULT_PAGE_NUM);
    let page_size = query.page_size.unwrap_or(constants::DEFAULT_PAGE_SIZE);

    // Parse the base and head from the base..head string
    let (base, head) = parse_base_head(&base_head)?;
    let (base_commit, head_commit) = resolve_base_head(&repository, &base, &head)?;

    let base_commit = base_commit.ok_or(OxenError::revision_not_found(base.into()))?;
    let head_commit = head_commit.ok_or(OxenError::revision_not_found(head.into()))?;

    // Check if mergeable
    let merger = Merger::new(&repository)?;

    // Get commits between base and head
    let commit_reader = CommitReader::new(&repository)?;
    let commits =
        merger.list_commits_between_commits(&commit_reader, &base_commit, &head_commit)?;
    let (paginated, pagination) = util::paginate(commits, page, page_size);

    let compare = CompareCommits {
        base_commit,
        head_commit,
        commits: paginated,
    };

    let view = CompareCommitsResponse {
        status: StatusMessage::resource_found(),
        compare,
        pagination,
    };
    Ok(HttpResponse::Ok().json(view))
}

pub async fn entries(
    req: HttpRequest,
    query: web::Query<PageNumQuery>,
) -> actix_web::Result<HttpResponse, OxenHttpError> {
    let app_data = app_data(&req)?;
    let namespace = path_param(&req, "namespace")?;
    let name = path_param(&req, "repo_name")?;
    let base_head = path_param(&req, "base_head")?;

    // Get the repository or return error
    let repository = get_repo(&app_data.path, namespace, name)?;

    // Page size and number
    let page = query.page.unwrap_or(constants::DEFAULT_PAGE_NUM);
    let page_size = query.page_size.unwrap_or(constants::DEFAULT_PAGE_SIZE);

    // Parse the base and head from the base..head string
    let (base, head) = parse_base_head(&base_head)?;
    let (base_commit, head_commit) = resolve_base_head(&repository, &base, &head)?;

    let base_commit = base_commit.ok_or(OxenError::revision_not_found(base.into()))?;
    let head_commit = head_commit.ok_or(OxenError::revision_not_found(head.into()))?;

    let entries_diff = api::local::diff::list_diff_entries(
        &repository,
        &base_commit,
        &head_commit,
        page,
        page_size,
    )?;
    let entries = entries_diff.entries;
    let pagination = entries_diff.pagination;

    let compare = CompareEntries {
        base_commit,
        head_commit,
        counts: entries_diff.counts,
        entries,
    };
    let view = CompareEntriesResponse {
        status: StatusMessage::resource_found(),
        compare,
        pagination,
    };
    Ok(HttpResponse::Ok().json(view))
}

pub async fn file(
    req: HttpRequest,
    query: web::Query<DFOptsQuery>,
) -> actix_web::Result<HttpResponse, OxenHttpError> {
    let app_data = app_data(&req)?;
    let namespace = path_param(&req, "namespace")?;
    let name = path_param(&req, "repo_name")?;
    let base_head = path_param(&req, "base_head")?;

    // Get the repository or return error
    let repository = get_repo(&app_data.path, namespace, name)?;

    // Parse the base and head from the base..head/resource string
    // For Example)
    //   main..feature/add-data/path/to/file.txt
    let (base_commit, head_commit, resource) = parse_base_head_resource(&repository, &base_head)?;

    let base_entry = api::local::entries::get_commit_entry(&repository, &base_commit, &resource)?;
    let head_entry = api::local::entries::get_commit_entry(&repository, &head_commit, &resource)?;


    let mut opts = DFOpts::empty();
    opts = df_opts_query::parse_opts(&query, &mut opts);

    let page_size = query.page_size.unwrap_or(constants::DEFAULT_PAGE_SIZE);
    let page = query.page.unwrap_or(constants::DEFAULT_PAGE_NUM);

    let start = if page == 0 { 0 } else { page_size * (page - 1) };
    let end = page_size * page;
    opts.slice = Some(format!("{}..{}", start, end));

    let diff = api::local::diff::diff_entries(
        &repository,
        base_entry,
        &base_commit,
        head_entry,
        &head_commit,
        opts,
    )?;

    let view = CompareEntryResponse {
        status: StatusMessage::resource_found(),
        compare: diff,
    };
    Ok(HttpResponse::Ok().json(view))
}

// TODONOW, naming - since `compare` namespae already eaten up by diff 

pub async fn df(
    req: HttpRequest, 
    query: web::Query<DFOptsQuery>, // todonow needed?
    body: String,
) -> actix_web::Result<HttpResponse, OxenHttpError> {
    let app_data = app_data(&req)?;
    let namespace = path_param(&req, "namespace")?;
    let name = path_param(&req, "repo_name")?;
    let base_head = path_param(&req, "base_head")?;
    let repository = get_repo(&app_data.path, namespace, name)?;


    let data: Result<TabularCompareBody, serde_json::Error> = serde_json::from_str(&body);
    let data = match data {
        Ok(data) => data,
        Err(err) => {
            log::error!("unable to parse tabular comparison data. Err: {}\n{}", err, body);
            return Ok(HttpResponse::BadRequest().json(StatusMessage::error(err.to_string())));
        }
    };

    let mut opts = DFOpts::empty();
    opts = df_opts_query::parse_opts(&query, &mut opts);

    let resource_1 = PathBuf::from(data.path_1);
    let resource_2 = PathBuf::from(data.path_2);
    let keys = data.keys;
    let targets = data.targets;
    

    let (commit_1, commit_2) = params::parse_base_head(&base_head)?;
    let commit_1 = api::local::revisions::get(&repository, &commit_1)?
        .ok_or_else(|| OxenError::revision_not_found(commit_1.into()))?;
    let commit_2 = api::local::revisions::get(&repository, &commit_2)?
        .ok_or_else(|| OxenError::revision_not_found(commit_2.into()))?;


    let entry_1 = api::local::entries::get_commit_entry(&repository, &commit_1, &resource_1)?;
    let entry_2 = api::local::entries::get_commit_entry(&repository, &commit_2, &resource_2)?;

    // TODONOW handle these opts later
    // let mut opts = DFOpts::empty();
    // opts = df_opts_query::parse_opts(&query, &mut opts);

    // let page_size = query.page_size.unwrap_or(constants::DEFAULT_PAGE_SIZE);
    // let page = query.page.unwrap_or(constants::DEFAULT_PAGE_NUM);

    // let start = if page == 0 { 0 } else { page_size * (page - 1) };
    // let end = page_size * page;
    // opts.slice = Some(format!("{}..{}", start, end));

    // TODONOW PAGINATION! 
    
    // TODONOW: resource with commit?
    let compare = api::local::compare::compare_files(&repository, resource_1, commit_1, resource_2, commit_2, keys, targets, opts)?;

    let view = CompareTabularResponse {
        status: StatusMessage::resource_found(),
        compare: compare,
    };

    Ok(HttpResponse::Ok().json(view))
}

fn parse_base_head_resource(
    repo: &LocalRepository,
    base_head: &str,
) -> Result<(Commit, Commit, PathBuf), OxenError> {
    log::debug!("Parsing base_head_resource: {}", base_head);

    let mut split = base_head.split("..");
    let base = split
        .next()
        .ok_or(OxenError::resource_not_found(base_head))?;
    let head = split
        .next()
        .ok_or(OxenError::resource_not_found(base_head))?;

    let base_commit = api::local::revisions::get(repo, base)?
        .ok_or(OxenError::revision_not_found(base.into()))?;

    // Split on / and find longest branch name
    let split_head = head.split('/');
    let mut longest_str = String::from("");
    let mut head_commit: Option<Commit> = None;
    let mut resource: Option<PathBuf> = None;

    for s in split_head {
        let maybe_revision = format!("{}{}", longest_str, s);
        log::debug!("Checking maybe head revision: {}", maybe_revision);
        let commit = api::local::revisions::get(repo, &maybe_revision)?;
        if commit.is_some() {
            head_commit = commit;
            let mut r_str = head.replace(&maybe_revision, "");
            // remove first char from r_str
            r_str.remove(0);
            resource = Some(PathBuf::from(r_str));
        }
        longest_str = format!("{}/", maybe_revision);
    }

    log::debug!("Got head_commit: {:?}", head_commit);
    log::debug!("Got resource: {:?}", resource);

    let head_commit = head_commit.ok_or(OxenError::revision_not_found(head.into()))?;
    let resource = resource.ok_or(OxenError::revision_not_found(head.into()))?;

    Ok((base_commit, head_commit, resource))
}

// TODONOW - anything we can factor out here? 
// fn parse_base_head_resources_symmetric(
//     repo: &LocalRepository,
//     base_head: &str, 
// ) -> Result<(Commit, Commit, PathBuf, PathBuf), OxenError> {
//     log::debug!("Parsing base_head_resource_symmetric: {}", base_head);

//     let mut split = base_head.split("..");
//     let base = split
//         .next()
//         .ok_or(OxenError::resource_not_found(base_head))?;
//     let head = split
//         .next()
//         .ok_or(OxenError::resource_not_found(base_head))?;

//     let longest_str = String::from("");
//     let base_commit: Option<Commit> = None;
//     let base_resource: Option<PathBuf> = None;
//     let split_base = base.split('/');

//     for s in split_base {
//         let maybe_revision = format!("{}{}", longest_str, s);
//         log::debug!("Checking maybe head revision: {}", maybe_revision);
//         let commit = api::local::revisions::get(repo, &maybe_revision)?;
//         if commit.is_some() {
//             base_commit = commit;
//             let mut r_str = head.replace(&maybe_revision, "");
//             // remove first char from r_str
//             r_str.remove(0);
//             base_resource = Some(PathBuf::from(r_str));
//         }
//         longest_str = format!("{}/", maybe_revision);
//     }

//     let longest_str = String::from("");
//     let head_commit: Option<Commit> = None;
//     let head_resource: Option<PathBuf> = None;
//     let split_base = base.split('/');

//     for s in split_head {
//         let maybe_revision = format!("{}{}", longest_str, s);
//         log::debug!("Checking maybe head revision: {}", maybe_revision);
//         let commit = api::local::revisions::get(repo, &maybe_revision)?;
//         if commit.is_some() {
//             head_commit = commit;
//             let mut r_str = head.replace(&maybe_revision, "");
//             // remove first char from r_str
//             r_str.remove(0);
//             base_resource = Some(PathBuf::from(r_str));
//         }
//         longest_str = format!("{}/", maybe_revision);
//     }

//     log::debug!("Got head_commit: {:?}", head_commit);
//     log::debug!("Got head resource: {:?}", head_resource);

//     log::debug!("Got base_commit: {:?}", base_commit);
//     log::debug!("Got base resource: {:?}", base_resource);
// }