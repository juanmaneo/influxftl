#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp
)]

//! Reference values below were produced with Apache Commons Math 3.6.1 on OpenJDK 27.

use crate::cmath::{erf_inv, normal_inverse_cdf};

#[test]
fn erf_inv_matches_commons_math() {
    let cases = [
        (0.0, 0.0),
        (0.5, 0.476_936_276_204_469_77),
        (-0.5, -0.476_936_276_204_469_77),
        (0.1, 0.088_855_990_494_257_67),
        (0.3, 0.272_462_714_726_754_4),
        (0.75, 0.813_419_847_597_618_4),
        (0.9, 1.163_087_153_676_674_3),
        (0.99, 1.821_386_367_718_449_4),
        (0.999, 2.326_753_765_513_524_6),
        (0.999_999, 3.458_910_737_275_499),
        (-0.999_999, -3.458_910_737_275_499),
        (0.999_999_999_9, 4.572_824_958_544_925),
        (1e-10, 8.862_269_254_527_581e-11),
        (1.0, f64::INFINITY),
        (-1.0, f64::NEG_INFINITY),
    ];
    for (x, expected) in cases {
        let actual = erf_inv(x);
        assert!(
            actual.to_bits() == expected.to_bits(),
            "erf_inv({x}) = {actual:?}, expected {expected:?}"
        );
    }
}

#[test]
fn normal_inverse_cdf_matches_commons_math() {
    assert_eq!(normal_inverse_cdf(0.0, 1.0, 0.975), 1.959_963_984_540_054);
    assert_eq!(normal_inverse_cdf(0.0, 1.0, 0.25), -0.674_489_750_196_081_6);
    assert_eq!(normal_inverse_cdf(0.0, 1.0, 0.5), 0.0);
    assert_eq!(normal_inverse_cdf(0.0, 1.0, 0.0), f64::NEG_INFINITY);
    assert_eq!(normal_inverse_cdf(0.0, 1.0, 1.0), f64::INFINITY);
    assert_eq!(normal_inverse_cdf(10.0, 2.0, 0.25), 8.651_020_499_607_837);
    assert_eq!(normal_inverse_cdf(10.0, 2.0, 0.75), 11.348_979_500_392_163);
}
