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

#[cfg(all(feature = "ssr", target_os = "linux"))]
fn start_hardware_sensor(pool: sqlx::PgPool) {
    tokio::spawn(async move {
        use rppal::gpio::Gpio;
        use std::time::{Duration, Instant};

        let gpio = match Gpio::new() {
            Ok(g) => g,
            Err(e) => {
                leptos::logging::log!("GPIO Fehler: Hardware-Sensor konnte nicht initialisiert werden: {}", e);
                return;
            }
        };

        let mut trig = gpio.get(18).unwrap().into_output();
        let echo = gpio.get(24).unwrap().into_input();

        leptos::logging::log!("Echter HC-SR04 Sensor-Task erfolgreich gestartet (GPIO 18/24).");

        loop {
            trig.set_low();
            tokio::time::sleep(Duration::from_micros(2)).await;
            trig.set_high();
            tokio::time::sleep(Duration::from_micros(10)).await;
            trig.set_low();

            let start_timeout = Instant::now();
            while echo.is_low() {
                if start_timeout.elapsed() > Duration::from_millis(30) { break; }
            }
            let echo_start = Instant::now();

            let mut echo_end = Instant::now();
            while echo.is_high() {
                if echo_start.elapsed() > Duration::from_millis(30) { break; }
                echo_end = Instant::now();
            }

            let duration = echo_end.duration_since(echo_start);
            let distance_cm = (duration.as_secs_f64() * 34300.0) / 2.0;

            if distance_cm > 2.0 && distance_cm < 400.0 {
                // --- NEU: LOG FÜR ERFOLGREICHE MESSUNG ---
                println!("Distanz gemessen: {:.1} cm", distance_cm);

                if let Ok(mut dist) = personalspace::app::SENSOR_DISTANCE.write() {
                    *dist = distance_cm;
                }

                let _ = sqlx::query("INSERT INTO sensor_logs (distance) VALUES ($1)")
                    .bind(distance_cm)
                    .execute(&pool)
                    .await;
            } else {
                // --- NEU: LOG FÜR FEHLMESSUNG / TIMEOUT ---
                println!("Fehlmessung oder Timeout (wird ignoriert): {:.1} cm", distance_cm);
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
}

#[cfg(not(all(feature = "ssr", target_os = "linux")))]
fn start_hardware_sensor(_pool: sqlx::PgPool) {
    leptos::logging::log!("Hinweis: Da du nicht auf Linux/Pi bist, bleibt der echte Hardware-Sensor inaktiv. Nutze Curl oder den Simulator!");
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

    start_hardware_sensor(pool.clone());

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
pub fn main() {}