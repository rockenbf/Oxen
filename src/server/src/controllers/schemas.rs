use std::path::Path;

use crate::errors::OxenHttpError;
use crate::helpers::get_repo;
use crate::params::{app_data, parse_resource, path_param};

use liboxen::core::df::tabular;
use liboxen::model::Schema;
use liboxen::opts::DFOpts;
use liboxen::view::schema::{SchemaResponse, SchemaWithPath};
use liboxen::{repositories, util};

use actix_web::{HttpRequest, HttpResponse};
use liboxen::error::OxenError;
use liboxen::view::entry::ResourceVersion;
use liboxen::view::{ListSchemaResponse, StatusMessage};

pub async fn list_or_get(req: HttpRequest) -> actix_web::Result<HttpResponse, OxenHttpError> {
    let app_data = app_data(&req)?;

    let namespace = path_param(&req, "namespace")?;
    let repo_name = path_param(&req, "repo_name")?;
    let repo = get_repo(&app_data.path, namespace, repo_name)?;

    // Try to see if they are asking for a specific file
    if let Ok(resource) = parse_resource(&req, &repo) {
        if resource.path != Path::new("") {
            let commit = &resource.clone().commit.unwrap();

            log::debug!(
                "schemas::list_or_get file {:?} commit {}",
                resource.path,
                commit
            );

            let schemas = repositories::schemas::list_from_ref(
                &repo,
                &commit.id,
                resource.path.to_string_lossy(),
            )?;

            if schemas.is_empty() {
                return Err(OxenHttpError::NotFound);
            }

            let mut schema_w_paths: Vec<SchemaWithPath> = schemas
                .into_iter()
                .map(|(path, schema)| SchemaWithPath::new(path.to_string_lossy().into(), schema))
                .collect();

            // sort by hash
            schema_w_paths.sort_by(|a, b| a.schema.hash.cmp(&b.schema.hash));

            // If none found, try to get the schema from the file
            // TODO: Do we need this?
            if schema_w_paths.is_empty() {
                if let Some(entry) = repositories::entries::get_commit_entry(
                    &repo,
                    &resource.commit.unwrap(),
                    &resource.path,
                )? {
                    let version_path = util::fs::version_path(&repo, &entry);
                    log::debug!(
                        "No schemas found, trying to get from file {:?}",
                        resource.path
                    );
                    if util::fs::is_tabular(&version_path) {
                        let df = tabular::read_df(&version_path, DFOpts::empty())?;
                        let schema = Schema::from_polars(&df.schema());
                        schema_w_paths.push(SchemaWithPath::new(
                            resource.path.to_string_lossy().into(),
                            schema,
                        ));
                    }
                }
            }

            let resource = ResourceVersion {
                path: resource.path.to_string_lossy().into(),
                version: resource.version.to_string_lossy().into(),
            };
            let response = ListSchemaResponse {
                status: StatusMessage::resource_found(),
                schemas: schema_w_paths,
                commit: Some(commit.clone()),
                resource: Some(resource),
            };
            return Ok(HttpResponse::Ok().json(response));
        }
    }

    // Otherwise, list all schemas
    let revision = path_param(&req, "resource")?;

    let commit = repositories::revisions::get(&repo, &revision)?
        .ok_or(OxenError::revision_not_found(revision.to_owned().into()))?;

    log::debug!(
        "schemas::list_or_get revision {} commit {}",
        revision,
        commit
    );

    let schemas = repositories::schemas::list(&repo, Some(&commit.id))?;
    let mut schema_w_paths: Vec<SchemaWithPath> = schemas
        .into_iter()
        .map(|(path, schema)| SchemaWithPath::new(path.to_string_lossy().into(), schema))
        .collect();
    schema_w_paths.sort_by(|a, b| a.schema.hash.cmp(&b.schema.hash));

    let response = ListSchemaResponse {
        status: StatusMessage::resource_found(),
        schemas: schema_w_paths,
        commit: Some(commit),
        resource: None,
    };
    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_by_hash(req: HttpRequest) -> actix_web::Result<HttpResponse, OxenHttpError> {
    let app_data = app_data(&req)?;

    let namespace = path_param(&req, "namespace")?;
    let repo_name = path_param(&req, "repo_name")?;
    let repo = get_repo(&app_data.path, namespace, repo_name)?;

    let hash = path_param(&req, "hash")?;

    let maybe_schema = repositories::schemas::get_by_hash(&repo, hash)?;

    if let Some(schema) = maybe_schema {
        let response = SchemaResponse {
            status: StatusMessage::resource_found(),
            schema,
        };
        Ok(HttpResponse::Ok().json(response))
    } else {
        Err(OxenHttpError::NotFound)
    }
}
