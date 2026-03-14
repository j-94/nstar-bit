use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State as AxumState,
    },
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use nstar_bit::autogenesis::{
    canonicalize_reference, canonical_symbol, relation_id, canonical_focus_id,
    inject_evidence, load_state, process_turn,
    EvidenceDelta, State,
};
use nstar_bit::state_sync::with_state_transaction;
use nstar_bit::manifest::{self, ManifestDispatch};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

#[derive(Clone)]
struct AppState {
    db: Arc<RwLock<State>>,
    path: PathBuf,
    /// Manifest-driven dispatch. None if manifest.yaml not found.
    manifest: Option<Arc<ManifestDispatch>>,
}

#[derive(Deserialize)]
struct BeliefQuery {
    source: Option<String>,
    target: Option<String>,
    relation: Option<String>,
    relation_id: Option<String>,
    declared_confidence: Option<f32>,
}

#[derive(Serialize)]
struct BeliefResponse {
    confidence: f32,
    evidence_for: u64,
    evidence_against: u64,
    support_set: Vec<String>,
    status: String,
    relation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    delusion_delta: Option<f32>,
}

#[derive(Deserialize)]
struct EvidencePayload {
    #[serde(flatten)]
    delta: EvidenceDelta,
    source_uri: String,
    #[serde(default)]
    meta: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct TurnPayload {
    /// Raw signal — what was said or happened
    text: String,
    /// OMNI synthesis — cross-signal reasoning that produced non-obvious conclusions.
    /// This is what the OMNI layer (LM) contributes: patterns across signals the user can't see.
    /// When present, this drives semantic concept extraction instead of raw word extraction.
    #[serde(default)]
    synthesis: String,
    /// Domain: "technical" | "life" | "business" | "learning" (default: "signal")
    #[serde(default = "default_domain")]
    domain: String,
    /// Source: where the signal came from (cli, webhook, voice, omni, git, calendar)
    #[serde(default)]
    source: String,
}

#[derive(Deserialize)]
struct SynthesizePayload {
    /// Conversation transcript or list of recent signals to synthesize across
    signals: Vec<String>,
    /// Optional context: what domain or focus area
    #[serde(default)]
    context: String,
}

fn default_domain() -> String { "signal".to_string() }

#[derive(Serialize)]
struct TurnResponse {
    turn: u64,
    domain: String,
    concepts_before: usize,
    concepts_after: usize,
    relations_before: usize,
    relations_after: usize,
    /// New concepts that landed this turn
    new_concepts: Vec<String>,
    /// Timestamp — proof that this signal was processed
    receipt_ts: u64,
}

#[tokio::main]
async fn main() {
    let state_file = std::env::var("NSTAR_STATE").unwrap_or_else(|_| "nstar-autogenesis/rust_state.json".to_string());
    let path = PathBuf::from(state_file);
    let mut state = load_state(&path).expect("Failed to load state");

    // Load manifest and seed primitives into state
    let manifest_dispatch: Option<Arc<ManifestDispatch>> = manifest::find_and_load(&path).map(|md| {
        md.seed_primitives(&mut state);
        Arc::new(md)
    });

    if manifest_dispatch.is_some() {
        println!("[manifest] loaded — deterministic dispatch active");
    } else {
        println!("[manifest] not found — all turns routed to OMNI (LM)");
    }

    let app_state = AppState {
        db: Arc::new(RwLock::new(state)),
        path,
        manifest: manifest_dispatch,
    };

    let app = Router::new()
        .route("/belief", get(get_belief))
        .route("/evidence", post(post_evidence))
        .route("/turn", post(post_turn))
        .route("/synthesize", post(post_synthesize))
        .route("/manifest", get(get_manifest))
        .route("/ws", get(ws_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Headless Epistemic API listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn get_belief(
    Query(query): Query<BeliefQuery>,
    AxumState(app): AxumState<AppState>,
) -> Result<Json<BeliefResponse>, StatusCode> {
    let db = app.db.read().await;
    let rel_id = if let Some(id) = &query.relation_id {
        canonical_focus_id(&canonicalize_reference(&db, id))
    } else if let (Some(s), Some(r), Some(t)) = (&query.source, &query.relation, &query.target) {
        relation_id(
            &canonicalize_reference(&db, s),
            &canonical_symbol(r),
            &canonicalize_reference(&db, t),
        )
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };

    if let Some(relation) = db.relations.get(&rel_id) {
        let mut delusion_delta = None;
        if let Some(dc) = query.declared_confidence {
            let delta = dc - relation.confidence;
            // delta can be negative if evidence-gated > declared, but typically it represents overconfidence
            delusion_delta = Some(delta.max(0.0));
        }

        Ok(Json(BeliefResponse {
            confidence: relation.confidence,
            evidence_for: relation.evidence_for,
            evidence_against: relation.evidence_against,
            support_set: relation.support_set.clone(),
            status: relation.status.clone(),
            relation_id: relation.id.clone(),
            delusion_delta,
        }))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn post_evidence(
    AxumState(app): AxumState<AppState>,
    Json(payload): Json<EvidencePayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut db = app.db.write().await;
    match inject_evidence(&mut db, payload.delta, payload.source_uri, payload.meta) {
        Ok(rel_id) => {
            let db_clone = db.clone();
            let path = app.path.clone();
            with_state_transaction(&path, |state| {
                *state = db_clone.clone();
                Ok(())
            })
            .map_err(|e| {
                eprintln!("Failed to save state atomically: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            Ok(Json(serde_json::json!({
                "status": "success",
                "relation_id": rel_id,
            })))
        }
        Err(_) => {
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

/// POST /turn — submit any signal from any domain.
/// The manifest updates immediately. Receipt proves it happened.
async fn post_turn(
    AxumState(app): AxumState<AppState>,
    Json(payload): Json<TurnPayload>,
) -> Result<Json<TurnResponse>, StatusCode> {
    let mut db = app.db.write().await;

    let concepts_before = db.concepts.len();
    let relations_before = db.relations.len();
    let concepts_before_set: std::collections::HashSet<String> =
        db.concepts.keys().cloned().collect();

    // If synthesis provided, use it as the semantic signal — it contains cross-signal reasoning.
    // Otherwise fall back to raw text. Synthesis is what OMNI contributes.
    let semantic_signal = if !payload.synthesis.is_empty() {
        format!("[domain:{}] [source:{}] [synthesis] {}\n[raw] {}",
            payload.domain, payload.source, payload.synthesis, payload.text)
    } else {
        format!("[domain:{}] [source:{}] {}", payload.domain, payload.source, payload.text)
    };

    let tagged = semantic_signal.clone();

    // Try manifest deterministic dispatch first.
    // If an edge matches → fast path (no LM, immediate receipt).
    // If no edge matches → OMNI fires via background autogenesis binary.
    let manifest_matched = if let Some(ref md) = app.manifest {
        if let Some(result) = md.try_dispatch(&tagged) {
            md.apply_ops(&mut db, &result);
            true
        } else {
            false
        }
    } else {
        false
    };

    let event = process_turn(&mut db, &tagged)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let receipt_ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let db_clone = db.clone();
    let path = app.path.clone();
    with_state_transaction(&path, |state| {
        *state = db_clone.clone();
        Ok(())
    })
    .map_err(|e| {
        eprintln!("atomic save failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // If manifest did not match → OMNI fires: spawn LM-enhanced turn in background.
    // If manifest matched → deterministic path handled it; skip LM overhead.
    if !manifest_matched {
        let state_path = app.path.clone();
        let signal_text = tagged.clone();
        tokio::spawn(async move {
            let _ = tokio::process::Command::new("./target/debug/autogenesis")
                .args(["--state", state_path.to_str().unwrap_or(""), "turn", &signal_text])
                .output()
                .await;
        });
    }

    let concepts_after = db.concepts.len();
    let relations_after = db.relations.len();
    let new_concepts: Vec<String> = db.concepts.keys()
        .filter(|k| !concepts_before_set.contains(*k))
        .cloned()
        .collect();

    Ok(Json(TurnResponse {
        turn: event.turn as u64,
        domain: payload.domain,
        concepts_before,
        concepts_after,
        relations_before,
        relations_after,
        new_concepts,
        receipt_ts,
    }))
}

/// POST /synthesize — feed a list of signals, get cross-signal synthesis back as a turn.
/// This is the OMNI layer: finds patterns across signals the user can't see from inside them.
/// The synthesis itself becomes a high-weight turn in the graph.
async fn post_synthesize(
    AxumState(app): AxumState<AppState>,
    Json(payload): Json<SynthesizePayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if payload.signals.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let state_path = app.path.clone();
    let signals_joined = payload.signals.join("\n---\n");
    let context = payload.context.clone();

    // Spawn LM synthesis turn — the binary does the cross-signal reasoning
    // and commits the result to the graph as a high-quality semantic turn
    let synthesis_prompt = format!(
        "[synthesis-request] [context: {}] Across these signals, find the non-obvious pattern \
         — what the builder keeps doing that they cannot see from inside. Name the attractor. \
         Name what keeps failing. Name what survives. This is OMNI layer reasoning.\n\nSIGNALS:\n{}",
        if context.is_empty() { "general" } else { &context },
        signals_joined
    );

    tokio::spawn(async move {
        let _ = tokio::process::Command::new("./target/debug/autogenesis")
            .args(["--state", state_path.to_str().unwrap_or(""), "turn", &synthesis_prompt])
            .output()
            .await;
    });

    Ok(Json(serde_json::json!({
        "status": "synthesis_queued",
        "signal_count": payload.signals.len(),
        "note": "LM synthesis running — cross-signal patterns will land in /manifest within ~30s"
    })))
}

/// GET /manifest — current live manifest: what the system knows right now.
/// This is the proof that life-domain signals are changing the system.
async fn get_manifest(
    AxumState(app): AxumState<AppState>,
) -> Json<serde_json::Value> {
    let db = app.db.read().await;

    // Surface live concepts by domain tag, sorted by mention count
    let mut concepts: Vec<serde_json::Value> = db.concepts.values()
        .filter(|c| c.status != "archived")
        .map(|c| serde_json::json!({
            "id": c.id,
            "label": c.label,
            "summary": c.summary,
            "first_seen_turn": c.first_seen_turn,
            "mention_count": c.mention_count,
            "status": c.status,
        }))
        .collect();
    concepts.sort_by(|a, b| {
        b["mention_count"].as_u64().unwrap_or(0)
            .cmp(&a["mention_count"].as_u64().unwrap_or(0))
    });

    Json(serde_json::json!({
        "turn": db.turn,
        "updated_at": db.updated_at,
        "summary": {
            "concepts": db.concepts.len(),
            "relations": db.relations.len(),
            "evidence": db.evidence_log.len(),
        },
        "active_focus": db.active_focus,
        "universal_primitives": ["signal", "pattern", "receipt", "void", "manifest", "turn"],
        "live_concepts": concepts,
    }))
}

async fn ws_handler(ws: WebSocketUpgrade, AxumState(app): AxumState<AppState>) -> axum::response::Response {
    ws.on_upgrade(|socket| handle_socket(socket, app))
}

async fn handle_socket(mut socket: WebSocket, app: AppState) {
    while let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            if let Message::Text(_) = msg {
                let db = app.db.read().await;
                let summary = serde_json::to_string(&nstar_bit::autogenesis::monitor(&db)).unwrap_or_default();
                let _ = socket.send(Message::Text(summary.into())).await;
            }
        } else {
            break;
        }
    }
}
