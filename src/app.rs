#[allow(unused_variables)]
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct LogEntry {
    pub id: i32,
    pub distance: f64,
    pub created_at_str: String,
    pub threshold_yellow: f64,
    pub threshold_red: f64,
    pub threshold_collision: f64,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LogStats {
    pub danger_count: i64,
    pub collision_count: i64,
}

#[cfg(feature = "ssr")]
pub static SENSOR_DISTANCE: std::sync::RwLock<f64> = std::sync::RwLock::new(150.0);
#[cfg(feature = "ssr")]
pub static THRESHOLD_YELLOW: std::sync::RwLock<f64> = std::sync::RwLock::new(100.0);
#[cfg(feature = "ssr")]
pub static THRESHOLD_RED: std::sync::RwLock<f64> = std::sync::RwLock::new(50.0);
#[cfg(feature = "ssr")]
pub static THRESHOLD_COLLISION: std::sync::RwLock<f64> = std::sync::RwLock::new(10.0);

#[server(GetDistance, "/api")]
pub async fn get_distance() -> Result<f64, ServerFnError> {
    let dist = *SENSOR_DISTANCE.read().unwrap();
    Ok(dist)
}

#[server(UpdateThresholds, "/api")]
pub async fn update_thresholds(yellow: f64, red: f64, collision: f64) -> Result<(), ServerFnError> {
    if let Ok(mut y) = THRESHOLD_YELLOW.write() { *y = yellow; }
    if let Ok(mut r) = THRESHOLD_RED.write() { *r = red; }
    if let Ok(mut c) = THRESHOLD_COLLISION.write() { *c = collision; }
    Ok(())
}

#[server(GetLogs, "/api")]
pub async fn get_logs() -> Result<Vec<LogEntry>, ServerFnError> {
    use sqlx::PgPool;
    let pool = leptos_axum::extract::<axum::Extension<PgPool>>().await?.0;
    
    let logs = sqlx::query_as::<_, LogEntry>(
        "SELECT id, distance, TO_CHAR(created_at, 'DD.MM.YYYY HH24:MI:SS') as created_at_str, threshold_yellow, threshold_red, threshold_collision, status FROM sensor_logs ORDER BY id DESC LIMIT 50"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    
    Ok(logs)
}

#[server(GetLogStats, "/api")]
pub async fn get_log_stats() -> Result<LogStats, ServerFnError> {
    use sqlx::PgPool;
    let pool = leptos_axum::extract::<axum::Extension<PgPool>>().await?.0;
    
    let danger_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sensor_logs WHERE status = 'Gefahr'")
        .fetch_one(&pool).await.unwrap_or(0);
    let collision_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sensor_logs WHERE status = 'KOLLISION!'")
        .fetch_one(&pool).await.unwrap_or(0);
        
    Ok(LogStats { danger_count, collision_count })
}

#[server(CheckDb, "/api")]
pub async fn check_db() -> Result<bool, ServerFnError> {
    use sqlx::PgPool;
    let pool = leptos_axum::extract::<axum::Extension<PgPool>>().await?.0;
    let res = sqlx::query("SELECT 1").execute(&pool).await;
    Ok(res.is_ok())
}

#[allow(unused_variables)]
fn play_alarm_sound(sound_type: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        let url = match sound_type {
            "beep" => "https://actions.google.com/sounds/v1/alarms/beep_short.ogg",
            "siren" => "https://actions.google.com/sounds/v1/alarms/alarm_clock.ogg",
            "buzzer" => "https://actions.google.com/sounds/v1/alarms/spaceship_alarm.ogg",
            _ => return,
        };
        let code = format!("new Audio('{}').play().catch(e => console.log('Audio Fehler:', e));", url);
        let _ = js_sys::eval(&code);
    }
}

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="de">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/personalspace.css"/>
        <Title text="Sensor Control - Pro"/>
        <Router>
            <main class="app-container">
                <Routes fallback=|| "Seite nicht gefunden.".into_view()>
                    <Route path=StaticSegment("") view=HomePage/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    let active_tab = RwSignal::new("dashboard");
    
    let distance = RwSignal::new(150.0);
    let threshold_yellow = RwSignal::new(100.0);
    let threshold_red = RwSignal::new(50.0);
    let threshold_collision = RwSignal::new(10.0);

    let custom_text = RwSignal::new("Alles in Ordnung! Distanz: {dist:.1} cm".to_string());
    let update_interval = RwSignal::new(500.0);
    let sound_enabled = RwSignal::new(true);
    let is_live_mode = RwSignal::new(false);

    let logs_resource = Resource::new(|| (), |_| async move { get_logs().await.unwrap_or_default() });
    let stats_resource = Resource::new(|| (), |_| async move { get_log_stats().await.unwrap_or(LogStats { danger_count: 0, collision_count: 0 }) });
    let db_status = Resource::new(|| (), |_| async move { check_db().await.unwrap_or(false) });

    let status_class = Memo::new(move |_| {
        let d = distance.get();
        if d <= threshold_collision.get() { "status-collision" }
        else if d <= threshold_red.get() { "status-red" }
        else if d <= threshold_yellow.get() { "status-yellow" }
        else { "status-green" }
    });

    Effect::new(move |_| {
        let y = threshold_yellow.get();
        let r = threshold_red.get();
        let c = threshold_collision.get();
        leptos::task::spawn_local(async move {
            let _ = update_thresholds(y, r, c).await;
        });
    });

    Effect::new(move |_| {
        let status = status_class.get();
        if sound_enabled.get() {
            match status {
                "status-yellow" => play_alarm_sound("beep"),
                "status-red" => play_alarm_sound("siren"),
                "status-collision" => play_alarm_sound("buzzer"),
                _ => {}
            }
        }
    });

    Effect::new(move |_| {
        if is_live_mode.get() {
            let interval = update_interval.get() as u64;
            let handle = set_interval_with_handle(move || {
                leptos::task::spawn_local(async move {
                    if let Ok(dist) = get_distance().await {
                        distance.set(dist);
                        if active_tab.get() == "data" {
                            logs_resource.refetch();
                            stats_resource.refetch();
                        }
                    }
                });
            }, std::time::Duration::from_millis(interval));

            on_cleanup(move || {
                if let Ok(h) = handle { h.clear(); }
            });
        }
    });

    let formatted_text = Memo::new(move |_| {
        custom_text.get().replace("{dist:.1}", &format!("{:.1}", distance.get()))
    });

    view! {
        <div class="dashboard-wrapper">
            <header class="header">
                <h1>"Sensor Control"</h1>
                <div class="tabs">
                    <button class=move || if active_tab.get() == "dashboard" { "tab active" } else { "tab" } on:click=move |_| active_tab.set("dashboard")>"Dashboard"</button>
                    <button class=move || if active_tab.get() == "data" { "tab active" } else { "tab" } on:click=move |_| { active_tab.set("data"); logs_resource.refetch(); stats_resource.refetch(); }>"Data"</button>
                </div>
            </header>

            <Show when=move || active_tab.get() == "dashboard">
                <div class="dashboard-grid">
                    <div class="column">
                        <div class="card settings-card">
                            <h3>"Grenzwerte (cm)"</h3>
                            <div class="input-group"><label>"Gelb ab:"</label><input type="number" prop:value=threshold_yellow on:input=move |ev| { if let Ok(val) = event_target_value(&ev).parse::<f64>() { threshold_yellow.set(val); } } /></div>
                            <div class="input-group"><label>"Rot ab:"</label><input type="number" prop:value=threshold_red on:input=move |ev| { if let Ok(val) = event_target_value(&ev).parse::<f64>() { threshold_red.set(val); } } /></div>
                            <div class="input-group"><label>"Kollision ab:"</label><input type="number" prop:value=threshold_collision on:input=move |ev| { if let Ok(val) = event_target_value(&ev).parse::<f64>() { threshold_collision.set(val); } } /></div>
                        </div>

                        <div class="card simulation-card">
                            <h3>"Sensor Steuerung"</h3>
                            <div class="toggle-group">
                                <label>"Live-Modus (Raspberry Pi)"</label>
                                <label class="switch">
                                    <input type="checkbox" prop:checked=is_live_mode on:click=move |_| { is_live_mode.update(|v| *v = !*v); db_status.refetch(); } />
                                    <span class="slider round"></span>
                                </label>
                            </div>
                            
                            <div style=move || if is_live_mode.get() { "opacity: 0.3; pointer-events: none;" } else { "" }>
                                <input type="range" min="0" max="400" step="1"
                                    prop:value=distance
                                    on:input=move |ev| {
                                        distance.set(event_target_value(&ev).parse().unwrap_or(0.0));
                                    }
                                />
                                <p class="help-text">"Schieberegler bewegen, um Messung manuell zu simulieren."</p>
                            </div>
                        </div>
                    </div>

                    <div class="column">
                        <div class="card traffic-light-card">
                            <h2>"Status Anzeige"</h2>
                            <div class=move || format!("traffic-light {}", status_class.get())>
                                <div class="light red"></div>
                                <div class="light yellow"></div>
                                <div class="light green"></div>
                            </div>
                            <p class="status-text">
                                {move || match status_class.get() {
                                    "status-collision" => "KOLLISION!",
                                    "status-red" => "Gefahr",
                                    "status-yellow" => "Warnung",
                                    _ => "Sicher",
                                }}
                            </p>
                        </div>

                        <div class="card text-display-card">
                            <p>{formatted_text}</p>
                        </div>

                        <div class="card settings-card">
                            <h3>"Einstellungen"</h3>
                            <div class="input-group">
                                <label>"Update-Intervall (ms):"</label>
                                <input type="number" prop:value=update_interval on:input=move |ev| { if let Ok(val) = event_target_value(&ev).parse::<f64>() { update_interval.set(val); } }/>
                            </div>
                            <div class="input-group vertical">
                                <label>"Anzeige Warnungstext:"</label>
                                <input type="text" prop:value=custom_text on:input=move |ev| custom_text.set(event_target_value(&ev)) />
                            </div>
                            <div class="toggle-group" style="margin-top: 15px;">
                                <label>"Warntöne (Beep/Sirene/Buzzer)"</label>
                                <label class="switch"><input type="checkbox" prop:checked=sound_enabled on:click=move |_| sound_enabled.update(|v| *v = !*v) /><span class="slider round"></span></label>
                            </div>
                        </div>
                    </div>

                    <div class="column">
                        <div class="card distance-card">
                            <h3>"Aktuelle Messdaten"</h3>
                            <div class="distance-display">
                                <span class="value">{move || format!("{:.1}", distance.get())}</span>
                                <span class="unit">"cm"</span>
                            </div>
                        </div>

                        <div class="card system-status-card">
                            <h3>"System-Status"</h3>
                            <div class="status-item"><span class="status-icon green"></span><span>"Raspberry Pi verbunden"</span></div>
                            <div class="status-item"><span class=move || if is_live_mode.get() { "status-icon green" } else { "status-icon yellow" }></span><span>{move || if is_live_mode.get() { "Live-Modus aktiv" } else { "Sensor simuliert" }}</span></div>
                            
                            <div class="status-item">
                                <Suspense fallback=|| view! { <><span class="status-icon yellow"></span><span>"Prüfe DB..."</span></> }>
                                    <span class=move || if db_status.get().unwrap_or(false) { "status-icon green" } else { "status-icon red" }></span>
                                    <span>{move || if db_status.get().unwrap_or(false) { "Datenbank verbunden" } else { "Datenbank-Verbindung fehlt" }}</span>
                                </Suspense>
                            </div>
                        </div>
                    </div>
                </div>
            </Show>

            <Show when=move || active_tab.get() == "data">
                <div class="data-view">
                    
                    <div class="stats-banner">
                        <Suspense fallback=|| view! { <p>"Lade Statistiken..."</p> }>
                            {move || stats_resource.get().map(|stats| view! {
                                <div class="stat-box">
                                    <span class="stat-label">"Anzahl Gefahrenzustände:"</span>
                                    <span class="stat-value danger">{stats.danger_count}</span>
                                </div>
                                <div class="stat-box">
                                    <span class="stat-label">"Anzahl Kollisionen:"</span>
                                    <span class="stat-value collision">{stats.collision_count}</span>
                                </div>
                            })}
                        </Suspense>
                    </div>

                    <div class="card log-table-card">
                        <h3>"Letzte 50 Messwerte aus der PostgreSQL Datenbank"</h3>
                        <div class="table-container">
                            <table>
                                <thead>
                                    <tr>
                                        <th>"ID"</th>
                                        <th>"Zeitpunkt"</th>
                                        <th>"Distanz"</th>
                                        <th>"Zustand"</th>
                                        <th>"Grenzwerte (G/R/K)"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    <Suspense fallback=|| view! { <tr><td colspan="5">"Lade Daten..."</td></tr> }>
                                        {move || logs_resource.get().map(|entries| {
                                            entries.into_iter().map(|entry| {
                                                view! {
                                                    <tr>
                                                        <td>{entry.id}</td>
                                                        <td>{entry.created_at_str}</td>
                                                        <td>{format!("{:.1} cm", entry.distance)}</td>
                                                        <td>{entry.status}</td>
                                                        <td>{format!("{:.0} / {:.0} / {:.0} cm", entry.threshold_yellow, entry.threshold_red, entry.threshold_collision)}</td>
                                                    </tr>
                                                }
                                            }).collect_view()
                                        })}
                                    </Suspense>
                                </tbody>
                            </table>
                        </div>
                        <button class="btn-refresh" on:click=move |_| { logs_resource.refetch(); stats_resource.refetch(); }>"Werte manuell neu laden"</button>
                    </div>
                </div>
            </Show>
        </div>
    }
}