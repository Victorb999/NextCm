//! O passo em que vai a criação de um jogo (a barra do carregamento do Novo Jogo). Fica fora
//! da `Session`: a tela pergunta-o enquanto o comando `new_game` ainda corre.

use std::sync::Mutex;

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CreationProgress {
    /// O que está a fazer ("A preparar os calendários…"); vazio quando não está a criar nada.
    pub label: String,
    /// Quanto já fez, de 0 a 1.
    pub done: f64,
}

static PROGRESS: Mutex<Option<CreationProgress>> = Mutex::new(None);

/// Marca o passo atual (`done` de 0 a 1).
pub(crate) fn set(label: &str, done: f64) {
    if let Ok(mut p) = PROGRESS.lock() {
        *p = Some(CreationProgress { label: label.into(), done: done.clamp(0.0, 1.0) });
    }
}

/// Acabou (ou falhou): a barra deixa de ter passo.
pub(crate) fn clear() {
    if let Ok(mut p) = PROGRESS.lock() {
        *p = None;
    }
}

/// O passo de agora (vazio fora da criação de um jogo).
pub fn creation_progress() -> CreationProgress {
    PROGRESS.lock().ok().and_then(|p| p.clone()).unwrap_or_default()
}
