//! Simulator: verhält sich wie Pilot 2 auf einer gekoppelten Fernsteuerung. Holt den Zugang über
//! `/pilot/api/connect`, setzt einen Kartenpunkt, spielt einen Flug ab und beantwortet
//! Livebild-Befehle mit einem Testbild.

use std::process::{ExitCode, Stdio};
use std::time::Duration;

use rumqttc::{AsyncClient, ConnectionError, Event, EventLoop, MqttOptions, Outgoing, Packet, QoS};
use serde_json::{Value, json};
use suchlage::dji::pilot::PilotConnection;
use suchlage::dji::record::{RecordLine, read_lines};
use suchlage::sim::fake_pilot::{
    Action, Args, ConnectOutcome, LiveVideo, broker_address, classify_connect, handle_service,
    live_status, parse_args, pilot_pin, replay_script, retry_delays,
};
use suchlage::sim::synth::{Scenario, generate};
use tokio::process::{Child, Command};
use tokio::time::Instant;
use tracing_subscriber::EnvFilter;

const PAUSE_BETWEEN_RUNS: Duration = Duration::from_secs(10);

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    match run(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Simulator beendet: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(args: Args) -> Result<(), String> {
    let connection = connect(&args).await?;
    // Wie ein Pilot, der einen Punkt setzt: prüft den Weg über `/map/api/v1/…`. Kein Abbruch, wenn es scheitert.
    match share_pin(&args, &connection.api_token).await {
        Ok(()) => tracing::info!("Kartenpunkt gesetzt"),
        Err(e) => tracing::warn!(error = %e, "Kartenpunkt nicht gesetzt"),
    }
    let mut pilot = Pilot::new(args, &connection)?;
    let result = pilot.fly().await;
    pilot.finish().await;
    result
}

/// Holt den Zugang wie die Pilot-Seite. Wartet, solange der Server noch startet.
async fn connect(args: &Args) -> Result<PilotConnection, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!("{}/pilot/api/connect", args.server.trim_end_matches('/'));
    let body = json!({ "code": args.code, "rc_sn": args.rc_sn });
    let mut delays = retry_delays().into_iter();
    loop {
        let (status, detail) = match client.post(&url).json(&body).send().await {
            Ok(r) if classify_connect(Some(r.status().as_u16())) == ConnectOutcome::Ready => {
                return r
                    .json()
                    .await
                    .map_err(|e| format!("Antwort des Servers unlesbar: {e}"));
            }
            Ok(r) => (Some(r.status().as_u16()), server_error(r).await),
            Err(e) => (None, e.to_string()),
        };
        if classify_connect(status) == ConnectOutcome::GiveUp {
            let status = status.unwrap_or_default();
            return Err(format!("Kopplung abgelehnt ({status}): {detail}"));
        }
        let Some(delay) = delays.next() else {
            return Err(format!("Server nicht bereit: {detail}"));
        };
        tracing::info!(?delay, %detail, "Server noch nicht bereit – neuer Versuch");
        tokio::time::sleep(delay).await;
    }
}

/// Legt den Punkt aus `pilot_pin` an. Die Workspace-ID kommt wie auf der Pilot-Seite aus der Lizenz.
async fn share_pin(args: &Args, token: &str) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let server = args.server.trim_end_matches('/');
    let license: Value = client
        .post(format!("{server}/pilot/api/license"))
        .json(&json!({ "code": args.code }))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let workspace = license["workspaceId"]
        .as_str()
        .ok_or("Lizenz ohne Workspace-ID")?;
    let url = format!(
        "{server}/map/api/v1/workspaces/{workspace}/element-groups/{}/elements",
        suchlage::dji::map::SHARED_GROUP_ID
    );
    let response = client
        .post(url)
        .header("x-auth-token", token)
        .json(&pilot_pin(&args.rc_sn, suchlage::contract::examples::HOME))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        let status = response.status();
        let body: Value = response.json().await.unwrap_or_default();
        return Err(format!("{status}: {}", body["message"]));
    }
    Ok(())
}

/// Der deutsche Satz aus `{"error": …}`, sonst der Statuscode.
async fn server_error(response: reqwest::Response) -> String {
    let status = response.status();
    response
        .json::<Value>()
        .await
        .ok()
        .and_then(|b| b["error"].as_str().map(String::from))
        .unwrap_or_else(|| status.to_string())
}

/// Synthetischer Flug ab jetzt oder der Mitschnitt, umgeschrieben auf die gekoppelte SN.
fn script(args: &Args) -> Result<Vec<RecordLine>, String> {
    let Some(path) = &args.replay else {
        let scenario = Scenario::live(&args.rc_sn, &args.aircraft_sn);
        return Ok(generate(&scenario, suchlage::time::now_ms()));
    };
    let file = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
    let lines = read_lines(std::io::BufReader::new(file))
        .filter_map(|line| {
            line.map_err(|e| tracing::warn!(error = %e, "Mitschnittzeile übersprungen"))
                .ok()
        })
        .collect();
    let script = replay_script(lines, &args.rc_sn);
    if script.is_empty() {
        return Err(format!("{path}: keine Gerätenachricht zum Abspielen"));
    }
    Ok(script)
}

/// Sendet das Testbild an die RTMP-Adresse aus `live_start_push`, wie Pilot 2 das Kamerabild.
fn start_ffmpeg(url: &str) -> Option<Child> {
    Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-re",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=1280x720:rate=25",
        ])
        .args([
            "-an",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-tune",
            "zerolatency",
        ])
        .args(["-g", "25", "-f", "flv", url])
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| tracing::warn!(error = %e, "ffmpeg nicht startbar – Livebild bleibt aus"))
        .ok()
}

/// Läuft ffmpeg noch? Ein beendeter Prozess, etwa von MediaMTX abgewiesen, wird beim nächsten Start ersetzt.
fn is_running(child: &mut Option<Child>) -> bool {
    child
        .as_mut()
        .is_some_and(|c| matches!(c.try_wait(), Ok(None)))
}

/// Wie `wait_until` endet: Termin erreicht oder Strg+C.
#[derive(PartialEq)]
enum Wake {
    Due,
    Interrupted,
}

struct Pilot {
    args: Args,
    client: AsyncClient,
    eventloop: EventLoop,
    services_topic: String,
    ffmpeg: Option<Child>,
    live: Option<LiveVideo>,
    /// Zuletzt gewählte Linse; die Kamera behält sie auch über einen neuen Stream hinweg.
    /// shortcut: gilt für jede Quelle, auch die FPV-Kamera (`normal`); bei Bedarf je `video_id` führen.
    lens: String,
}

impl Pilot {
    fn new(args: Args, connection: &PilotConnection) -> Result<Self, String> {
        let (host, port) = broker_address(&connection.mqtt_host)?;
        let mut options = MqttOptions::new(format!("sim-{}", args.rc_sn), host, port);
        options.set_credentials(&connection.mqtt_user, &connection.mqtt_pass);
        options.set_keep_alive(Duration::from_secs(30));
        let (client, eventloop) = AsyncClient::new(options, 256);
        Ok(Self {
            services_topic: format!("thing/product/{}/services", args.rc_sn),
            args,
            client,
            eventloop,
            ffmpeg: None,
            live: None,
            lens: "wide".into(),
        })
    }

    /// Spielt den Flug ab, mit `--loop` immer wieder. Strg+C beendet sauber.
    async fn fly(&mut self) -> Result<(), String> {
        loop {
            let script = script(&self.args)?;
            let t0 = script.iter().find_map(|l| l.at_ms().ok()).unwrap_or(0);
            let started = Instant::now();
            for line in &script {
                let Ok(at_ms) = line.at_ms() else {
                    tracing::warn!(tst = %line.tst, "Zeitstempel unlesbar – Zeile übersprungen");
                    continue;
                };
                let offset = (at_ms - t0).max(0) as f64 / 1000.0 / self.args.speed;
                let due = started + Duration::from_secs_f64(offset);
                if self.wait_until(due).await == Wake::Interrupted {
                    return Ok(());
                }
                self.send(
                    &line.topic,
                    QoS::AtMostOnce,
                    line.payload.clone().into_bytes(),
                );
            }
            // Der Flug ist vorbei; ein weiterbestehender Stream hielte den Server auf „Livebild läuft“.
            self.stop_video();
            if !self.args.repeat {
                return Ok(());
            }
            tracing::info!(pause = ?PAUSE_BETWEEN_RUNS, "Flug zu Ende – nächster Durchlauf");
            if self.wait_until(Instant::now() + PAUSE_BETWEEN_RUNS).await == Wake::Interrupted {
                return Ok(());
            }
        }
    }

    /// Bedient bis zum Termin die MQTT-Verbindung: Services beantworten, nach jedem ConnAck abonnieren.
    async fn wait_until(&mut self, due: Instant) -> Wake {
        loop {
            tokio::select! {
                _ = tokio::time::sleep_until(due) => return Wake::Due,
                _ = tokio::signal::ctrl_c() => return Wake::Interrupted,
                event = self.eventloop.poll() => self.on_event(event).await,
            }
        }
    }

    async fn on_event(&mut self, event: Result<Event, ConnectionError>) {
        match event {
            Ok(Event::Incoming(Packet::ConnAck(_))) => {
                // Nach jedem Neuaufbau neu abonnieren, sonst kämen keine Livebild-Befehle mehr an.
                if let Err(e) = self
                    .client
                    .try_subscribe(&self.services_topic, QoS::AtLeastOnce)
                {
                    tracing::warn!(error = %e, "Abo der Services nicht möglich");
                }
                tracing::info!(rc = %self.args.rc_sn, "Simulator mit dem Broker verbunden");
            }
            Ok(Event::Incoming(Packet::Publish(p))) if p.topic == self.services_topic => {
                if let Some((reply, action)) = handle_service(&p.payload) {
                    let topic = format!("{}_reply", self.services_topic);
                    self.send(&topic, QoS::AtLeastOnce, reply);
                    self.apply(action);
                }
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, "MQTT getrennt – neuer Versuch");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::StartVideo { url, video_id } => {
                if self.args.video && !is_running(&mut self.ffmpeg) {
                    self.ffmpeg = start_ffmpeg(&url);
                }
                self.live = Some(LiveVideo {
                    video_id,
                    video_type: self.lens.clone(),
                });
                self.report_live_status();
            }
            Action::StopVideo => self.stop_video(),
            Action::ChangeLens { video_type } => {
                tracing::info!(%video_type, "Linse gewechselt (simuliert)");
                self.lens = video_type;
                if let Some(live) = &mut self.live {
                    live.video_type = self.lens.clone();
                    self.report_live_status();
                }
            }
            Action::None => {}
        }
    }

    /// Beendet das Testbild (`kill_on_drop`) und meldet den Stream ab, wie Pilot 2 beim Stoppen.
    /// Immer, auch ohne eigenen Stream: Nach einem harten Abbruch hält der Server ihn sonst für laufend.
    fn stop_video(&mut self) {
        self.ffmpeg = None;
        self.live = None;
        self.report_live_status();
    }

    fn report_live_status(&self) {
        let topic = format!("thing/product/{}/state", self.args.rc_sn);
        let payload = live_status(&self.args.rc_sn, self.live.as_ref());
        self.send(&topic, QoS::AtLeastOnce, payload);
    }

    /// Nie auf `publish(...).await` warten: Während `select!` pollt sonst niemand die
    /// Ereignisschleife, und eine volle Warteschlange hielte alles an (rumqttc-Falle).
    fn send(&self, topic: &str, qos: QoS, payload: Vec<u8>) {
        if let Err(e) = self.client.try_publish(topic, qos, false, payload) {
            tracing::warn!(error = %e, "Nachricht verworfen (Warteschlange voll oder getrennt)");
        }
    }

    /// Meldet den Stream ab und trennt erst, wenn alles Wartende draußen ist.
    async fn finish(&mut self) {
        self.stop_video();
        if self.client.try_disconnect().is_err() {
            return;
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while let Ok(Ok(event)) = tokio::time::timeout_at(deadline, self.eventloop.poll()).await {
            if event == Event::Outgoing(Outgoing::Disconnect) {
                return;
            }
        }
    }
}
