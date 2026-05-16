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
        use std::thread; // Wichtig für die präzisen 10 Mikrosekunden

        let gpio = match Gpio::new() {
            Ok(g) => g,
            Err(e) => {
                println!("GPIO Fehler: {}", e);
                return;
            }
        };

        let mut trig = gpio.get(23).unwrap().into_output();
        let echo = gpio.get(24).unwrap().into_input();

        println!("Sensor gestartet. Messe alle 300ms...");

        loop {
            // 1. Trigger kurz erden, damit der Sensor einen sauberen Zustand hat
            trig.set_low();
            thread::sleep(Duration::from_micros(2));

            // 2. 10 Mikrosekunden Impuls senden
            trig.set_high();
            thread::sleep(Duration::from_micros(10));
            trig.set_low();

            let timeout_start = Instant::now();
            let mut start = Instant::now();

            // 3. Warten bis Echo HIGH wird (max 30ms)
            while echo.is_low() {
                if timeout_start.elapsed() > Duration::from_millis(30) { break; }
                start = Instant::now();
            }

            let timeout_end = Instant::now();
            let mut end = Instant::now();

            // 4. Warten bis Echo wieder LOW wird (max 30ms)
            while echo.is_high() {
                if timeout_end.elapsed() > Duration::from_millis(30) { break; }
                end = Instant::now();
            }

            // 5. Distanz berechnen
            let duration = end.duration_since(start);
            let distance_cm = (duration.as_secs_f64() * 34300.0) / 2.0;

            // 6. Auswerten und in Konsole drucken
            if distance_cm >= 2.0 && distance_cm <= 400.0 {
                println!("-> Distanz: {:.1} cm", distance_cm);

                // Nur echte, gültige Werte in die Datenbank und ins Web-UI schreiben
                if let Ok(mut dist) = personalspace::app::SENSOR_DISTANCE.write() {
                    *dist = distance_cm;
                }
                let _ = sqlx::query("INSERT INTO sensor_logs (distance) VALUES ($1)")
                    .bind(distance_cm)
                    .execute(&pool)
                    .await;
            } else {
                // Wenn der Wert Quatsch ist (z.B. 0.0 oder Timeout), zeigen wir es dir in der Konsole an!
                println!("-> Fehlmessung (ignoriert): {:.1} cm", distance_cm);
            }

            // Exakt 300ms warten bis zur nächsten Messung
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    });
}

#[cfg(not(all(feature = "ssr", target_os = "linux")))]
fn start_hardware_sensor(_pool: sqlx::PgPool) {
    leptos::logging::log!("Hinweis: Da du nicht auf Linux/Pi bist, bleibt der echte Hardware-Sensor inaktiv.");
}
// -------------------------------------------------------------------------

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
        .expect("Fehler bei der Verbindung zu PostgreSQL.");

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