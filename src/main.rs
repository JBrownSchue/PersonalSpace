#[cfg(feature = "ssr")]
#[derive(serde::Deserialize)]
pub struct SensorPayload {
    pub distance: f64,
}

#[cfg(feature = "ssr")]
pub async fn update_sensor(
    axum::Extension(pool): axum::Extension<sqlx::PgPool>, 
    axum::Json(payload): axum::Json<SensorPayload>
) -> &'static str {
    if let Ok(mut dist) = personalspace::app::SENSOR_DISTANCE.write() {
        *dist = payload.distance;
    }

    let _ = sqlx::query("INSERT INTO sensor_logs (distance) VALUES ($1)")
        .bind(payload.distance)
        .execute(&pool)
        .await;

    "OK"
}

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::{routing::post, Router};
    use leptos::logging::log;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use personalspace::app::*;
    use sqlx::postgres::PgPoolOptions;

    dotenvy::dotenv().ok();

    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL in .env fehlt!");
    log!("Verbinde mit der Datenbank...");
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .expect("Fehler bei der Verbindung zu PostgreSQL. Läuft der Docker Container?");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sensor_logs (
            id SERIAL PRIMARY KEY,
            distance DOUBLE PRECISION NOT NULL,
            created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
        );"
    )
    .execute(&pool)
    .await
    .expect("Konnte die Tabelle nicht erstellen.");
    
    log!("Datenbank bereit!");

    let conf = get_configuration(Some("Cargo.toml")).unwrap();
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    let app = Router::new()
        .route("/api/sensor", post(update_sensor)) 
        .layer(axum::Extension(pool)) 
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);

    log!("Server lauscht auf http://{}", &addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {

}