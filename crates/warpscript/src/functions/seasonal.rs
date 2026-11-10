// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Outlier tests on seasonal series (`GTSOutliersHelper`): `HYBRIDTEST2`, the entropy
//! variant of the seasonal hybrid ESD test of Twitter's AnomalyDetection.

use super::Entry;
use super::args::fail;
use super::functional::java_double_compare;
use super::gts::gts_or_list_try;
use super::gts_stats::musigma;
use super::outliers::{double_values, esd};
use crate::error::series::SeriesError;
use crate::error::{Error, Result};
use crate::gts::{GeoTimeSerie, NO_ELEVATION, NO_LOCATION, Values, java_binary_search_range};
use crate::stack::Stack;
use crate::stl::{int_div, sub_cycle_serie};
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[("HYBRIDTEST2", Entry::Stack(hybridtest2))];

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
    sorted.sort_by(|a, b| java_double_compare(*a, *b));
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
            let probability = weight / sum;
            if probability != 0.0 {
                entropy -= probability * probability.ln();
            }
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
        idx = java_binary_search_range(&seasonal.ticks, idx, seasonal.size() as i64, tick);
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

/// `GTSOutliersHelper.entropyHybridTest`: the ticks of the outliers of each piece of the
/// series, once the seasonal component and the median are removed from it. The series is
/// sorted.
fn entropy_hybrid_test(gts: &mut GeoTimeSerie, test: Piecewise) -> Result<Vec<i64>> {
    let pieces = Pieces::of(gts, test)?;
    let mut anomalies = Vec::new();
    for index in 0..pieces.count {
        let (mut piece, stop) = pieces.piece(gts, index);
        let seasonal =
            entropy_seasonal(&mut piece, stop, pieces.bucketspan, test.buckets_per_period)?;
        let median = median(&piece)?;
        deseasonalize(&mut piece, &seasonal, median, "entropyHybridTest")?;
        anomalies.extend(esd(&piece, test.upper_bound, true, test.alpha)?);
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
