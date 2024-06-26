use std::sync::{Arc, Mutex, MutexGuard};

use candle_core::{quantized::QTensor, Result, Tensor};
use candle_nn::{
    layer_norm::{RmsNormNonQuantized, RmsNormQuantized},
    Module, VarBuilder,
};

use crate::get_mut_arcmutex;

pub(crate) mod gemma;
pub(crate) mod llama;
pub(crate) mod mistral;
pub(crate) mod mixtral;
pub(crate) mod phi2;
pub(crate) mod quantized_llama;

pub type LayerCaches = Vec<Option<(Tensor, Tensor)>>;

#[derive(Debug, Clone)]
pub struct Cache {
    cache: Arc<Mutex<LayerCaches>>,
    xlora_cache: Option<Arc<Mutex<LayerCaches>>>,
    scalings_cache: Option<Arc<Mutex<Option<Tensor>>>>,
}

impl Cache {
    pub(crate) fn new(len: usize, is_xlora: bool) -> Self {
        Self {
            cache: Arc::new(Mutex::new(vec![None; len])),
            xlora_cache: if is_xlora {
                Some(Arc::new(Mutex::new(vec![None; len])))
            } else {
                None
            },
            scalings_cache: if is_xlora {
                Some(Arc::new(Mutex::new(None)))
            } else {
                None
            },
        }
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, LayerCaches> {
        get_mut_arcmutex!(self.cache)
    }

    /// # Panics
    /// If there is no xlora cache
    pub(crate) fn xlora_lock(&self) -> MutexGuard<'_, LayerCaches> {
        get_mut_arcmutex!(self.xlora_cache.as_ref().unwrap())
    }

    /// # Panics
    /// If there is no xlora cache
    pub(crate) fn get_scalings_cache(&self) -> MutexGuard<'_, Option<Tensor>> {
        get_mut_arcmutex!(self.scalings_cache.as_ref().unwrap())
    }

    pub(crate) fn is_xlora(&self) -> bool {
        self.xlora_cache.is_some()
    }
}

#[derive(Debug, Clone)]
pub struct RmsNorm {
    inner: candle_nn::RmsNorm<RmsNormNonQuantized>,
}

impl RmsNorm {
    pub fn new(size: usize, eps: f64, vb: VarBuilder) -> Result<Self> {
        let inner = candle_nn::rms_norm_non_quant(size, eps, vb)?;
        Ok(Self { inner })
    }
}

impl Module for RmsNorm {
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        self.inner.forward(x)
    }
}

#[derive(Debug, Clone)]
pub struct QRmsNorm {
    inner: candle_nn::RmsNorm<RmsNormQuantized>,
}

impl QRmsNorm {
    pub fn new(scale: QTensor, eps: f32) -> Result<Self> {
        let scale = scale.dequantize(&scale.device())?;
        let inner = candle_nn::RmsNorm::<RmsNormQuantized>::new(scale, eps as f64);
        Ok(Self { inner })
    }

    pub fn forward(&self, x: &Tensor) -> Result<Tensor> {
        self.inner.forward(x)
    }
}

#[cfg(feature = "flash-attn")]
pub fn flash_attn(
    q: &Tensor,
    k: &Tensor,
    v: &Tensor,
    softmax_scale: f32,
    causal: bool,
) -> Result<Tensor> {
    candle_flash_attn::flash_attn(q, k, v, softmax_scale, causal)
}

#[cfg(not(feature = "flash-attn"))]
pub fn flash_attn(_: &Tensor, _: &Tensor, _: &Tensor, _: f32, _: bool) -> Result<Tensor> {
    unimplemented!("Compile with '--features flash-attn'")
}

pub fn verify_sanity_gguf(arch: &str, expected_arch: &str) -> Result<()> {
    if arch != expected_arch {
        candle_core::bail!("Expected `{expected_arch}` architecture, got `{arch}`.");
    }
    Ok(())
}
