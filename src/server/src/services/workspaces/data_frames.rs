use actix_web::web;
use actix_web::Scope;

use crate::controllers;

pub mod rows;

pub fn data_frames() -> Scope {
    web::scope("/data_frames")
        .route(
            "/branch/{branch:.*}",
            web::get().to(controllers::workspaces::data_frames::get_by_branch),
        )
        .route(
            "/resource/{resource:.*}",
            web::get().to(controllers::workspaces::data_frames::get_by_resource),
        )
        .route(
            "/diff/{resource:.*}",
            web::get().to(controllers::workspaces::data_frames::diff),
        )
        .route(
            "/resource/{resource:.*}",
            web::put().to(controllers::workspaces::data_frames::put),
        )
        .service(rows::rows())
}
