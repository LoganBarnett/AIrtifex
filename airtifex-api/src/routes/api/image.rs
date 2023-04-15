use crate::auth::Claims;
use crate::gen::image::{GenerateImageRequest, TextToImageData};
use crate::id::Uuid;
use crate::models::image::Image;
use crate::models::image_model::ImageModel;
use crate::models::image_sample::ImageSample;
use crate::models::user::User;
use crate::routes::handle_db_result_as_json;
use crate::Error;
use crate::{SharedAppState, ToAxumResponse};
use airtifex_core::image::{ImageModelListEntry, ImageSampleInspect};
use airtifex_core::{
    api_response::ApiResponse,
    image::{ImageInspect, TextToImageRequest, TextToImageResponse},
};

use axum::extract::Path;
use axum::{
    extract::{Json, State},
    response::Response,
    routing, Router,
};
use rand::Rng;

pub fn router() -> Router<SharedAppState> {
    Router::new()
        .route("/from-text", routing::post(text_to_image))
        .route("/", routing::get(list_images))
        .route("/models", routing::get(list_models))
        .route(
            "/:id",
            routing::get(get_image_metadata).delete(delete_image),
        )
        .route("/:id/samples", routing::get(list_image_entries))
        .route("/:id/samples/:n", routing::get(get_image_entry))
}

async fn text_to_image(
    claims: Claims,
    State(state): State<SharedAppState>,
    Json(request): Json<TextToImageRequest>,
) -> Response {
    let db = &state.db;
    with_user_guard!(claims, db);

    log::info!("{request:?}");

    let user_id = match User::get(&db, &claims.sub).await.map(|u| u.id) {
        Ok(id) => id,
        Err(e) => return ApiResponse::failure(e).internal_server_error(),
    };

    let image = Image::new(
        user_id,
        request.model,
        request.width.unwrap_or(512),
        request.height.unwrap_or(512),
        request.prompt,
        request.n_steps.map(|x| x as i64).unwrap_or(15),
        request.seed.unwrap_or_else(|| rand::thread_rng().gen()),
        request.num_samples.unwrap_or(1),
    );

    if let Err(e) = image.create(db).await {
        return ApiResponse::failure(e).internal_server_error();
    }

    let request = GenerateImageRequest::TextToImages(TextToImageData {
        id: image.id.to_string(),
        prompt: image.prompt,
        width: image.width,
        height: image.height,
        n_steps: image.n_steps as usize,
        seed: image.seed,
        num_samples: image.num_samples,
    });

    if let Some((model, tx_gen_req)) = state.tx_image_gen_req.get(&image.model) {
        log::error!("sending request to model {} == {model}", image.model);
        if let Err(e) = tx_gen_req.send_async(request).await {
            return ApiResponse::failure(e).internal_server_error();
        }
    } else {
        return ApiResponse::failure("Image generation from text is disabled")
            .internal_server_error();
    }

    ApiResponse::success(TextToImageResponse {
        image_id: image.id.to_string(),
    })
    .ok()
}

async fn list_images(claims: Claims, state: State<SharedAppState>) -> Response {
    let db = &state.db;
    with_user_guard!(claims, db);

    handle_db_result_as_json(
        Image::list(&db)
            .await
            .map(|e| {
                e.into_iter()
                    .map(|e| ImageInspect {
                        id: e.id.to_string(),
                        user_id: e.user_id.to_string(),
                        model: e.model,
                        width: e.width,
                        height: e.height,
                        prompt: e.prompt,
                        n_steps: e.n_steps,
                        seed: e.seed,
                        num_samples: e.num_samples,
                        processing: e.processing,
                        create_date: e.create_date,
                    })
                    .collect::<Vec<_>>()
            })
            .map_err(Error::from),
    )
}

async fn list_image_entries(
    claims: Claims,
    state: State<SharedAppState>,
    id: Path<Uuid>,
) -> Response {
    let db = &state.db;
    with_user_guard!(claims, db);

    handle_db_result_as_json(
        ImageSample::get_image_samples(&db, &id)
            .await
            .map(|e| {
                e.into_iter()
                    .map(|e| ImageSampleInspect {
                        sample_id: e.sample_id.to_string(),
                        image_id: e.image_id.to_string(),
                        n_sample: e.n,
                        data: e.data,
                    })
                    .collect::<Vec<_>>()
            })
            .map_err(Error::from),
    )
}

async fn get_image_entry(
    claims: Claims,
    state: State<SharedAppState>,
    Path((id, n)): Path<(Uuid, i32)>,
) -> Response {
    let db = &state.db;
    with_user_guard!(claims, db);

    handle_db_result_as_json(
        ImageSample::get_sample(&db, &id, n)
            .await
            .map(|e| ImageSampleInspect {
                sample_id: e.sample_id.to_string(),
                image_id: e.image_id.to_string(),
                n_sample: e.n,
                data: e.data,
            })
            .map_err(Error::from),
    )
}

async fn get_image_metadata(
    claims: Claims,
    state: State<SharedAppState>,
    Path(id): Path<Uuid>,
) -> Response {
    let db = &state.db;
    with_user_guard!(claims, db);

    let user_id = match User::get(&db, &claims.sub).await.map(|u| u.id) {
        Ok(id) => id.to_string(),
        Err(e) => return ApiResponse::failure(e).internal_server_error(),
    };

    handle_db_result_as_json(
        Image::get_by_id(&db, &id)
            .await
            .map(|image| ImageInspect {
                id: image.id.to_string(),
                user_id,
                model: image.model,
                width: image.width,
                height: image.height,
                prompt: image.prompt,
                n_steps: image.n_steps,
                seed: image.seed,
                num_samples: image.num_samples,
                processing: image.processing,
                create_date: image.create_date,
            })
            .map_err(Error::from),
    )
}

async fn delete_image(
    claims: Claims,
    state: State<SharedAppState>,
    Path(id): Path<Uuid>,
) -> Response {
    let db = &state.db;
    with_user_guard!(claims, db);

    handle_db_result_as_json(Image::delete(&db, &id).await.map_err(Error::from))
}

async fn list_models(claims: Claims, state: State<SharedAppState>) -> Response {
    let db = &state.db;
    with_user_guard!(claims, db);

    handle_db_result_as_json(
        ImageModel::list(&db)
            .await
            .map(|entries| {
                entries
                    .into_iter()
                    .map(|model| ImageModelListEntry {
                        model_id: model.model_id.to_string(),
                        name: model.name,
                        description: model.description,
                    })
                    .collect::<Vec<_>>()
            })
            .map_err(Error::from),
    )
}