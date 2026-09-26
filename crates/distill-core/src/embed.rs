//! Local embeddings for recall and for spotting duplicate topics (spec D3, P4).
//!
//! The model runs in-process on ONNX Runtime through fastembed. Nothing is downloaded
//! implicitly: `distill model pull` fetches the model into `<data dir>/models` once, and
//! until then every caller falls back to keyword recall. The model never sees anything but
//! note titles and questions already on this device.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};

use crate::config::Dirs;
use crate::error::{Error, IoContext, Result};

/// Stored with every vector, so switching models later re-embeds instead of mixing spaces.
pub const MODEL_KEY: &str = "paraphrase-multilingual-minilm-l12-v2-q";
/// Hugging Face repository the model comes from.
pub const MODEL_REPO: &str = "Qdrant/paraphrase-multilingual-MiniLM-L12-v2-onnx-Q";
const MODEL: EmbeddingModel = EmbeddingModel::ParaphraseMLMiniLML12V2Q;
const BATCH: usize = 32;

pub struct Embedder {
    model: Mutex<TextEmbedding>,
}

/// One loaded model per process: loading takes a few hundred milliseconds, and the MCP
/// server and web server answer many calls.
static LOADED: Mutex<Option<(PathBuf, Arc<Embedder>)>> = Mutex::new(None);

impl Embedder {
    pub fn models_dir(dirs: &Dirs) -> PathBuf {
        dirs.data_dir.join("models")
    }

    fn ready_marker(dirs: &Dirs) -> PathBuf {
        Self::models_dir(dirs).join(format!("{MODEL_KEY}.ready"))
    }

    /// Whether `distill model pull` has completed on this device.
    pub fn installed(dirs: &Dirs) -> bool {
        Self::ready_marker(dirs).is_file()
    }

    /// Downloads the model (about 0.25 GB) unless it is already cached, then marks it ready.
    pub fn pull(dirs: &Dirs, show_progress: bool) -> Result<()> {
        let dir = Self::models_dir(dirs);
        std::fs::create_dir_all(&dir).at(&dir)?;
        let embedder = Self::init(dirs, show_progress)?;
        embedder.embed(&["distill".to_string()])?;
        let marker = Self::ready_marker(dirs);
        std::fs::write(&marker, format!("{MODEL_REPO}\n")).at(&marker)?;
        Ok(())
    }

    /// The model, when it has been pulled; `None` means callers use keyword recall.
    pub fn load(dirs: &Dirs) -> Result<Option<Arc<Self>>> {
        if !Self::installed(dirs) {
            return Ok(None);
        }
        let dir = Self::models_dir(dirs);
        let mut loaded = LOADED
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some((path, embedder)) = loaded.as_ref()
            && *path == dir
        {
            return Ok(Some(Arc::clone(embedder)));
        }
        let embedder = Arc::new(Self::init(dirs, false)?);
        *loaded = Some((dir, Arc::clone(&embedder)));
        Ok(Some(embedder))
    }

    fn init(dirs: &Dirs, show_progress: bool) -> Result<Self> {
        let options = TextInitOptions::new(MODEL)
            .with_cache_dir(Self::models_dir(dirs))
            .with_show_download_progress(show_progress);
        let model = TextEmbedding::try_new(options).map_err(|source| Error::Embedding {
            action: format!(
                "loading {MODEL_REPO} from {}",
                Self::models_dir(dirs).display()
            ),
            source,
        })?;
        Ok(Self {
            model: Mutex::new(model),
        })
    }

    /// Unit-length vectors, one per text, in order.
    pub fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut model = self
            .model
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let vectors = model
            .embed(texts, Some(BATCH))
            .map_err(|source| Error::Embedding {
                action: format!("embedding {} text(s)", texts.len()),
                source,
            })?;
        Ok(vectors.into_iter().map(normalized).collect())
    }
}

impl Embedder {
    pub fn embed_one(&self, text: &str) -> Result<Vec<f32>> {
        self.embed(&[text.to_string()])?
            .pop()
            .ok_or_else(|| Error::Embedding {
                action: "embedding one text".into(),
                source: fastembed::Error::Other("the model returned no vector".into()),
            })
    }
}

fn normalized(mut v: Vec<f32>) -> Vec<f32> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        v.iter_mut().for_each(|x| *x /= norm);
    }
    v
}

/// Cosine similarity of two unit-length vectors.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// The mean direction of several unit vectors, itself unit length.
pub fn centroid<'a>(vectors: impl IntoIterator<Item = &'a [f32]>) -> Option<Vec<f32>> {
    let mut sum: Option<Vec<f32>> = None;
    for v in vectors {
        match sum.as_mut() {
            None => sum = Some(v.to_vec()),
            Some(s) => s.iter_mut().zip(v).for_each(|(a, b)| *a += b),
        }
    }
    sum.map(normalized)
}

/// What a note is embedded as: the question it answers, not the answer.
pub fn note_text(title: &str, question: &str) -> String {
    format!("{title}\n{question}")
}

/// Vectors are stored as little-endian `f32` bytes.
pub fn to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

pub fn from_blob(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vectors_round_trip_and_compare() {
        let a = normalized(vec![3.0, 4.0]);
        assert_eq!(from_blob(&to_blob(&a)), a);
        assert!((cosine(&a, &a) - 1.0).abs() < 1e-6);
        let c = centroid([[1.0, 0.0].as_slice(), [0.0, 1.0].as_slice()]).unwrap_or_default();
        assert!((c[0] - c[1]).abs() < 1e-6);
        assert!((cosine(&c, &c) - 1.0).abs() < 1e-6);
    }
}
