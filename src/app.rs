use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

// Hilfsfunktion: Spielt den ausgewählten Ton über den Browser ab
fn play_alarm_sound(_sound_type: &str) {
    // Wird nur im WebAssembly-Client ausgeführt (nicht auf dem Server)
    #[cfg(target_arch = "wasm32")]
    {
        // Öffentliche, freie Alarmtöne
        let url = match _sound_type {
            "beep" => "https://actions.google.com/sounds/v1/alarms/beep_short.ogg",
            "siren" => "https://actions.google.com/sounds/v1/alarms/alarm_clock.ogg",
            "buzzer" => "https://actions.google.com/sounds/v1/weapons/big_explosion_cut_off.ogg",
            _ => return,
        };
        
        // JS-Befehl zum Abspielen (catch verhindert Fehler, falls der Browser Autoplay blockiert)
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
    // Zustände für die Sensor-Simulation
    let distance = RwSignal::new(150.0);
    
    // Benutzereinstellbare Grenzwerte
    let threshold_yellow = RwSignal::new(100.0);
    let threshold_red = RwSignal::new(50.0);
    let threshold_collision = RwSignal::new(10.0);

    // Zustände für das Layout
    let custom_text = RwSignal::new("Alles in Ordnung! Distanz: {dist:.1} cm".to_string());
    let update_interval = RwSignal::new(100.0);
    let is_recording = RwSignal::new(true);
    let show_timestamps = RwSignal::new(true);
    
    // Zustand für den ausgewählten Alarmton
    let selected_sound = RwSignal::new("beep".to_string());

    // Berechneter Status für die Ampel
    let status_class = Memo::new(move |_| {
        let d = distance.get();
        if d <= threshold_collision.get() { "status-collision" }
        else if d <= threshold_red.get() { "status-red" }
        else if d <= threshold_yellow.get() { "status-yellow" }
        else { "status-green" }
    });

    // NEU: Automatischer Alarm, sobald der Zustand auf Rot oder Kollision springt
    Effect::new(move |_| {
        let status = status_class.get();
        let sound = selected_sound.get();
        
        if status == "status-collision" || status == "status-red" {
            if sound != "mute" {
                play_alarm_sound(&sound);
            }
        }
    });

    // Berechneter benutzerdefinierter Text
    let formatted_text = Memo::new(move |_| {
        custom_text.get().replace("{dist:.1}", &format!("{:.1}", distance.get()))
    });

    view! {
        <div class="dashboard-wrapper">
            <header class="header">
                <h1>"Sensor Control"</h1>
            </header>

            <div class="dashboard-grid">
                
                // --- Linke Spalte ---
                <div class="column">
                    <div class="card settings-card">
                        <h3>"Grenzwerte (cm)"</h3>
                        <div class="input-group">
                            <label>"Gelb ab:"</label>
                            <input type="number" 
                                prop:value=threshold_yellow 
                                on:input=move |ev| {
                                    if let Ok(val) = event_target_value(&ev).parse::<f64>() {
                                        threshold_yellow.set(val);
                                    }
                                } 
                            />
                        </div>
                        <div class="input-group">
                            <label>"Rot ab:"</label>
                            <input type="number" 
                                prop:value=threshold_red 
                                on:input=move |ev| {
                                    if let Ok(val) = event_target_value(&ev).parse::<f64>() {
                                        threshold_red.set(val);
                                    }
                                } 
                            />
                        </div>
                        <div class="input-group">
                            <label>"Kollision ab:"</label>
                            <input type="number" 
                                prop:value=threshold_collision 
                                on:input=move |ev| {
                                    if let Ok(val) = event_target_value(&ev).parse::<f64>() {
                                        threshold_collision.set(val);
                                    }
                                } 
                            />
                        </div>
                    </div>

                    <div class="card simulation-card">
                        <h3>"Sensor Simulation"</h3>
                        <input type="range" min="0" max="400" step="1"
                            prop:value=distance
                            on:input=move |ev| {
                                distance.set(event_target_value(&ev).parse().unwrap_or(0.0));
                            }
                        />
                        <p class="help-text">"Schieberegler bewegen, um HC-SR04 Messung zu simulieren."</p>
                    </div>
                </div>

                // --- Mittlere Spalte ---
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
                            <input type="number" prop:value=update_interval />
                        </div>
                        <div class="input-group vertical">
                            <label>"Anzeige Warnungstext:"</label>
                            <input type="text" prop:value=custom_text on:input=move |ev| custom_text.set(event_target_value(&ev)) />
                        </div>
                        <div class="input-group vertical">
                            <label>"Alarmton:"</label>
                            <div class="sound-select-row">
                                <select 
                                    prop:value=selected_sound
                                    on:change=move |ev| selected_sound.set(event_target_value(&ev))
                                >
                                    <option value="beep">"Beep (Standard)"</option>
                                    <option value="siren">"Sirene"</option>
                                    <option value="buzzer">"Buzzer"</option>
                                    <option value="mute">"Stumm"</option>
                                </select>
                                // Manueller Test-Button
                                <button class="btn-test" on:click=move |_| {
                                    let sound = selected_sound.get();
                                    if sound != "mute" {
                                        play_alarm_sound(&sound);
                                    }
                                }>"Test"</button>
                            </div>
                        </div>
                    </div>
                </div>

                // --- Rechte Spalte ---
                <div class="column">
                    <div class="card distance-card">
                        <h3>"Aktuelle Messdaten"</h3>
                        <div class="distance-display">
                            <span class="value">{move || format!("{:.1}", distance.get())}</span>
                            <span class="unit">"cm"</span>
                        </div>
                    </div>

                    <div class="card data-log-card">
                        <h3>"Daten-Aufzeichnung"</h3>
                        <div class="toggle-group">
                            <label>"Aufzeichnung läuft"</label>
                            <label class="switch">
                                <input type="checkbox" prop:checked=is_recording on:click=move |_| is_recording.update(|v| *v = !*v) />
                                <span class="slider round"></span>
                            </label>
                        </div>
                        <div class="toggle-group">
                            <label>"Zeitstempel anzeigen"</label>
                            <label class="switch">
                                <input type="checkbox" prop:checked=show_timestamps on:click=move |_| show_timestamps.update(|v| *v = !*v) />
                                <span class="slider round"></span>
                            </label>
                        </div>
                        <div class="input-group vertical">
                            <label>"Dateiname:"</label>
                            <input type="text" value="sensor_log.csv" />
                        </div>
                    </div>

                    <div class="card system-status-card">
                        <h3>"System-Status"</h3>
                        <div class="status-item">
                            <span class="status-icon green"></span>
                            <span>"Raspberry Pi verbunden"</span>
                        </div>
                        <div class="status-item">
                            <span class="status-icon green"></span>
                            <span>"HC-SR04 Sensor aktiv"</span>
                        </div>
                        <div class="status-item">
                            <span class="status-icon red"></span>
                            <span>"Datenbank-Verbindung fehlt"</span>
                        </div>
                    </div>
                </div>

            </div>
        </div>
    }
}