//! Der Lage-Aktor: besitzt `Situation`, führt ihre Effekte gegen Speicher und Hub aus und ist der
//! einzige Schreiber von Flügen. Alle Zugriffe laufen über einen Befehlskanal, damit Observations
//! und Abfragen in Reihenfolge bleiben.

use std::collections::BTreeMap;

use sqlx::SqlitePool;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::task::JoinHandle;

use crate::contract::{GatewayState, LiveEvent, SituationSnapshot, StreamState, TrackPoint};
use crate::error::AppError;
use crate::situation::observation::{Observed, StreamCommand};
use crate::situation::state::{Config, Effect, ResumeFlight, Situation};
use crate::store;

use super::Clock;

const BROADCAST_CAPACITY: usize = 1024;
const FLUSH_POINTS_AT: usize = 20;

#[expect(
    clippy::large_enum_variant,
    reason = "Observe ist fast jeder Befehl; ein Box spart keinen Platz im Kanal, kostet aber je Nachricht eine Allokation"
)]
enum Command {
    Observe(Observed),
    Subscribe(oneshot::Sender<(SituationSnapshot, broadcast::Receiver<LiveEvent>)>),
    Snapshot(oneshot::Sender<SituationSnapshot>),
    Gateway(String, oneshot::Sender<Option<GatewayState>>),
    Stream(String, oneshot::Sender<Option<StreamState>>),
    /// Livebild-Befehl beginnen; `false`, wenn der Zustand ihn nicht zulässt.
    Begin(String, StreamCommand, oneshot::Sender<bool>),
    SetConfig(Config),
}

#[derive(Clone)]
pub struct SituationHandle {
    tx: mpsc::Sender<Command>,
}

fn gone() -> AppError {
    AppError::Unavailable("Lage wird gerade neu gestartet".into())
}

impl SituationHandle {
    pub async fn observe(&self, o: Observed) {
        if self.tx.send(Command::Observe(o)).await.is_err() {
            tracing::debug!("Lage-Aktor beendet – Observation verworfen");
        }
    }

    /// Ein einfacher Kanal für Bausteine, die `runtime` nicht kennen sollen (MQTT-Sitzung). Leitet in
    /// den einen Befehlskanal weiter: Observations bleiben vor späteren Abfragen in der Reihe.
    pub fn observer(&self) -> mpsc::Sender<Observed> {
        let (tx, mut rx) = mpsc::channel::<Observed>(1024);
        let handle = self.clone();
        tokio::spawn(async move {
            while let Some(o) = rx.recv().await {
                handle.observe(o).await;
            }
        });
        tx
    }

    async fn ask<T>(
        &self,
        make: impl FnOnce(oneshot::Sender<T>) -> Command,
    ) -> Result<T, AppError> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(make(tx)).await.map_err(|_| gone())?;
        rx.await.map_err(|_| gone())
    }

    /// Snapshot und Empfänger entstehen im selben Schritt des Aktors: kein Ereignis fehlt, keins doppelt.
    pub async fn subscribe(
        &self,
    ) -> Result<(SituationSnapshot, broadcast::Receiver<LiveEvent>), AppError> {
        self.ask(Command::Subscribe).await
    }

    pub async fn snapshot(&self) -> Result<SituationSnapshot, AppError> {
        self.ask(Command::Snapshot).await
    }

    pub async fn gateway(&self, sn: &str) -> Result<Option<GatewayState>, AppError> {
        let sn = sn.to_string();
        self.ask(|tx| Command::Gateway(sn, tx)).await
    }

    pub async fn stream(&self, sn: &str) -> Result<Option<StreamState>, AppError> {
        let sn = sn.to_string();
        self.ask(|tx| Command::Stream(sn, tx)).await
    }

    /// Prüft und setzt den Beginn eines Livebild-Befehls in einem Schritt des Aktors.
    pub async fn begin(&self, sn: &str, command: StreamCommand) -> Result<bool, AppError> {
        let sn = sn.to_string();
        self.ask(|tx| Command::Begin(sn, command, tx)).await
    }

    pub async fn set_config(&self, config: Config) {
        if self.tx.send(Command::SetConfig(config)).await.is_err() {
            tracing::debug!("Lage-Aktor beendet – Einstellung verworfen");
        }
    }
}

struct Actor {
    situation: Situation,
    pool: SqlitePool,
    events: broadcast::Sender<LiveEvent>,
    /// Gepufferte Spurpunkte: `(flight_id, segment, point)`.
    points: Vec<(i64, u32, TrackPoint)>,
    live_requests: mpsc::UnboundedSender<String>,
    clock: Clock,
}

/// Lädt offene Flüge als `ResumeFlight` und startet den Aktor. Er endet, sobald `shutdown` auf `true`
/// steht oder der Sender wegfällt; vorher schreibt er die gepufferten Punkte. Offene Flüge bleiben
/// offen und werden beim nächsten Start fortgesetzt (Spec §5).
pub async fn start(
    pool: SqlitePool,
    config: Config,
    clock: Clock,
    live_requests: mpsc::UnboundedSender<String>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(SituationHandle, JoinHandle<()>), sqlx::Error> {
    let resume: Vec<ResumeFlight> = store::flights::open(&pool)
        .await?
        .into_iter()
        .map(|(flight, segments)| ResumeFlight { flight, segments })
        .collect();
    if !resume.is_empty() {
        tracing::info!(count = resume.len(), "laufende Flüge werden fortgesetzt");
    }
    let next_id = store::flights::next_id(&pool).await?;
    let (events, _) = broadcast::channel(BROADCAST_CAPACITY);
    let mut actor = Actor {
        situation: Situation::new(config, next_id, resume),
        pool,
        events,
        points: Vec::new(),
        live_requests,
        clock,
    };
    let (tx, mut rx) = mpsc::channel::<Command>(4096);

    let task = tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // Ein fallender Sender gilt ebenfalls als Stopp.
        let stopped = async move {
            let _ = shutdown.wait_for(|stop| *stop).await;
        };
        tokio::pin!(stopped);
        loop {
            tokio::select! {
                cmd = rx.recv() => match cmd {
                    Some(cmd) => actor.handle(cmd).await,
                    None => break,
                },
                _ = tick.tick() => {
                    let fx = actor.situation.tick((actor.clock)());
                    actor.run(fx).await;
                    actor.flush().await;
                }
                () = &mut stopped => break,
            }
        }
        // Erst den Rest der Warteschlange verarbeiten, dann den Puffer schreiben.
        rx.close();
        while let Ok(cmd) = rx.try_recv() {
            actor.handle(cmd).await;
        }
        actor.flush().await;
        tracing::info!("Lage-Aktor beendet");
    });
    Ok((SituationHandle { tx }, task))
}

impl Actor {
    async fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::Observe(o) => {
                let fx = self.situation.apply(o);
                self.run(fx).await;
            }
            Command::Subscribe(reply) => {
                let rx = self.events.subscribe();
                let _ = reply.send((self.situation.snapshot((self.clock)()), rx));
            }
            Command::Snapshot(reply) => {
                let _ = reply.send(self.situation.snapshot((self.clock)()));
            }
            Command::Gateway(sn, reply) => {
                let _ = reply.send(self.situation.gateway(&sn).cloned());
            }
            Command::Stream(sn, reply) => {
                let _ = reply.send(self.situation.stream(&sn).cloned());
            }
            Command::Begin(sn, command, reply) => {
                let fx = self.situation.begin_command(&sn, &command, (self.clock)());
                let accepted = fx.is_some();
                self.run(fx.unwrap_or_default()).await;
                let _ = reply.send(accepted);
            }
            Command::SetConfig(c) => self.situation.set_config(c),
        }
    }

    async fn run(&mut self, effects: Vec<Effect>) {
        for e in effects {
            match e {
                Effect::Publish(event) => {
                    let _ = self.events.send(event); // ohne Empfänger ist das kein Fehler
                }
                Effect::InsertFlight(f) => {
                    if let Err(err) = store::flights::insert(&self.pool, &f).await {
                        tracing::error!(error = %err, flight = f.id, "Flug nicht gespeichert");
                    }
                }
                Effect::AppendTrackPoint {
                    flight_id,
                    segment,
                    point,
                } => {
                    self.points.push((flight_id, segment, point));
                    if self.points.len() >= FLUSH_POINTS_AT {
                        self.flush().await;
                    }
                }
                Effect::UpdateFlight(f) => {
                    // Die Spur steht vollständig in der DB, bevor der Flug als beendet gilt.
                    self.flush().await;
                    if let Err(err) = store::flights::update(&self.pool, &f).await {
                        tracing::error!(error = %err, flight = f.id, "Flugende nicht gespeichert");
                    }
                }
                Effect::RequestLiveStart { gateway_sn } => {
                    if self.live_requests.send(gateway_sn).is_err() {
                        tracing::warn!("Livebild-Auftrag ohne Empfänger verworfen");
                    }
                }
            }
        }
    }

    async fn flush(&mut self) {
        if self.points.is_empty() {
            return;
        }
        let mut by_flight: BTreeMap<i64, Vec<(u32, TrackPoint)>> = BTreeMap::new();
        for (id, segment, point) in std::mem::take(&mut self.points) {
            by_flight.entry(id).or_default().push((segment, point));
        }
        for (id, points) in by_flight {
            if let Err(err) = store::flights::append_points(&self.pool, id, &points).await {
                // Verlust einzelner Punkte ist hinnehmbar; der Flug selbst steht schon in der DB.
                tracing::error!(error = %err, flight = id, count = points.len(), "Spurpunkte nicht gespeichert");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::FlightEndReason;
    use crate::dji::record::{RecordLine, read_lines};
    use crate::dji::translate::Translator;
    use crate::sim::synth::FIXTURE_PATH;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::time::Duration;

    fn fixture() -> Vec<RecordLine> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH);
        read_lines(std::io::BufReader::new(std::fs::File::open(path).unwrap()))
            .map(Result::unwrap)
            .collect()
    }

    /// Uhr, die der Replay vorstellt – sonst schlösse `tick` den Flug nach Wanduhr.
    fn replay_clock() -> (Clock, Arc<AtomicI64>) {
        let now = Arc::new(AtomicI64::new(0));
        let n = now.clone();
        (Arc::new(move || n.load(Ordering::SeqCst)), now)
    }

    async fn feed(
        handle: &SituationHandle,
        now: &AtomicI64,
        translator: &mut Translator,
        lines: &[RecordLine],
    ) {
        for line in lines {
            let at = line.at_ms().unwrap();
            now.store(at, Ordering::SeqCst);
            for o in translator
                .handle(at, &line.topic, line.payload.as_bytes())
                .observations
            {
                handle.observe(o).await;
            }
        }
        handle.snapshot().await.unwrap(); // wartet, bis der Aktor alles verarbeitet hat
    }

    async fn run_until_stopped(stop: watch::Sender<bool>, task: JoinHandle<()>) {
        stop.send(true).unwrap();
        task.await.unwrap();
    }

    fn stored_points(segments: &[Vec<TrackPoint>]) -> usize {
        segments.iter().map(Vec::len).sum()
    }

    #[tokio::test]
    async fn replay_stores_one_landed_flight_with_its_track() {
        let pool = crate::db::test_pool().await;
        let (clock, now) = replay_clock();
        let (live_tx, mut live_rx) = mpsc::unbounded_channel();
        let (stop, stop_rx) = watch::channel(false);
        let config = Config {
            auto_live_on_takeoff: true,
        };
        let (handle, task) = start(pool.clone(), config, clock, live_tx, stop_rx)
            .await
            .unwrap();
        feed(&handle, &now, &mut Translator::new(), &fixture()).await;
        run_until_stopped(stop, task).await;

        let flights = store::flights::list(&pool, 10).await.unwrap();
        assert_eq!(flights.len(), 1);
        assert_eq!(flights[0].end_reason, Some(FlightEndReason::Landed));
        let segments = store::flights::segments(&pool, flights[0].id)
            .await
            .unwrap();
        assert_eq!(segments.len(), 2);
        assert!(stored_points(&segments) > 60);
        assert_eq!(live_rx.try_recv().unwrap(), "RCSIM0001");
    }

    #[tokio::test]
    async fn restart_mid_flight_continues_same_flight() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("suchlage.db");
        let lines = fixture();
        let half = lines.len() / 2;

        for (part, slice) in [(0, &lines[..half]), (1, &lines[half..])] {
            let pool = crate::db::connect(&path).await.unwrap();
            crate::db::migrate(&pool).await.unwrap();
            let (clock, now) = replay_clock();
            let (live_tx, _live_rx) = mpsc::unbounded_channel();
            let (stop, stop_rx) = watch::channel(false);
            let config = Config {
                auto_live_on_takeoff: false,
            };
            let (handle, task) = start(pool.clone(), config, clock, live_tx, stop_rx)
                .await
                .unwrap();
            // Ein neuer Prozess hat auch einen neuen Übersetzer (kein Merge-Stand, keine Topologie).
            feed(&handle, &now, &mut Translator::new(), slice).await;
            // Das Füttern dauert Millisekunden: Vor dem Stopp schreibt höchstens die Bündelgrenze.
            let running = if part == 0 {
                let snapshot = handle.snapshot().await.unwrap();
                let [active] = snapshot.active_flights.as_slice() else {
                    panic!("genau ein laufender Flug erwartet");
                };
                Some((active.flight.id, stored_points(&active.segments)))
            } else {
                None
            };
            run_until_stopped(stop, task).await;
            if let Some((id, fed)) = running {
                assert_ne!(
                    fed % FLUSH_POINTS_AT,
                    0,
                    "sonst bewiese schon die Bündelgrenze den Schreibvorgang"
                );
                assert_eq!(
                    store::flights::open(&pool).await.unwrap().len(),
                    1,
                    "Flug läuft beim Herunterfahren"
                );
                let segments = store::flights::segments(&pool, id).await.unwrap();
                assert_eq!(
                    stored_points(&segments),
                    fed,
                    "Herunterfahren schreibt die gepufferten Spurpunkte"
                );
            }
            pool.close().await;
        }

        let pool = crate::db::connect(&path).await.unwrap();
        let flights = store::flights::list(&pool, 10).await.unwrap();
        assert_eq!(flights.len(), 1, "kein zweiter Flug nach dem Neustart");
        assert_eq!(flights[0].end_reason, Some(FlightEndReason::Landed));
        let segments = store::flights::segments(&pool, flights[0].id)
            .await
            .unwrap();
        assert!(
            stored_points(&segments) > 60,
            "Spur beider Prozesse steht in einem Flug"
        );
    }

    #[tokio::test]
    async fn subscribe_delivers_snapshot_then_live_events() {
        let pool = crate::db::test_pool().await;
        let (clock, now) = replay_clock();
        let (live_tx, _) = mpsc::unbounded_channel();
        let (_stop, stop_rx) = watch::channel(false);
        let config = Config {
            auto_live_on_takeoff: false,
        };
        let (handle, _task) = start(pool, config, clock, live_tx, stop_rx).await.unwrap();
        let (snapshot, mut rx) = handle.subscribe().await.unwrap();
        assert!(snapshot.gateways.is_empty());
        feed(&handle, &now, &mut Translator::new(), &fixture()[..1]).await;
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("Ereignis nach dem Snapshot")
            .unwrap();
        assert!(matches!(event, LiveEvent::Gateway { .. }));
    }

    #[tokio::test]
    async fn buffered_points_reach_the_store_within_a_second() {
        let pool = crate::db::test_pool().await;
        let (clock, now) = replay_clock();
        let (live_tx, _) = mpsc::unbounded_channel();
        let (_stop, stop_rx) = watch::channel(false);
        let config = Config {
            auto_live_on_takeoff: false,
        };
        let (handle, _task) = start(pool.clone(), config, clock, live_tx, stop_rx)
            .await
            .unwrap();
        feed(&handle, &now, &mut Translator::new(), &fixture()[..40]).await;

        let snapshot = handle.snapshot().await.unwrap();
        let [active] = snapshot.active_flights.as_slice() else {
            panic!("genau ein laufender Flug erwartet");
        };
        let fed = stored_points(&active.segments);
        assert_ne!(
            fed % FLUSH_POINTS_AT,
            0,
            "sonst bewiese schon die Bündelgrenze den Schreibvorgang"
        );
        // Ohne Herunterfahren und ohne Flugende: Nur der Sekundentakt schreibt den Rest.
        // Abfragen statt fest warten: ein ausgelasteter Testlauf verzögert den Takt.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        let stored = loop {
            let segments = store::flights::segments(&pool, active.flight.id)
                .await
                .unwrap();
            let stored = stored_points(&segments);
            if stored == fed || tokio::time::Instant::now() >= deadline {
                break stored;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        assert_eq!(stored, fed);
    }
}
