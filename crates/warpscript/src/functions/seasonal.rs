// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Seasonal analysis of series: the Seasonal-Trend decomposition based on Loess (`STL`) and
//! the outlier tests built on it (`GTSOutliersHelper`): `STLESDTEST`, and `HYBRIDTEST` and
//! `HYBRIDTEST2`, the seasonal hybrid ESD tests of Twitter's AnomalyDetection.

use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;

use super::Entry;
use super::args::fail;
use super::functional::double_compare;
use super::gts::gts_or_list_try;
use super::gts_stats::musigma;
use super::outliers::{double_values, esd};
use crate::error::fault::BoundsError;
use crate::error::series::SeriesError;
use crate::error::{Class, Error, Result};
use crate::gts::{GeoTimeSerie, NO_ELEVATION, NO_LOCATION, Values, binary_search_range};
use crate::stack::Stack;
use crate::stl::{Stl, int_div, sub_cycle_serie};
use crate::value::{Map, Value, ValueKind};

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("STL", Entry::Stack(stl)),
    ("STLESDTEST", Entry::Stack(stlesdtest)),
    ("HYBRIDTEST", Entry::Stack(hybridtest)),
    ("HYBRIDTEST2", Entry::Stack(hybridtest2)),
];

/// The significance level of the tests that are not given one.
const DEFAULT_ALPHA: f64 = 0.05;

/// The parameters of a piecewise test, in the `int`s of Java.
#[derive(Clone, Copy, Debug)]
struct Piecewise {
    /// The number of buckets of a seasonal period.
    buckets_per_period: i32,
    /// The number of periods of a piece.
    periods_per_piece: i32,
    /// The upper bound of the number of outliers of a piece.
    upper_bound: i32,
    /// The significance level.
    alpha: f64,
}

/// The pieces a series is cut into: the last ones of `count` pieces of `buckets` buckets.
#[derive(Clone, Copy, Debug)]
struct Pieces {
    /// The number of pieces.
    count: i64,
    /// The number of buckets of a piece.
    buckets: i32,
    /// The last bucket of the series.
    lastbucket: i64,
    /// The span of the buckets of the series.
    bucketspan: i64,
}

impl Pieces {
    /// The pieces of a series, after the checks the piecewise tests (`hybridTest` and
    /// `entropyHybridTest`) share.
    fn of(gts: &GeoTimeSerie, test: Piecewise) -> Result<Pieces> {
        double_values(gts)?;
        if !gts.is_bucketized() {
            return Err(SeriesError::NotBucketized.into());
        }
        let Piecewise {
            buckets_per_period,
            periods_per_piece,
            upper_bound,
            ..
        } = test;
        let buckets = periods_per_piece.wrapping_mul(buckets_per_period);
        if upper_bound >= buckets / 2 {
            return Err(SeriesError::OutliersUpperBound.into());
        }
        let bucketcount = gts.bucketcount as i32;
        let periods = int_div(bucketcount, buckets_per_period)?;
        if periods < 1 {
            return Err(SeriesError::NoSeasonalPeriod.into());
        }
        let count = int_div(periods, periods_per_piece)?;
        if count == 0 {
            return Err(SeriesError::NoPiece.into());
        }
        Ok(Pieces {
            count: i64::from(count),
            buckets,
            lastbucket: gts.lastbucket,
            bucketspan: gts.bucketspan,
        })
    }

    /// The `index`-th piece of a series, bucketized like the series (sorted in place).
    fn piece(&self, gts: &mut GeoTimeSerie, index: i64) -> (GeoTimeSerie, i64) {
        let remaining = self.count.wrapping_sub(index);
        let buckets = i64::from(self.buckets);
        let start = self.lastbucket.wrapping_sub(
            self.bucketspan
                .wrapping_mul(remaining.wrapping_mul(buckets).wrapping_sub(1)),
        );
        let stop = self.lastbucket.wrapping_sub(
            self.bucketspan
                .wrapping_mul(remaining.wrapping_sub(1))
                .wrapping_mul(buckets),
        );
        let mut piece = gts.subserie(start, stop);
        piece.lastbucket = stop;
        piece.bucketcount = buckets;
        piece.bucketspan = self.bucketspan;
        (piece, stop)
    }
}

/// `GTSOutliersHelper.median` of the values of a piece: it has to hold one.
fn median(piece: &GeoTimeSerie) -> Result<f64> {
    let mut sorted = piece.values.doubles().to_vec();
    if sorted.is_empty() {
        // Java reads the middle of an empty array.
        return Err(Error::index_out_of_bounds(0, 0));
    }
    sorted.sort_by(|a, b| double_compare(*a, *b));
    let n = sorted.len();
    Ok(if n.is_multiple_of(2) {
        #[allow(clippy::manual_midpoint)] // Java's `(a + b) / 2`
        let middle = (sorted[n / 2] + sorted[n / 2 - 1]) / 2.0;
        middle
    } else {
        sorted[n / 2]
    })
}

/// The seasonal component of a piece, estimated by the entropy of each of its cycle
/// sub-series: the mean of the sub-series, the more so as its values are alike.
fn entropy_seasonal(
    piece: &mut GeoTimeSerie,
    stop: i64,
    bucketspan: i64,
    buckets_per_period: i32,
) -> Result<GeoTimeSerie> {
    let mut seasonal = GeoTimeSerie::new();
    seasonal.values = Values::Double(Vec::new());
    for offset in 0..buckets_per_period {
        let cycle = sub_cycle_serie(
            piece,
            stop.wrapping_sub(i64::from(offset).wrapping_mul(bucketspan)),
            buckets_per_period,
        )?;
        let values = cycle.values.doubles();
        let (mean, sigma) = musigma(values, true);
        // The z-scores, turned into probabilities by a softmax of their square roots (the
        // exponential alone sets the outliers too far from the others).
        let weights: Vec<f64> = values
            .iter()
            .map(|value| {
                let score = if sigma == 0.0 {
                    1.0
                } else {
                    ((value - mean) / sigma).abs()
                };
                score.sqrt().exp()
            })
            .collect();
        let sum = weights.iter().fold(0.0, |sum, weight| sum + weight);
        let mut entropy = 0.0;
        for weight in &weights {
            // No probability is zero: the weights are at least 1 and their sum is finite.
            let probability = weight / sum;
            entropy -= probability * probability.ln();
        }
        // Normalized, and 1 when the values are all the same.
        entropy = if entropy == 0.0 {
            1.0
        } else {
            entropy / (values.len() as f64).ln()
        };
        for &tick in &cycle.ticks {
            seasonal.set_value(
                tick,
                NO_LOCATION,
                NO_ELEVATION,
                &Value::Double(entropy * mean),
                false,
            );
        }
    }
    seasonal.sort(false);
    Ok(seasonal)
}

/// Subtract the seasonal component and the median from the values of a piece, whose ticks
/// all have a seasonal value.
fn deseasonalize(
    piece: &mut GeoTimeSerie,
    seasonal: &GeoTimeSerie,
    median: f64,
    method: &str,
) -> Result<()> {
    let seasons = seasonal.values.doubles();
    let mut residuals = Vec::with_capacity(piece.size());
    let mut idx = 0;
    for (&tick, &value) in piece.ticks.iter().zip(piece.values.doubles()) {
        idx = binary_search_range(&seasonal.ticks, idx, seasonal.size() as i64, tick);
        let Ok(found) = usize::try_from(idx) else {
            return Err(SeriesError::TickNotSeasonal {
                method: method.to_owned(),
                tick,
            }
            .into());
        };
        residuals.push(value - (seasons[found] + median));
    }
    piece.values = Values::Double(residuals);
    Ok(())
}

/// The outliers of the `index`-th piece of a series, once the seasonal component (which
/// `seasonal_of` gives the piece and its last bucket) and the median are removed from it.
fn piece_outliers(
    gts: &mut GeoTimeSerie,
    pieces: &Pieces,
    index: i64,
    test: Piecewise,
    method: &str,
    seasonal_of: impl FnOnce(&mut GeoTimeSerie, i64) -> Result<GeoTimeSerie>,
) -> Result<Vec<i64>> {
    let (mut piece, stop) = pieces.piece(gts, index);
    let seasonal = seasonal_of(&mut piece, stop)?;
    let median = median(&piece)?;
    deseasonalize(&mut piece, &seasonal, median, method)?;
    esd(&piece, test.upper_bound, true, test.alpha)
}

/// `GTSOutliersHelper.entropyHybridTest`: the ticks of the outliers of each piece of the
/// series, once the seasonal component and the median are removed from it. The series is
/// sorted.
fn entropy_hybrid_test(gts: &mut GeoTimeSerie, test: Piecewise) -> Result<Vec<i64>> {
    let pieces = Pieces::of(gts, test)?;
    let mut anomalies = Vec::new();
    for index in 0..pieces.count {
        anomalies.extend(piece_outliers(
            gts,
            &pieces,
            index,
            test,
            "entropyHybridTest",
            |piece, stop| entropy_seasonal(piece, stop, pieces.bucketspan, test.buckets_per_period),
        )?);
    }
    Ok(anomalies)
}

/// `HYBRIDTEST2`: the entropy seasonal hybrid ESD test, for a series bucketized and filled.
fn hybridtest2(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "HYBRIDTEST2";
    // The significance level is optional: a `LONG` on top is the upper bound already.
    let (alpha, top) = match stack.pop()? {
        Value::Double(alpha) => (alpha, stack.pop()?),
        top @ Value::Long(_) => (DEFAULT_ALPHA, top),
        _ => {
            return Err(fail(
                NAME,
                "expects a significance level (a DOUBLE) or an upper bound of the number of outliers (a LONG) on top of the stack.",
            ));
        }
    };
    let Value::Long(upper_bound) = top else {
        return Err(fail(
            NAME,
            "expects an upper bound of the number of outliers (a LONG) below the significance level.",
        ));
    };
    let Value::Long(periods_per_piece) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a number of periods per piece (a LONG) below the upper bound of the number of outliers.",
        ));
    };
    let Value::Long(buckets_per_period) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a number of buckets per period (a LONG) below the number of periods per piece.",
        ));
    };
    // `intValue()`.
    let test = Piecewise {
        buckets_per_period: buckets_per_period as i32,
        periods_per_piece: periods_per_piece as i32,
        upper_bound: upper_bound as i32,
        alpha,
    };
    gts_or_list_try(stack, NAME, |gts| {
        let mut gts = gts.borrow_mut();
        if !gts.is_bucketized() || gts.size() as i64 != gts.bucketcount {
            return Err(SeriesError::NotBucketizedFilled(NAME.to_owned()).into());
        }
        let ticks = entropy_hybrid_test(&mut gts, test)?;
        Ok(Value::list(ticks.into_iter().map(Value::Long).collect()))
    })
}

// -----------------------------------------------------------------------------------------
// STL
// -----------------------------------------------------------------------------------------

/// The parameters of `STL`, in a map of `STRING` keys.
type Params = IndexMap<Value, Value>;

const PERIOD: &str = "PERIOD";
const PRECISION: &str = "PRECISION";
const ROBUSTNESS: &str = "ROBUSTNESS";
const ROBUST: &str = "ROBUST";
const BANDWIDTH: &str = "BANDWIDTH";
const DEGREE: &str = "DEGREE";
const SPEED: &str = "SPEED";
/// The smoothings the bandwidth, the degree and the speed apply to (`_S`easonal, `_L`ow-pass,
/// `_T`rend, `_P`ost seasonal), as suffixes.
const SMOOTHINGS: [&str; 4] = ["_S", "_L", "_T", "_P"];

/// `((Number) value).intValue()`.
fn int_value(value: &Value) -> Result<i32> {
    match value {
        Value::Double(double) => Ok(*double as i32),
        other => other
            .as_i64()
            .map(|long| long as i32)
            .ok_or_else(|| Error::class_cast(other.kind(), Class::Number)),
    }
}

/// The value of a parameter, when it is set (a `null` is as good as nothing, like in Java).
fn param<'a>(params: &'a Params, name: &str) -> Option<&'a Value> {
    params
        .get(&Value::from(name))
        .filter(|value| !matches!(value, Value::Null))
}

/// The value of a parameter as an `int`, when it is set.
fn int_param(params: &Params, name: &str) -> Result<Option<i32>> {
    param(params, name).map(int_value).transpose()
}

/// Set a parameter that is not.
fn set_default(params: &mut Params, name: &str, value: Value) {
    if param(params, name).is_none() {
        params.insert(Value::from(name), value);
    }
}

/// `STL.retrieveParameters`: the checked parameters of the map on the stack: the
/// bandwidth, degree and speed of every smoothing (`BANDWIDTH`, `DEGREE` and `SPEED` set them
/// all), the period, the precision, the robustness and whether to be robust.
fn retrieve_parameters(given: &Params) -> Result<Params> {
    const NAME: &str = "STL";
    let mut params = Params::new();
    for (key, value) in given {
        let Value::String(key) = key else {
            return Err(Error::class_cast(key.kind(), ValueKind::String));
        };
        // `key.substring(0, key.length() - 2)` and the last 2 characters.
        let units: Vec<u16> = key.encode_utf16().collect();
        let Some(body_length) = units.len().checked_sub(2) else {
            return Err(BoundsError::StringEnd {
                end: units.len() as i64 - 2,
                length: units.len(),
            }
            .into());
        };
        let body = String::from_utf16_lossy(&units[..body_length]);
        let suffix = String::from_utf16_lossy(&units[body_length..]);
        if &**key == ROBUST {
            if !matches!(value, Value::Boolean(_)) {
                return Err(fail(
                    NAME,
                    &format!("expects argument {key} to be of type BOOLEAN."),
                ));
            }
            params.insert(Value::String(Rc::clone(key)), value.clone());
            continue;
        }
        let smoothing = [BANDWIDTH, DEGREE, SPEED];
        let known = [PERIOD, PRECISION, ROBUSTNESS].contains(&&**key)
            || smoothing.contains(&&**key)
            || (smoothing.contains(&body.as_str()) && SMOOTHINGS.contains(&suffix.as_str()));
        if !known {
            return Err(fail(NAME, &format!("does not expect argument {key}")));
        }
        let Value::Long(long) = value else {
            return Err(fail(
                NAME,
                &format!("expects argument {key} to be of type LONG."),
            ));
        };
        params
            .entry(Value::String(Rc::clone(key)))
            .or_insert(Value::Long(i64::from(*long as i32)));
    }
    // The parameters without a suffix are those of every smoothing, when not set.
    for name in [BANDWIDTH, DEGREE, SPEED] {
        let Some(common) = params.get(&Value::from(name)).cloned() else {
            continue;
        };
        for suffix in SMOOTHINGS {
            params
                .entry(Value::from(format!("{name}{suffix}")))
                .or_insert_with(|| common.clone());
        }
    }
    Ok(params)
}

/// The next odd number of a size, the default bandwidth (which is not always odd, like Warp 10's:
/// `STL.nextOdd(5)` is 6).
fn next_odd(size: i32) -> i32 {
    if size > 0 {
        if size / 2 == 1 {
            size
        } else {
            size.wrapping_add(1)
        }
    } else {
        1
    }
}

/// The default parameters of the smoothing `suffix`: its bandwidth, degree and speed (a tenth
/// of the bandwidth) once the parameters are read.
fn smoothing(
    params: &Params,
    suffix: &str,
    bandwidth: i32,
    degree: i32,
) -> Result<(i32, i32, i32)> {
    let ns = int_param(params, &format!("{BANDWIDTH}{suffix}"))?.unwrap_or(bandwidth);
    let ds = int_param(params, &format!("{DEGREE}{suffix}"))?.unwrap_or(degree);
    let js = match int_param(params, &format!("{SPEED}{suffix}"))? {
        Some(speed) => speed,
        None => ns.wrapping_div(10),
    };
    Ok((ns, ds, js))
}

/// `STL.gtsOp`, up to the decomposition: the parameters of the decomposition, from the
/// parameters set and the defaults of R's `stl`. `ROBUST` is set when it is not.
fn read_parameters(params: &mut Params) -> Result<Stl> {
    const NAME: &str = "STL";
    // The period is a `LONG` the function checked, or that the test put there.
    let Some(period) = param(params, PERIOD).and_then(Value::as_i64) else {
        return Err(fail(
            NAME,
            "expects map of parameters to at least contains field PERIOD",
        ));
    };
    let period = period as i32;
    let robust = match param(params, ROBUST) {
        Some(Value::Boolean(robust)) => *robust,
        Some(other) => return Err(Error::class_cast(other.kind(), ValueKind::Boolean)),
        None => {
            params.insert(Value::from(ROBUST), Value::Boolean(false));
            false
        }
    };
    // The loops: 1 and 15 when robust, 2 and 0 otherwise.
    let inner = int_param(params, PRECISION)?.unwrap_or(if robust { 1 } else { 2 });
    let outer = int_param(params, ROBUSTNESS)?.unwrap_or(if robust { 15 } else { 0 });
    // The authors recommend a seasonal bandwidth that is odd and at least 7.
    let (ns, ds, js) = smoothing(params, "_S", 7, 1)?;
    let (nl, dl, jl) = smoothing(params, "_L", next_odd(period), 1)?;
    let trend = (1.5 * f64::from(period) / (1.0 - 1.5 / f64::from(ns))).ceil() as i32;
    let (nt, dt, jt) = smoothing(params, "_T", next_odd(trend), 1)?;
    // No post seasonal smoothing by default.
    let (np, dp, jp) = smoothing(params, "_P", 0, 2)?;
    let checks = [
        (
            period < 2,
            "expects seasonal periods to be composed by at least 2 buckets.".to_owned(),
        ),
        (inner < 1, format!("expects {PRECISION} to be positive.")),
        (
            outer < 0,
            format!("expects {ROBUSTNESS} to be non-negative."),
        ),
        (
            ns == 0,
            format!("expects {BANDWIDTH}_S to be different than zero."),
        ),
        (ds < 0, format!("expects {DEGREE}_S to be non-negative.")),
        (js < 0, format!("expects {SPEED}_S to be non-negative.")),
        (nl < 0, format!("expects {BANDWIDTH}_L to be non-negative.")),
        (dl < 0, format!("expects {DEGREE}_L to be non-negative.")),
        (jl < 0, format!("expects {SPEED}_L to be non-negative.")),
        (nt < 0, format!("expects {BANDWIDTH}_T to be non-negative.")),
        (dt < 0, format!("expects {DEGREE}_T to be non-negative.")),
        (jt < 0, format!("expects {SPEED}_T to be non-negative.")),
        (np < 0, format!("expects {BANDWIDTH}_P to be non-negative.")),
        (dp < 0, format!("expects {DEGREE}_P to be non-negative.")),
        (jp < 0, format!("expects {SPEED}_P to be non-negative.")),
    ];
    if let Some((_, message)) = checks.iter().find(|(failed, _)| *failed) {
        return Err(fail(NAME, message));
    }
    Ok(Stl {
        period,
        inner,
        outer,
        ns,
        ds,
        js,
        nl,
        dl,
        jl,
        nt,
        dt,
        jt,
        np,
        dp,
        jp,
    })
}

/// The seasonal and the trend components of a series, as a list.
fn decomposition(
    gts: &mut GeoTimeSerie,
    params: &mut Params,
) -> Result<(GeoTimeSerie, GeoTimeSerie)> {
    read_parameters(params)?.decompose(gts)
}

/// `STL`: `gts { 'PERIOD' 12 … } STL`, the seasonal and trend components of a series
/// bucketized, as a list.
fn stl(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "STL";
    let Value::Map(given) = stack.pop()? else {
        return Err(fail(NAME, "expects a map of parameters below input GTS"));
    };
    let params = retrieve_parameters(&given.borrow())?;
    gts_or_list_try(stack, NAME, |gts| {
        let (seasonal, trend) = decomposition(&mut gts.borrow_mut(), &mut params.clone())?;
        Ok(Value::list(vec![
            Value::Gts(seasonal.shared()),
            Value::Gts(trend.shared()),
        ]))
    })
}

// -----------------------------------------------------------------------------------------
// STLESDTEST and HYBRIDTEST
// -----------------------------------------------------------------------------------------

/// The parameters the tests keep in the map they are given (or in a new one), where `STL`
/// reads its own.
const BUCKETS_KEY: &str = "bpp";
const PIECE_KEY: &str = "ppp";
const BOUND_KEY: &str = "k";
const ALPHA_KEY: &str = "alpha";

/// The arguments of the seasonal tests. Java keeps them in the map of the script (or in a new
/// one), which the test then removes them from and `STL` reads its parameters in: a series after
/// the first one of a list finds none.
struct TestArguments {
    params: Map,
    test: Piecewise,
    hybrid: bool,
}

impl TestArguments {
    /// `STLESDTEST.retrieveParameters` and `HYBRIDTEST.retrieveParameters`: the optional map of
    /// parameters on top of the stack, the optional significance level, the upper bound of
    /// the number of outliers, and for the hybrid test the number of periods per piece, then
    /// the number of buckets per period.
    fn retrieve(stack: &mut Stack, name: &str, hybrid: bool) -> Result<TestArguments> {
        let mut top = stack.pop()?;
        let params = if let Value::Map(map) = &top {
            let map = Rc::clone(map);
            top = stack.pop()?;
            map
        } else {
            Rc::new(RefCell::new(IndexMap::new()))
        };
        let alpha = match top {
            Value::Double(alpha) => {
                top = stack.pop()?;
                alpha
            }
            // An upper bound: the significance level is the default.
            Value::Long(_) => DEFAULT_ALPHA,
            _ => {
                return Err(fail(
                    name,
                    "expects a significance level (a DOUBLE) or an upper bound of the number of outliers (a LONG) on top of the stack.",
                ));
            }
        };
        let put = |key: &str, value: Value| {
            params.borrow_mut().insert(Value::from(key), value);
        };
        put(ALPHA_KEY, Value::Double(alpha));
        let Value::Long(upper_bound) = top else {
            return Err(fail(
                name,
                "expects an upper bound of the number of outliers (a LONG) below the significance level.",
            ));
        };
        put(BOUND_KEY, Value::Long(upper_bound));
        let mut periods_per_piece = 0;
        if hybrid {
            let Value::Long(pieces) = stack.pop()? else {
                return Err(fail(
                    name,
                    "expects a number of periods per piece (a LONG) below the upper bound of the number of outliers.",
                ));
            };
            put(PIECE_KEY, Value::Long(pieces));
            periods_per_piece = pieces as i32;
        }
        let Value::Long(period) = stack.pop()? else {
            return Err(fail(
                name,
                if hybrid {
                    "expects a number of buckets per period (a LONG) below the number of periods per piece."
                } else {
                    "expects a number of buckets per period (a LONG) below the upper bound of the number of outliers."
                },
            ));
        };
        put(BUCKETS_KEY, Value::Long(period));
        Ok(TestArguments {
            params,
            test: Piecewise {
                buckets_per_period: period as i32,
                periods_per_piece,
                upper_bound: upper_bound as i32,
                alpha,
            },
            hybrid,
        })
    }

    /// The arguments of the test on a series, which are removed from the map: the series after
    /// the first one find none.
    fn take(&self) -> Result<Piecewise> {
        let keys: &[&str] = if self.hybrid {
            &[BUCKETS_KEY, PIECE_KEY, BOUND_KEY, ALPHA_KEY]
        } else {
            &[BUCKETS_KEY, BOUND_KEY, ALPHA_KEY]
        };
        for key in keys {
            if self
                .params
                .borrow_mut()
                .shift_remove(&Value::from(*key))
                .is_none()
            {
                return Err(Error::null_pointer());
            }
        }
        Ok(self.test)
    }
}

/// The parameters of the decomposition of a test: the map of the user (or a new one), with the
/// period of the test, no seasonal smoothing (the mean of each cycle) and no robustness when
/// they are not set.
///
/// # Errors
///
/// A period that is not the one of the test, or a parameter that is not a number.
fn test_parameters(params: &mut Params, buckets_per_period: i32) -> Result<()> {
    if let Some(period) = param(params, PERIOD) {
        // Warp 10 casts to the `Integer` the test puts there: the `Long` of a script, which
        // makes the cast fail, is accepted here when it is the period of the test.
        let Value::Long(period) = period else {
            return Err(Error::class_cast(period.kind(), Class::Integer));
        };
        if *period as i32 != buckets_per_period {
            return Err(SeriesError::PeriodMismatch.into());
        }
    } else {
        params.insert(
            Value::from(PERIOD),
            Value::Long(i64::from(buckets_per_period)),
        );
    }
    set_default(params, &format!("{BANDWIDTH}_S"), Value::Long(-1));
    set_default(params, ROBUST, Value::Boolean(false));
    Ok(())
}

/// `GTSOutliersHelper.STLESDTest`: the ticks of the outliers of the residuals of the
/// decomposition of the series (`ESDTEST` with the median).
fn stl_esd_test(gts: &mut GeoTimeSerie, test: Piecewise, params: &mut Params) -> Result<Vec<i64>> {
    double_values(gts)?;
    if !gts.is_bucketized() {
        return Err(SeriesError::NotBucketized.into());
    }
    test_parameters(params, test.buckets_per_period)?;
    let (seasonal, trend) = decomposition(gts, params)?;
    let residuals = stl_residuals(gts, &seasonal, &trend)?;
    // The residuals are those of the first datapoints of the seasonal series.
    let mut remainder = seasonal.clone_empty();
    remainder.ticks = seasonal.ticks[..gts.size()].to_vec();
    remainder.values = Values::Double(residuals);
    remainder.bucketcount = gts.bucketcount;
    remainder.bucketspan = gts.bucketspan;
    remainder.lastbucket = gts.lastbucket;
    esd(&remainder, test.upper_bound, true, test.alpha)
}

/// The residuals of a series: its values less the seasonal and the trend components at their
/// ticks. Warp 10 reads the value of the series at the index of the tick in the seasonal series.
///
/// # Errors
///
/// A tick that the seasonal series does not have, or an index past the values of the series.
pub(super) fn stl_residuals(
    gts: &GeoTimeSerie,
    seasonal: &GeoTimeSerie,
    trend: &GeoTimeSerie,
) -> Result<Vec<f64>> {
    let values = gts.values.doubles();
    let (seasons, trends) = (seasonal.values.doubles(), trend.values.doubles());
    let mut residuals = Vec::with_capacity(gts.size());
    let mut idx = 0;
    for &tick in &gts.ticks {
        idx = binary_search_range(&seasonal.ticks, idx, seasonal.size() as i64, tick);
        let Ok(found) = usize::try_from(idx) else {
            return Err(SeriesError::TickNotSeasonal {
                method: "STLESDTest".to_owned(),
                tick,
            }
            .into());
        };
        let Some(value) = values.get(found) else {
            return Err(Error::index_out_of_bounds(found as i64, values.len()));
        };
        residuals.push(value - (seasons[found] + trends[found]));
    }
    Ok(residuals)
}

/// The ticks of the outliers, as a list.
fn tick_list(ticks: Vec<i64>) -> Value {
    Value::list(ticks.into_iter().map(Value::Long).collect())
}

/// `STLESDTEST`: `gts bpp k (alpha) ({ … }) STLESDTEST`, the ESD test of the residuals of the
/// `STL` decomposition.
fn stlesdtest(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "STLESDTEST";
    let arguments = TestArguments::retrieve(stack, NAME, false)?;
    gts_or_list_try(stack, NAME, |gts| {
        let test = arguments.take()?;
        let mut params = arguments.params.borrow().clone();
        let ticks = stl_esd_test(&mut gts.borrow_mut(), test, &mut params);
        // The test sets the parameters of `STL` in the map of the script.
        *arguments.params.borrow_mut() = params;
        Ok(tick_list(ticks?))
    })
}

/// `GTSOutliersHelper.hybridTest`: the ticks of the outliers of each piece of the series, once
/// the seasonal component of the `STL` decomposition of the piece and its median are removed
/// from it. The series is sorted.
fn hybrid_test(gts: &mut GeoTimeSerie, test: Piecewise, params: &mut Params) -> Result<Vec<i64>> {
    let pieces = Pieces::of(gts, test)?;
    test_parameters(params, test.buckets_per_period)?;
    let mut anomalies = Vec::new();
    for index in 0..pieces.count {
        anomalies.extend(piece_outliers(
            gts,
            &pieces,
            index,
            test,
            "hybridTest",
            |piece, _| decomposition(piece, params).map(|(seasonal, _)| seasonal),
        )?);
    }
    Ok(anomalies)
}

/// `HYBRIDTEST`: `gts bpp ppp k (alpha) ({ … }) HYBRIDTEST`, the seasonal hybrid ESD test, by
/// pieces of `ppp` periods.
fn hybridtest(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "HYBRIDTEST";
    let arguments = TestArguments::retrieve(stack, NAME, true)?;
    gts_or_list_try(stack, NAME, |gts| {
        let test = arguments.take()?;
        let mut params = arguments.params.borrow().clone();
        let ticks = hybrid_test(&mut gts.borrow_mut(), test, &mut params);
        *arguments.params.borrow_mut() = params;
        Ok(tick_list(ticks?))
    })
}
