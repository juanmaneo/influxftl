// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Spectral analysis of Geo Time Series: the Fourier transform (`FFT`, `FFTAP`, `IFFT`,
//! `FFTWINDOW`) and the discrete wavelet transform (`FDWT`, `IDWT`, `DWTSPLIT`).

use std::f64::consts::PI;
use std::rc::Rc;

use super::Entry;
use super::args::fail;
use super::gts::gts_or_list_try;
use super::time::TIME_UNITS_PER_S;
use crate::error::Result;
use crate::error::fault::IllegalArgumentError;
use crate::error::series::SeriesError;
use crate::fft::{self, Complex};
use crate::gts::{GeoTimeSerie, NO_ELEVATION, NO_LOCATION, Type, Values};
use crate::jmath;
use crate::selector::build_selector_ordered;
use crate::stack::Stack;
use crate::value::Value;
use crate::wavelet::Wavelet;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("FFT", Entry::Stack(|s| fft_function(s, true))),
    ("FFTAP", Entry::Stack(|s| fft_function(s, false))),
    ("IFFT", Entry::Stack(ifft)),
    ("FFTWINDOW", Entry::Stack(fftwindow)),
    ("FDWT", Entry::Stack(fdwt)),
    ("IDWT", Entry::Stack(idwt)),
    ("DWTSPLIT", Entry::Stack(dwtsplit)),
];

/// Whether a series holds numbers (`LONG` or `DOUBLE` values).
fn is_numeric(gts: &GeoTimeSerie) -> bool {
    matches!(gts.type_of(), Type::Long | Type::Double)
}

/// The numbers of a series as doubles (`((Number) value).doubleValue()`): a series of any
/// other type holds none.
pub(super) fn doubles(gts: &GeoTimeSerie) -> Vec<f64> {
    match &gts.values {
        Values::Long(values) => values.iter().map(|&value| value as f64).collect(),
        Values::Double(values) => values.clone(),
        Values::Undefined | Values::Boolean(_) | Values::String(_) => Vec::new(),
    }
}

/// A series of `DOUBLE` values at ticks `0, 1, 2…`, named and labeled like `source`.
fn spectrum(source: &GeoTimeSerie, values: impl Iterator<Item = f64>) -> GeoTimeSerie {
    let mut series = GeoTimeSerie::new();
    series.metadata.name.clone_from(&source.metadata.name);
    series.metadata.labels.clone_from(&source.metadata.labels);
    for (tick, value) in values.enumerate() {
        series.set_value(
            tick as i64,
            NO_LOCATION,
            NO_ELEVATION,
            &Value::Double(value),
            false,
        );
    }
    series
}

/// A series like `source` (metadata, bucketization, ticks, locations and elevations) whose
/// values are the given doubles.
fn with_doubles(source: &GeoTimeSerie, values: &[f64]) -> GeoTimeSerie {
    let mut series = source.clone_empty();
    for (idx, value) in values.iter().enumerate() {
        series.set_value(
            source.tick_at(idx),
            source.location_at(idx),
            source.elevation_at(idx),
            &Value::Double(*value),
            false,
        );
    }
    series
}

/// `FFT.fft`: the transform of a numeric, bucketized series whose ticks are all 1 bucket
/// apart, as `[ real imaginary frequency ]` (`complex`) or `[ amplitude phase frequency ]`,
/// the frequency being that of the first coefficient after the mean. The series is sorted.
fn fft_series(gts: &mut GeoTimeSerie, complex: bool) -> Result<Value> {
    if !is_numeric(gts) {
        return Err(SeriesError::FftNotNumeric.into());
    }
    if !gts.is_bucketized() {
        return Err(SeriesError::FftNotBucketized.into());
    }
    gts.sort(false);
    let span = gts.bucketspan;
    let boundary = gts.lastbucket % span;
    // Warp 10 starts from `Long.MAX_VALUE`: a first tick of that value is a duplicate.
    let mut last_tick = i64::MAX;
    for &tick in &gts.ticks {
        if tick % span != boundary {
            return Err(SeriesError::TickOffBucketBoundary.into());
        }
        if tick == last_tick {
            return Err(SeriesError::FftDuplicateTick.into());
        }
        if last_tick != i64::MAX && tick.wrapping_sub(last_tick) != span {
            return Err(SeriesError::DisplacedTick.into());
        }
        last_tick = tick;
    }
    let values = doubles(gts);
    if values.is_empty() {
        return Err(IllegalArgumentError::TransformLength.into());
    }
    let data = fft::forward_real(&values);
    let (first, second) = if complex {
        (
            spectrum(gts, data.iter().map(|c| c.re)),
            spectrum(gts, data.iter().map(|c| c.im)),
        )
    } else {
        (
            spectrum(gts, data.iter().map(|c| (c.re * c.re + c.im * c.im).sqrt())),
            spectrum(gts, data.iter().map(|c| c.im.atan2(c.re))),
        )
    };
    let frequency =
        TIME_UNITS_PER_S as f64 / (data.len() as i64).wrapping_mul(gts.bucketspan) as f64;
    Ok(Value::list(vec![
        Value::Gts(first.shared()),
        Value::Gts(second.shared()),
        Value::Double(frequency),
    ]))
}

/// `FFT` (`complex`) and `FFTAP`: the transform of a series or of each series of a list.
fn fft_function(stack: &mut Stack, complex: bool) -> Result<()> {
    // Both functions name `FFT` in their messages.
    gts_or_list_try(stack, "FFT", |gts| {
        fft_series(&mut gts.borrow_mut(), complex)
    })
}

/// `IFFT`: the series of the real parts of the inverse transform of the real and imaginary
/// parts (`DOUBLE` series) of 2 series. Both are sorted.
fn ifft(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "IFFT";
    let Value::Gts(imaginary) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a Geo Time Series of imaginary parts on top of the stack.",
        ));
    };
    let Value::Gts(real) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a Geo Time Series of real parts below the imaginary parts GTS.",
        ));
    };
    if real.borrow().type_of() != Type::Double || imaginary.borrow().type_of() != Type::Double {
        return Err(SeriesError::PartsType(NAME.to_owned()).into());
    }
    real.borrow_mut().sort(false);
    // The same series can be both parts.
    if !Rc::ptr_eq(&real, &imaginary) {
        imaginary.borrow_mut().sort(false);
    }
    let real = real.borrow();
    let (real_values, imaginary_values) = (doubles(&real), doubles(&imaginary.borrow()));
    if real_values.len() != imaginary_values.len() {
        return Err(SeriesError::PartsLength.into());
    }
    let mut data: Vec<Complex> = real_values
        .iter()
        .zip(&imaginary_values)
        .map(|(&re, &im)| Complex { re, im })
        .collect();
    if data.is_empty() {
        return Err(IllegalArgumentError::TransformLength.into());
    }
    fft::inverse(&mut data);
    let series = spectrum(&real, data.iter().map(|c| c.re));
    drop(real);
    stack.push(Value::Gts(series.shared()));
    Ok(())
}

/// A window function (<https://en.wikipedia.org/wiki/Window_function>) `FFTWINDOW` knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Window {
    Blackman,
    BlackmanHarris,
    BlackmanNuttall,
    Flattop,
    Hamming,
    Hann,
    Nuttall,
    Parzen,
    Rectangular,
    Sine,
    Triangular,
    Welch,
}

impl Window {
    /// The window of a lowercase algorithm name.
    fn parse(algorithm: &str) -> Option<Window> {
        Some(match algorithm {
            "blackman" => Window::Blackman,
            "blackman-harris" => Window::BlackmanHarris,
            "blackman-nuttall" => Window::BlackmanNuttall,
            "flattop" => Window::Flattop,
            "hamming" => Window::Hamming,
            "hann" => Window::Hann,
            "nuttall" => Window::Nuttall,
            "parzen" => Window::Parzen,
            "rectangular" => Window::Rectangular,
            "sine" => Window::Sine,
            "triangular" => Window::Triangular,
            "welch" => Window::Welch,
            _ => return None,
        })
    }

    /// The weight of the `n`-th of `size` values. A window of one value divides by zero
    /// like Warp 10 does, which makes a `NaN`.
    fn weight(self, n: usize, size: usize) -> f64 {
        let (n, size) = (n as f64, size as f64);
        // The weights that are sums of cosines of multiples of the same angle.
        let cos = |multiple: f64| (multiple * PI * n / (size - 1.0)).cos();
        match self {
            Window::Blackman => 0.42 - 0.5 * cos(2.0) + 0.08 * cos(4.0),
            Window::BlackmanHarris => {
                0.35875 - 0.48829 * cos(2.0) + 0.14128 * cos(4.0) - 0.01168 * cos(6.0)
            }
            Window::BlackmanNuttall => {
                0.363_581_9 - 0.489_177_5 * cos(2.0) + 0.136_599_5 * cos(4.0)
                    - 0.010_641_1 * cos(6.0)
            }
            Window::Flattop => {
                1.0 - 1.93 * cos(2.0) + 1.29 * cos(4.0) - 0.388 * cos(6.0) + 0.028 * cos(8.0)
            }
            Window::Hamming => 0.54 - 0.46 * cos(2.0),
            Window::Hann => {
                let sine = (PI * n / (size - 1.0)).sin();
                sine * sine
            }
            Window::Nuttall => {
                0.355_768 - 0.487_396 * cos(2.0) + 0.144_232 * cos(4.0) - 0.012_604 * cos(6.0)
            }
            Window::Parzen => {
                if n <= size / 4.0 {
                    1.0 - 6.0 * jmath::pow(n / (size / 2.0), 2.0) * (1.0 - n / (size / 2.0))
                } else {
                    2.0 * jmath::pow(1.0 - n / (size / 2.0), 3.0)
                }
            }
            Window::Rectangular => 1.0,
            Window::Sine => (PI * n / (size - 1.0)).sin(),
            Window::Triangular => 1.0 - ((n - (size - 1.0) / 2.0) / (size / 2.0)).abs(),
            Window::Welch => (n - (size - 1.0) / 2.0) / ((size - 1.0) / 2.0),
        }
    }
}

/// `FFTWINDOW`: a series multiplied by a window function, to apply before `FFT`. The
/// result has the bucketization of the series, but not its name nor its labels.
fn fftwindow(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "FFTWINDOW";
    let Value::String(algorithm) = stack.pop()? else {
        return Err(fail(NAME, "expects a STRING algorithm name."));
    };
    let algorithm = algorithm.to_lowercase();
    gts_or_list_try(stack, NAME, |gts| {
        let mut gts = gts.borrow_mut();
        gts.sort(false);
        let mut windowed =
            GeoTimeSerie::with_buckets(gts.lastbucket, gts.bucketcount, gts.bucketspan);
        if !is_numeric(&gts) {
            return Err(SeriesError::WindowNotNumeric {
                function: NAME.to_owned(),
                selector: build_selector_ordered(&gts.metadata, false),
            }
            .into());
        }
        let values = doubles(&gts);
        // Warp 10 only finds the algorithm unknown when it has a value to weigh.
        if !values.is_empty() {
            let window = Window::parse(&algorithm).ok_or_else(|| SeriesError::UnknownWindow {
                function: NAME.to_owned(),
                algorithm: algorithm.clone(),
            })?;
            for (n, value) in values.iter().enumerate() {
                windowed.set_value(
                    gts.tick_at(n),
                    gts.location_at(n),
                    gts.elevation_at(n),
                    &Value::Double(value * window.weight(n, values.len())),
                    false,
                );
            }
        }
        Ok(Value::Gts(windowed.shared()))
    })
}

/// The wavelet named on top of the stack.
fn pop_wavelet(stack: &mut Stack, name: &str) -> Result<&'static Wavelet> {
    let Value::String(wavelet) = stack.pop()? else {
        return Err(fail(name, "expects a wavelet name on top of the stack."));
    };
    Wavelet::find(&wavelet).ok_or_else(|| {
        SeriesError::UnknownWavelet {
            function: name.to_owned(),
            wavelet: wavelet.to_string(),
        }
        .into()
    })
}

/// Whether a length is `0` or a power of two (`0 == (len & (len - 1))`).
fn is_power_of_two(length: usize) -> bool {
    length & length.wrapping_sub(1) == 0
}

/// Check that a series has the length a wavelet transform requires.
fn ensure_power_of_two(name: &str, gts: &GeoTimeSerie) -> Result<()> {
    if is_power_of_two(gts.size()) {
        Ok(())
    } else {
        Err(SeriesError::NotPowerOfTwo(name.to_owned()).into())
    }
}

/// `FDWT`: the discrete wavelet transform of a numeric series whose length is a power of
/// two, which is sorted.
fn fdwt(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "FDWT";
    let wavelet = pop_wavelet(stack, NAME)?;
    gts_or_list_try(stack, NAME, |gts| {
        let mut gts = gts.borrow_mut();
        if !is_numeric(&gts) {
            return Err(SeriesError::WaveletNotNumeric(NAME.to_owned()).into());
        }
        ensure_power_of_two(NAME, &gts)?;
        gts.sort(false);
        let values = doubles(&gts);
        Ok(Value::Gts(
            with_doubles(&gts, &wavelet.forward(&values)).shared(),
        ))
    })
}

/// `IDWT`: the inverse of `FDWT`, for a `DOUBLE` series whose length is a power of two.
fn idwt(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "IDWT";
    let wavelet = pop_wavelet(stack, NAME)?;
    gts_or_list_try(stack, NAME, |gts| {
        let gts = gts.borrow();
        if gts.type_of() != Type::Double {
            return Err(SeriesError::WaveletNotNumeric(NAME.to_owned()).into());
        }
        ensure_power_of_two(NAME, &gts)?;
        let values = doubles(&gts);
        Ok(Value::Gts(
            with_doubles(&gts, &wavelet.reverse(&values)).shared(),
        ))
    })
}

/// `DWTSPLIT`: the series of the transform of each level, the first one holding 1 value and
/// each next one twice as many, labeled with the number of levels left (`1` for the finest).
fn dwtsplit(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "DWTSPLIT";
    let Value::String(level_label) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a level label name on top of the stack.",
        ));
    };
    gts_or_list_try(stack, NAME, |gts| {
        let mut gts = gts.borrow_mut();
        if gts.type_of() != Type::Double {
            return Err(SeriesError::WaveletNotNumeric(NAME.to_owned()).into());
        }
        ensure_power_of_two(NAME, &gts)?;
        if gts.metadata.labels.contains_key(&*level_label) {
            return Err(SeriesError::LevelLabelPresent {
                function: NAME.to_owned(),
                label: level_label.to_string(),
            }
            .into());
        }
        gts.sort(false);
        let levels = gts.size().checked_ilog2().unwrap_or(0);
        let mut first = 0;
        let mut series = Vec::new();
        for level in 0..levels {
            let width = 1usize << level;
            let mut split = GeoTimeSerie::new();
            split.metadata = gts.metadata.clone();
            split
                .metadata
                .labels
                .insert(level_label.to_string(), (levels - level).to_string());
            for idx in first..first + width {
                split.copy_datapoint(&gts, idx);
            }
            first += width;
            series.push(Value::Gts(split.shared()));
        }
        Ok(Value::list(series))
    })
}
