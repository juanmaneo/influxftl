// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of the seasonal outlier tests: `HYBRIDTEST2`.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const A24: &str = r"NEWGTS 'x' RENAME { 'l' 'v' } RELABEL 0 NaN NaN NaN 4.982 ADDVALUE 10 NaN NaN NaN 8.137 ADDVALUE 20 NaN NaN NaN 4.882 ADDVALUE 30 NaN NaN NaN 2.232 ADDVALUE 40 NaN NaN NaN 4.946 ADDVALUE 50 NaN NaN NaN 8.13 ADDVALUE 60 NaN NaN NaN 4.859 ADDVALUE 70 NaN NaN NaN 1.847 ADDVALUE 80 NaN NaN NaN 5.188 ADDVALUE 90 NaN NaN NaN 19.999 ADDVALUE 100 NaN NaN NaN 4.95 ADDVALUE 110 NaN NaN NaN 2.137 ADDVALUE 120 NaN NaN NaN 5.278 ADDVALUE 130 NaN NaN NaN 7.886 ADDVALUE 140 NaN NaN NaN 5.122 ADDVALUE 150 NaN NaN NaN 2.012 ADDVALUE 160 NaN NaN NaN 5.139 ADDVALUE 170 NaN NaN NaN -0.7 ADDVALUE 180 NaN NaN NaN 4.824 ADDVALUE 190 NaN NaN NaN 2.152 ADDVALUE 200 NaN NaN NaN 4.981 ADDVALUE 210 NaN NaN NaN 8.125 ADDVALUE 220 NaN NaN NaN 5.223 ADDVALUE 230 NaN NaN NaN 1.789 ADDVALUE 'g' STORE [ $g bucketizer.last 230 10 0 ] BUCKETIZE 0 GET";
const A48: &str = r"NEWGTS 'x' RENAME { 'l' 'v' } RELABEL 0 NaN NaN NaN 4.828 ADDVALUE 10 NaN NaN NaN 7.545 ADDVALUE 20 NaN NaN NaN 7.333 ADDVALUE 30 NaN NaN NaN 4.91 ADDVALUE 40 NaN NaN NaN 2.352 ADDVALUE 50 NaN NaN NaN 17.176 ADDVALUE 60 NaN NaN NaN 5.146 ADDVALUE 70 NaN NaN NaN 7.756 ADDVALUE 80 NaN NaN NaN 7.532 ADDVALUE 90 NaN NaN NaN 4.907 ADDVALUE 100 NaN NaN NaN 2.223 ADDVALUE 110 NaN NaN NaN 2.358 ADDVALUE 120 NaN NaN NaN 4.89 ADDVALUE 130 NaN NaN NaN 7.427 ADDVALUE 140 NaN NaN NaN 7.819 ADDVALUE 150 NaN NaN NaN 4.837 ADDVALUE 160 NaN NaN NaN 2.126 ADDVALUE 170 NaN NaN NaN 2.237 ADDVALUE 180 NaN NaN NaN 4.712 ADDVALUE 190 NaN NaN NaN 7.817 ADDVALUE 200 NaN NaN NaN 7.804 ADDVALUE 210 NaN NaN NaN 4.892 ADDVALUE 220 NaN NaN NaN 2.678 ADDVALUE 230 NaN NaN NaN 2.585 ADDVALUE 240 NaN NaN NaN 4.953 ADDVALUE 250 NaN NaN NaN 7.365 ADDVALUE 260 NaN NaN NaN 7.809 ADDVALUE 270 NaN NaN NaN 5.064 ADDVALUE 280 NaN NaN NaN 2.24 ADDVALUE 290 NaN NaN NaN 2.699 ADDVALUE 300 NaN NaN NaN -7.081 ADDVALUE 310 NaN NaN NaN 7.42 ADDVALUE 320 NaN NaN NaN 7.594 ADDVALUE 330 NaN NaN NaN 5.202 ADDVALUE 340 NaN NaN NaN 2.187 ADDVALUE 350 NaN NaN NaN 2.334 ADDVALUE 360 NaN NaN NaN 4.902 ADDVALUE 370 NaN NaN NaN 7.862 ADDVALUE 380 NaN NaN NaN 7.898 ADDVALUE 390 NaN NaN NaN 4.979 ADDVALUE 400 NaN NaN NaN 2.204 ADDVALUE 410 NaN NaN NaN 2.519 ADDVALUE 420 NaN NaN NaN 5.217 ADDVALUE 430 NaN NaN NaN 7.497 ADDVALUE 440 NaN NaN NaN 27.422 ADDVALUE 450 NaN NaN NaN 5.112 ADDVALUE 460 NaN NaN NaN 2.161 ADDVALUE 470 NaN NaN NaN 2.608 ADDVALUE 'g' STORE [ $g bucketizer.last 470 10 0 ] BUCKETIZE 0 GET";
const A40: &str = r"NEWGTS 'x' RENAME { 'l' 'v' } RELABEL 0 NaN NaN NaN 4.207 ADDVALUE 10 NaN NaN NaN 7.294 ADDVALUE 20 NaN NaN NaN 7.089 ADDVALUE 30 NaN NaN NaN 2.997 ADDVALUE 40 NaN NaN NaN 1.477 ADDVALUE 50 NaN NaN NaN 5.485 ADDVALUE 60 NaN NaN NaN 7.436 ADDVALUE 70 NaN NaN NaN 16.701 ADDVALUE 80 NaN NaN NaN 2.858 ADDVALUE 90 NaN NaN NaN 2.185 ADDVALUE 100 NaN NaN NaN 4.898 ADDVALUE 110 NaN NaN NaN 8.608 ADDVALUE 120 NaN NaN NaN 6.285 ADDVALUE 130 NaN NaN NaN 2.55 ADDVALUE 140 NaN NaN NaN 1.831 ADDVALUE 150 NaN NaN NaN 4.417 ADDVALUE 160 NaN NaN NaN 8.112 ADDVALUE 170 NaN NaN NaN 6.363 ADDVALUE 180 NaN NaN NaN 2.598 ADDVALUE 190 NaN NaN NaN 1.697 ADDVALUE 200 NaN NaN NaN 4.513 ADDVALUE 210 NaN NaN NaN 7.675 ADDVALUE 220 NaN NaN NaN 14.788 ADDVALUE 230 NaN NaN NaN 11.794 ADDVALUE 240 NaN NaN NaN 1.914 ADDVALUE 250 NaN NaN NaN 5.324 ADDVALUE 260 NaN NaN NaN 8.333 ADDVALUE 270 NaN NaN NaN 6.89 ADDVALUE 280 NaN NaN NaN 3.928 ADDVALUE 290 NaN NaN NaN 2.217 ADDVALUE 300 NaN NaN NaN 5.698 ADDVALUE 310 NaN NaN NaN 8.206 ADDVALUE 320 NaN NaN NaN 6.977 ADDVALUE 330 NaN NaN NaN 2.626 ADDVALUE 340 NaN NaN NaN 1.436 ADDVALUE 350 NaN NaN NaN 4.401 ADDVALUE 360 NaN NaN NaN 8.38 ADDVALUE 370 NaN NaN NaN 7.414 ADDVALUE 380 NaN NaN NaN 3.421 ADDVALUE 390 NaN NaN NaN 1.487 ADDVALUE 'g' STORE [ $g bucketizer.last 390 10 0 ] BUCKETIZE 0 GET";
const A30: &str = r"NEWGTS 'x' RENAME { 'l' 'v' } RELABEL 0 NaN NaN NaN 5.002 ADDVALUE 10 NaN NaN NaN 7.695 ADDVALUE 20 NaN NaN NaN 6.814 ADDVALUE 30 NaN NaN NaN 3.376 ADDVALUE 40 NaN NaN NaN 1.944 ADDVALUE 50 NaN NaN NaN 4.775 ADDVALUE 60 NaN NaN NaN 7.616 ADDVALUE 70 NaN NaN NaN 7.03 ADDVALUE 80 NaN NaN NaN 3.274 ADDVALUE 90 NaN NaN NaN 2.444 ADDVALUE 100 NaN NaN NaN 5.079 ADDVALUE 110 NaN NaN NaN 7.91 ADDVALUE 120 NaN NaN NaN 17.799 ADDVALUE 130 NaN NaN NaN 3.263 ADDVALUE 140 NaN NaN NaN 2.036 ADDVALUE 150 NaN NaN NaN 4.774 ADDVALUE 160 NaN NaN NaN 7.571 ADDVALUE 170 NaN NaN NaN 6.5 ADDVALUE 180 NaN NaN NaN 3.318 ADDVALUE 190 NaN NaN NaN 2.222 ADDVALUE 200 NaN NaN NaN 5.072 ADDVALUE 210 NaN NaN NaN 7.679 ADDVALUE 220 NaN NaN NaN 6.588 ADDVALUE 230 NaN NaN NaN 3.051 ADDVALUE 240 NaN NaN NaN 1.959 ADDVALUE 250 NaN NaN NaN 4.888 ADDVALUE 260 NaN NaN NaN 7.993 ADDVALUE 270 NaN NaN NaN 6.844 ADDVALUE 280 NaN NaN NaN 3.437 ADDVALUE 290 NaN NaN NaN 2.33 ADDVALUE 'g' STORE [ $g bucketizer.last 290 10 0 ] BUCKETIZE 0 GET";
const A12: &str = r"NEWGTS 'x' RENAME { 'l' 'v' } RELABEL 0 NaN NaN NaN 4.83 ADDVALUE 10 NaN NaN NaN 8.052 ADDVALUE 20 NaN NaN NaN 4.969 ADDVALUE 30 NaN NaN NaN 1.951 ADDVALUE 40 NaN NaN NaN 5.122 ADDVALUE 50 NaN NaN NaN 8.229 ADDVALUE 60 NaN NaN NaN 18.796 ADDVALUE 70 NaN NaN NaN 1.939 ADDVALUE 80 NaN NaN NaN 5.152 ADDVALUE 90 NaN NaN NaN 7.914 ADDVALUE 100 NaN NaN NaN 5.112 ADDVALUE 110 NaN NaN NaN 1.772 ADDVALUE 'g' STORE [ $g bucketizer.last 110 10 0 ] BUCKETIZE 0 GET";
const A60: &str = r"NEWGTS 'x' RENAME { 'l' 'v' } RELABEL 0 NaN NaN NaN 5.175 ADDVALUE 10 NaN NaN NaN 6.773 ADDVALUE 20 NaN NaN NaN 7.85 ADDVALUE 30 NaN NaN NaN 7.919 ADDVALUE 40 NaN NaN NaN 8.074 ADDVALUE 50 NaN NaN NaN 6.214 ADDVALUE 60 NaN NaN NaN 4.965 ADDVALUE 70 NaN NaN NaN 3.496 ADDVALUE 80 NaN NaN NaN 2.708 ADDVALUE 90 NaN NaN NaN 1.932 ADDVALUE 100 NaN NaN NaN 1.913 ADDVALUE 110 NaN NaN NaN 3.34 ADDVALUE 120 NaN NaN NaN 5.473 ADDVALUE 130 NaN NaN NaN 16.938 ADDVALUE 140 NaN NaN NaN 16.354 ADDVALUE 150 NaN NaN NaN 8.233 ADDVALUE 160 NaN NaN NaN 7.323 ADDVALUE 170 NaN NaN NaN 6.969 ADDVALUE 180 NaN NaN NaN 5.327 ADDVALUE 190 NaN NaN NaN 3.846 ADDVALUE 200 NaN NaN NaN 1.969 ADDVALUE 210 NaN NaN NaN 2.383 ADDVALUE 220 NaN NaN NaN 2.077 ADDVALUE 230 NaN NaN NaN 3.861 ADDVALUE 240 NaN NaN NaN 4.807 ADDVALUE 250 NaN NaN NaN 6.916 ADDVALUE 260 NaN NaN NaN 7.593 ADDVALUE 270 NaN NaN NaN 7.863 ADDVALUE 280 NaN NaN NaN 7.564 ADDVALUE 290 NaN NaN NaN 6.434 ADDVALUE 300 NaN NaN NaN 4.992 ADDVALUE 310 NaN NaN NaN 3.326 ADDVALUE 320 NaN NaN NaN 1.939 ADDVALUE 330 NaN NaN NaN 1.622 ADDVALUE 340 NaN NaN NaN 2.265 ADDVALUE 350 NaN NaN NaN 3.637 ADDVALUE 360 NaN NaN NaN 4.681 ADDVALUE 370 NaN NaN NaN 6.705 ADDVALUE 380 NaN NaN NaN 7.152 ADDVALUE 390 NaN NaN NaN 7.936 ADDVALUE 400 NaN NaN NaN 7.124 ADDVALUE 410 NaN NaN NaN 6.912 ADDVALUE 420 NaN NaN NaN 5.44 ADDVALUE 430 NaN NaN NaN 3.231 ADDVALUE 440 NaN NaN NaN 2.22 ADDVALUE 450 NaN NaN NaN 2.37 ADDVALUE 460 NaN NaN NaN 2.039 ADDVALUE 470 NaN NaN NaN 3.57 ADDVALUE 480 NaN NaN NaN 4.608 ADDVALUE 490 NaN NaN NaN 6.103 ADDVALUE 500 NaN NaN NaN -3.304 ADDVALUE 510 NaN NaN NaN 8.198 ADDVALUE 520 NaN NaN NaN 7.532 ADDVALUE 530 NaN NaN NaN 6.159 ADDVALUE 540 NaN NaN NaN 4.731 ADDVALUE 550 NaN NaN NaN 3.0 ADDVALUE 560 NaN NaN NaN 2.026 ADDVALUE 570 NaN NaN NaN 1.878 ADDVALUE 580 NaN NaN NaN 2.475 ADDVALUE 590 NaN NaN NaN 3.446 ADDVALUE 'g' STORE [ $g bucketizer.last 590 10 0 ] BUCKETIZE 0 GET";
const CLEAN: &str = r"NEWGTS 'x' RENAME { 'l' 'v' } RELABEL 0 NaN NaN NaN 5.029 ADDVALUE 10 NaN NaN NaN 8.033 ADDVALUE 20 NaN NaN NaN 5.042 ADDVALUE 30 NaN NaN NaN 2.005 ADDVALUE 40 NaN NaN NaN 5.027 ADDVALUE 50 NaN NaN NaN 8.04 ADDVALUE 60 NaN NaN NaN 4.978 ADDVALUE 70 NaN NaN NaN 1.986 ADDVALUE 80 NaN NaN NaN 5.014 ADDVALUE 90 NaN NaN NaN 7.99 ADDVALUE 100 NaN NaN NaN 5.026 ADDVALUE 110 NaN NaN NaN 1.982 ADDVALUE 120 NaN NaN NaN 5.002 ADDVALUE 130 NaN NaN NaN 7.974 ADDVALUE 140 NaN NaN NaN 5.028 ADDVALUE 150 NaN NaN NaN 1.986 ADDVALUE 160 NaN NaN NaN 4.992 ADDVALUE 170 NaN NaN NaN 7.997 ADDVALUE 180 NaN NaN NaN 4.981 ADDVALUE 190 NaN NaN NaN 2.046 ADDVALUE 200 NaN NaN NaN 5.019 ADDVALUE 210 NaN NaN NaN 7.969 ADDVALUE 220 NaN NaN NaN 5.015 ADDVALUE 230 NaN NaN NaN 1.955 ADDVALUE 'g' STORE [ $g bucketizer.last 230 10 0 ] BUCKETIZE 0 GET";
const FLAT: &str = r"NEWGTS 'x' RENAME { 'l' 'v' } RELABEL 0 NaN NaN NaN 5.0 ADDVALUE 10 NaN NaN NaN 5.0 ADDVALUE 20 NaN NaN NaN 5.0 ADDVALUE 30 NaN NaN NaN 5.0 ADDVALUE 40 NaN NaN NaN 5.0 ADDVALUE 50 NaN NaN NaN 5.0 ADDVALUE 60 NaN NaN NaN 5.0 ADDVALUE 70 NaN NaN NaN 5.0 ADDVALUE 80 NaN NaN NaN 5.0 ADDVALUE 90 NaN NaN NaN 5.0 ADDVALUE 100 NaN NaN NaN 5.0 ADDVALUE 110 NaN NaN NaN 5.0 ADDVALUE 120 NaN NaN NaN 5.0 ADDVALUE 130 NaN NaN NaN 5.0 ADDVALUE 140 NaN NaN NaN 5.0 ADDVALUE 150 NaN NaN NaN 5.0 ADDVALUE 160 NaN NaN NaN 5.0 ADDVALUE 170 NaN NaN NaN 5.0 ADDVALUE 180 NaN NaN NaN 5.0 ADDVALUE 190 NaN NaN NaN 5.0 ADDVALUE 200 NaN NaN NaN 5.0 ADDVALUE 210 NaN NaN NaN 5.0 ADDVALUE 220 NaN NaN NaN 5.0 ADDVALUE 230 NaN NaN NaN 5.0 ADDVALUE 'g' STORE [ $g bucketizer.last 230 10 0 ] BUCKETIZE 0 GET";
const NEG: &str = r"NEWGTS 'x' RENAME { 'l' 'v' } RELABEL 0 NaN NaN NaN 4.928 ADDVALUE 10 NaN NaN NaN 1.091 ADDVALUE 20 NaN NaN NaN 4.858 ADDVALUE 30 NaN NaN NaN -5.989 ADDVALUE 40 NaN NaN NaN 5.091 ADDVALUE 50 NaN NaN NaN 0.871 ADDVALUE 60 NaN NaN NaN 5.023 ADDVALUE 70 NaN NaN NaN 9.265 ADDVALUE 80 NaN NaN NaN 5.159 ADDVALUE 90 NaN NaN NaN 1.003 ADDVALUE 100 NaN NaN NaN 4.978 ADDVALUE 110 NaN NaN NaN 8.869 ADDVALUE 120 NaN NaN NaN 4.848 ADDVALUE 130 NaN NaN NaN 1.011 ADDVALUE 140 NaN NaN NaN 5.143 ADDVALUE 150 NaN NaN NaN 9.273 ADDVALUE 160 NaN NaN NaN 5.278 ADDVALUE 170 NaN NaN NaN 1.247 ADDVALUE 180 NaN NaN NaN 4.988 ADDVALUE 190 NaN NaN NaN 8.919 ADDVALUE 200 NaN NaN NaN 19.719 ADDVALUE 210 NaN NaN NaN 1.087 ADDVALUE 220 NaN NaN NaN 5.117 ADDVALUE 230 NaN NaN NaN 9.153 ADDVALUE 'g' STORE [ $g bucketizer.last 230 10 0 ] BUCKETIZE 0 GET";
const LOC: &str = r"NEWGTS 'x' RENAME { 'l' 'v' } RELABEL 0 40.0 2.0 100 5.242 ADDVALUE 10 41.0 3.0 101 7.761 ADDVALUE 20 42.0 4.0 102 5.067 ADDVALUE 30 43.0 5.0 103 1.814 ADDVALUE 40 44.0 6.0 104 5.16 ADDVALUE 50 45.0 7.0 105 8.274 ADDVALUE 60 46.0 8.0 106 5.155 ADDVALUE 70 47.0 9.0 107 1.853 ADDVALUE 80 48.0 10.0 108 5.184 ADDVALUE 90 49.0 11.0 109 20.165 ADDVALUE 100 50.0 12.0 110 5.103 ADDVALUE 110 51.0 13.0 111 2.012 ADDVALUE 120 52.0 14.0 112 5.265 ADDVALUE 130 53.0 15.0 113 8.18 ADDVALUE 140 54.0 16.0 114 4.801 ADDVALUE 150 55.0 17.0 115 2.128 ADDVALUE 160 56.0 18.0 116 5.196 ADDVALUE 170 57.0 19.0 117 7.742 ADDVALUE 180 58.0 20.0 118 4.775 ADDVALUE 190 59.0 21.0 119 2.056 ADDVALUE 200 60.0 22.0 120 4.778 ADDVALUE 210 61.0 23.0 121 7.931 ADDVALUE 220 62.0 24.0 122 5.288 ADDVALUE 230 63.0 25.0 123 2.198 ADDVALUE 'g' STORE [ $g bucketizer.last 230 10 0 ] BUCKETIZE 0 GET";
const NL: &str = r"NEWGTS 'x' RENAME 0 NaN NaN NaN 4.759 ADDVALUE 10 NaN NaN NaN 8.153 ADDVALUE 20 NaN NaN NaN 4.841 ADDVALUE 30 NaN NaN NaN 1.877 ADDVALUE 40 NaN NaN NaN 5.238 ADDVALUE 50 NaN NaN NaN 8.19 ADDVALUE 60 NaN NaN NaN 5.258 ADDVALUE 70 NaN NaN NaN 1.931 ADDVALUE 80 NaN NaN NaN 4.765 ADDVALUE 90 NaN NaN NaN 19.778 ADDVALUE 100 NaN NaN NaN 4.947 ADDVALUE 110 NaN NaN NaN 2.078 ADDVALUE 120 NaN NaN NaN 4.9 ADDVALUE 130 NaN NaN NaN 8.194 ADDVALUE 140 NaN NaN NaN 4.842 ADDVALUE 150 NaN NaN NaN 2.062 ADDVALUE 160 NaN NaN NaN 4.929 ADDVALUE 170 NaN NaN NaN 8.042 ADDVALUE 180 NaN NaN NaN 4.803 ADDVALUE 190 NaN NaN NaN 2.179 ADDVALUE 200 NaN NaN NaN 4.854 ADDVALUE 210 NaN NaN NaN 8.228 ADDVALUE 220 NaN NaN NaN 5.029 ADDVALUE 230 NaN NaN NaN 2.122 ADDVALUE 'g' STORE [ $g bucketizer.last 230 10 0 ] BUCKETIZE 0 GET";
const WIDE: &str = r"NEWGTS 'x' RENAME { 'l' 'v' } RELABEL 0 NaN NaN NaN 5.144 ADDVALUE 1000 NaN NaN NaN 7.815 ADDVALUE 2000 NaN NaN NaN 5.188 ADDVALUE 3000 NaN NaN NaN 2.105 ADDVALUE 4000 NaN NaN NaN 4.839 ADDVALUE 5000 NaN NaN NaN 7.715 ADDVALUE 6000 NaN NaN NaN 5.07 ADDVALUE 7000 NaN NaN NaN 1.766 ADDVALUE 8000 NaN NaN NaN 4.954 ADDVALUE 9000 NaN NaN NaN 19.952 ADDVALUE 10000 NaN NaN NaN 4.985 ADDVALUE 11000 NaN NaN NaN 1.866 ADDVALUE 12000 NaN NaN NaN 4.916 ADDVALUE 13000 NaN NaN NaN 7.909 ADDVALUE 14000 NaN NaN NaN 4.989 ADDVALUE 15000 NaN NaN NaN 2.106 ADDVALUE 16000 NaN NaN NaN 4.969 ADDVALUE 17000 NaN NaN NaN 8.031 ADDVALUE 18000 NaN NaN NaN 4.719 ADDVALUE 19000 NaN NaN NaN 2.247 ADDVALUE 20000 NaN NaN NaN 4.812 ADDVALUE 21000 NaN NaN NaN 8.091 ADDVALUE 22000 NaN NaN NaN 4.997 ADDVALUE 23000 NaN NaN NaN 1.917 ADDVALUE 'g' STORE [ $g bucketizer.last 23000 1000 0 ] BUCKETIZE 0 GET";
const NEGT: &str = r"NEWGTS 'x' RENAME -20 NaN NaN NaN 4.992 ADDVALUE -10 NaN NaN NaN 7.862 ADDVALUE 0 NaN NaN NaN 2.141 ADDVALUE 10 NaN NaN NaN 5.142 ADDVALUE 20 NaN NaN NaN 7.884 ADDVALUE 30 NaN NaN NaN 2.263 ADDVALUE 40 NaN NaN NaN 4.809 ADDVALUE 50 NaN NaN NaN 7.578 ADDVALUE 60 NaN NaN NaN 2.475 ADDVALUE 'g' STORE [ $g bucketizer.last 60 10 0 ] BUCKETIZE 0 GET";
const LONG: &str = r"NEWGTS 'x' RENAME { 'l' 'v' } RELABEL 0 NaN NaN NaN 47 ADDVALUE 10 NaN NaN NaN 77 ADDVALUE 20 NaN NaN NaN 47 ADDVALUE 30 NaN NaN NaN 21 ADDVALUE 40 NaN NaN NaN 51 ADDVALUE 50 NaN NaN NaN 81 ADDVALUE 60 NaN NaN NaN 47 ADDVALUE 70 NaN NaN NaN 19 ADDVALUE 80 NaN NaN NaN 52 ADDVALUE 90 NaN NaN NaN 80 ADDVALUE 100 NaN NaN NaN 51 ADDVALUE 110 NaN NaN NaN 22 ADDVALUE 120 NaN NaN NaN 51 ADDVALUE 130 NaN NaN NaN 80 ADDVALUE 140 NaN NaN NaN 50 ADDVALUE 150 NaN NaN NaN 20 ADDVALUE 160 NaN NaN NaN 49 ADDVALUE 170 NaN NaN NaN 80 ADDVALUE 180 NaN NaN NaN 51 ADDVALUE 190 NaN NaN NaN 21 ADDVALUE 200 NaN NaN NaN 50 ADDVALUE 210 NaN NaN NaN 81 ADDVALUE 220 NaN NaN NaN 48 ADDVALUE 230 NaN NaN NaN 22 ADDVALUE 'g' STORE [ $g bucketizer.last 230 10 0 ] BUCKETIZE 0 GET";
const PLAIN: &str = r"NEWGTS 'x' RENAME 0 NaN NaN NaN 4.85 ADDVALUE 10 NaN NaN NaN 7.723 ADDVALUE 20 NaN NaN NaN 5.074 ADDVALUE 30 NaN NaN NaN 2.176 ADDVALUE 40 NaN NaN NaN 4.857 ADDVALUE 50 NaN NaN NaN 7.907 ADDVALUE 60 NaN NaN NaN 4.732 ADDVALUE 70 NaN NaN NaN 1.709 ADDVALUE 80 NaN NaN NaN 4.89 ADDVALUE 90 NaN NaN NaN 7.889 ADDVALUE 100 NaN NaN NaN 4.812 ADDVALUE 110 NaN NaN NaN 1.87 ADDVALUE 120 NaN NaN NaN 4.972 ADDVALUE 130 NaN NaN NaN 7.73 ADDVALUE 140 NaN NaN NaN 4.796 ADDVALUE 150 NaN NaN NaN 2.172 ADDVALUE 160 NaN NaN NaN 5.083 ADDVALUE 170 NaN NaN NaN 7.936 ADDVALUE 180 NaN NaN NaN 4.947 ADDVALUE 190 NaN NaN NaN 2.108 ADDVALUE 200 NaN NaN NaN 4.781 ADDVALUE 210 NaN NaN NaN 7.708 ADDVALUE 220 NaN NaN NaN 4.916 ADDVALUE 230 NaN NaN NaN 1.949 ADDVALUE";
const GAP: &str = r"NEWGTS 'x' RENAME 0 NaN NaN NaN 4.849 ADDVALUE 10 NaN NaN NaN 7.906 ADDVALUE 20 NaN NaN NaN 5.066 ADDVALUE 30 NaN NaN NaN 1.897 ADDVALUE 40 NaN NaN NaN 5.141 ADDVALUE 50 NaN NaN NaN 8.057 ADDVALUE 60 NaN NaN NaN 5.184 ADDVALUE 80 NaN NaN NaN 5.076 ADDVALUE 90 NaN NaN NaN 7.965 ADDVALUE 100 NaN NaN NaN 4.776 ADDVALUE 110 NaN NaN NaN 1.77 ADDVALUE 120 NaN NaN NaN 5.297 ADDVALUE 130 NaN NaN NaN 8.262 ADDVALUE 140 NaN NaN NaN 4.945 ADDVALUE 150 NaN NaN NaN 2.299 ADDVALUE 160 NaN NaN NaN 4.998 ADDVALUE 170 NaN NaN NaN 8.136 ADDVALUE 180 NaN NaN NaN 5.255 ADDVALUE 190 NaN NaN NaN 2.209 ADDVALUE 200 NaN NaN NaN 5.281 ADDVALUE 210 NaN NaN NaN 8.204 ADDVALUE 220 NaN NaN NaN 5.084 ADDVALUE 230 NaN NaN NaN 2.19 ADDVALUE 'g' STORE [ $g bucketizer.last 230 10 0 ] BUCKETIZE 0 GET";
const STR: &str = r"NEWGTS 'x' RENAME 0 NaN NaN NaN 'a' ADDVALUE 10 NaN NaN NaN 'b' ADDVALUE 'g' STORE [ $g bucketizer.last 10 10 0 ] BUCKETIZE 0 GET";
const E: &str = r"NEWGTS 'x' RENAME";
const OFF: &str = r"NEWGTS 'x' RENAME { 'l' 'v' } RELABEL 0 NaN NaN NaN 5.274 ADDVALUE 10 NaN NaN NaN 8.239 ADDVALUE 20 NaN NaN NaN 5.157 ADDVALUE 30 NaN NaN NaN 1.922 ADDVALUE 40 NaN NaN NaN 5.22 ADDVALUE 50 NaN NaN NaN 8.21 ADDVALUE 60 NaN NaN NaN 4.742 ADDVALUE 70 NaN NaN NaN 2.234 ADDVALUE 80 NaN NaN NaN 4.99 ADDVALUE 90 NaN NaN NaN 8.056 ADDVALUE 100 NaN NaN NaN 5.216 ADDVALUE 110 NaN NaN NaN 2.168 ADDVALUE 120 NaN NaN NaN 5.267 ADDVALUE 130 NaN NaN NaN 8.25 ADDVALUE 140 NaN NaN NaN 5.086 ADDVALUE 150 NaN NaN NaN 2.134 ADDVALUE 160 NaN NaN NaN 4.925 ADDVALUE 170 NaN NaN NaN 8.084 ADDVALUE 180 NaN NaN NaN 5.241 ADDVALUE 190 NaN NaN NaN 1.711 ADDVALUE 200 NaN NaN NaN 5.215 ADDVALUE 210 NaN NaN NaN 8.24 ADDVALUE 220 NaN NaN NaN 5.129 ADDVALUE 230 NaN NaN NaN 2.237 ADDVALUE 'g' STORE [ $g bucketizer.last 230 10 0 ] BUCKETIZE 0 GET DUP 100 REMOVETICK 103 NaN NaN NaN 4.2 ADDVALUE";
const OFF0: &str = r"NEWGTS 'x' RENAME { 'l' 'v' } RELABEL 0 NaN NaN NaN 4.798 ADDVALUE 10 NaN NaN NaN 7.791 ADDVALUE 20 NaN NaN NaN 4.709 ADDVALUE 30 NaN NaN NaN 1.709 ADDVALUE 40 NaN NaN NaN 4.736 ADDVALUE 50 NaN NaN NaN 7.901 ADDVALUE 60 NaN NaN NaN 5.286 ADDVALUE 70 NaN NaN NaN 1.843 ADDVALUE 80 NaN NaN NaN 5.01 ADDVALUE 90 NaN NaN NaN 8.168 ADDVALUE 100 NaN NaN NaN 4.793 ADDVALUE 110 NaN NaN NaN 2.2 ADDVALUE 120 NaN NaN NaN 4.725 ADDVALUE 130 NaN NaN NaN 8.25 ADDVALUE 140 NaN NaN NaN 5.191 ADDVALUE 150 NaN NaN NaN 1.915 ADDVALUE 160 NaN NaN NaN 5.014 ADDVALUE 170 NaN NaN NaN 8.035 ADDVALUE 180 NaN NaN NaN 4.765 ADDVALUE 190 NaN NaN NaN 1.852 ADDVALUE 200 NaN NaN NaN 4.708 ADDVALUE 210 NaN NaN NaN 8.187 ADDVALUE 220 NaN NaN NaN 5.005 ADDVALUE 230 NaN NaN NaN 2.023 ADDVALUE 'g' STORE [ $g bucketizer.last 230 10 0 ] BUCKETIZE 0 GET DUP 0 REMOVETICK 3 NaN NaN NaN 4.2 ADDVALUE";
const DUP: &str = r"NEWGTS 'x' RENAME { 'l' 'v' } RELABEL 0 NaN NaN NaN 4.967 ADDVALUE 10 NaN NaN NaN 8.152 ADDVALUE 20 NaN NaN NaN 5.169 ADDVALUE 30 NaN NaN NaN 2.106 ADDVALUE 40 NaN NaN NaN 4.785 ADDVALUE 50 NaN NaN NaN 8.22 ADDVALUE 60 NaN NaN NaN 4.86 ADDVALUE 70 NaN NaN NaN 1.702 ADDVALUE 80 NaN NaN NaN 5.246 ADDVALUE 90 NaN NaN NaN 7.845 ADDVALUE 100 NaN NaN NaN 4.963 ADDVALUE 110 NaN NaN NaN 1.787 ADDVALUE 120 NaN NaN NaN 4.902 ADDVALUE 130 NaN NaN NaN 8.054 ADDVALUE 140 NaN NaN NaN 4.773 ADDVALUE 150 NaN NaN NaN 1.748 ADDVALUE 160 NaN NaN NaN 5.134 ADDVALUE 170 NaN NaN NaN 7.838 ADDVALUE 180 NaN NaN NaN 5.154 ADDVALUE 190 NaN NaN NaN 2.238 ADDVALUE 200 NaN NaN NaN 4.797 ADDVALUE 210 NaN NaN NaN 7.98 ADDVALUE 220 NaN NaN NaN 5.228 ADDVALUE 230 NaN NaN NaN 1.819 ADDVALUE 'g' STORE [ $g bucketizer.last 230 10 0 ] BUCKETIZE 0 GET DUP 100 REMOVETICK 90 NaN NaN NaN 4.2 ADDVALUE";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@CLEAN", CLEAN)
        .replace("@PLAIN", PLAIN)
        .replace("@FLAT", FLAT)
        .replace("@WIDE", WIDE)
        .replace("@NEGT", NEGT)
        .replace("@LONG", LONG)
        .replace("@OFF0", OFF0)
        .replace("@A24", A24)
        .replace("@A48", A48)
        .replace("@A40", A40)
        .replace("@A30", A30)
        .replace("@A12", A12)
        .replace("@A60", A60)
        .replace("@NEG", NEG)
        .replace("@LOC", LOC)
        .replace("@GAP", GAP)
        .replace("@STR", STR)
        .replace("@OFF", OFF)
        .replace("@DUP", DUP)
        .replace("@NL", NL)
        .replace("@E", E)
}

/// The stack left by `script`, top first, as the JSON array of the Warp 10 exec endpoint,
/// or the error message.
fn stack_json(script: &str) -> Result<String, String> {
    let mut stack = test_stack();
    stack
        .exec_checked(&format!("{script} DEPTH ->LIST REVERSE ->JSON"))
        .map_err(|err| err.to_string())?;
    match stack.pop().expect("pop should succeed") {
        Value::String(json) => Ok(json.to_string()),
        other => panic!("not a string: {other:?}"),
    }
}

/// Check every case, reporting all the mismatches at once.
fn check(cases: &[(&str, Result<&str, &str>)]) {
    let mut failures = Vec::new();
    for (template, expected) in cases {
        let script = expand(template);
        match (expected, stack_json(&script)) {
            (Ok(json), Ok(actual)) if actual == *json => {}
            (Err(message), Err(actual)) if actual.contains(message) => {}
            (expected, actual) => failures.push(format!(
                "{template}\n  expected {expected:?}\n  actual   {actual:?}"
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn hybridtest2_0() {
    check(&[
        (
            r"@A24 4 1 1 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@A24 4 1 2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A24 4 1 3 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A24 4 1 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@A24 4 2 1 HYBRIDTEST2", Ok(r"[[30,90,170]]")),
        (r"@A24 4 2 2 HYBRIDTEST2", Ok(r"[[30,70,90,130,170,210]]")),
        (r"@A24 4 2 3 HYBRIDTEST2", Ok(r"[[30,70,90,130,170,210]]")),
        (
            r"@A24 4 2 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@A24 4 3 1 HYBRIDTEST2", Ok(r"[[90,170]]")),
        (r"@A24 4 3 2 HYBRIDTEST2", Ok(r"[[90,50,170,210]]")),
        (r"@A24 4 3 3 HYBRIDTEST2", Ok(r"[[90,50,10,170,210,130]]")),
        (
            r"@A24 4 3 5 HYBRIDTEST2",
            Ok(r"[[90,50,10,70,170,210,130,180,230]]"),
        ),
        (
            r"@A48 6 1 1 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@A48 6 1 2 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@A48 6 1 3 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A48 6 1 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@A48 6 2 1 HYBRIDTEST2", Ok(r"[[50,300,380]]")),
        (r"@A48 6 2 2 HYBRIDTEST2", Ok(r"[[50,110,300,240,380,440]]")),
        (r"@A48 6 2 3 HYBRIDTEST2", Ok(r"[[50,110,300,240,380,440]]")),
        (r"@A48 6 2 5 HYBRIDTEST2", Ok(r"[[50,110,300,240,380,440]]")),
        (r"@A48 6 3 1 HYBRIDTEST2", Ok(r"[[440]]")),
        (r"@A48 6 3 2 HYBRIDTEST2", Ok(r"[[440,300]]")),
        (r"@A48 6 3 3 HYBRIDTEST2", Ok(r"[[440,300,320]]")),
        (
            r"@A48 6 3 5 HYBRIDTEST2",
            Ok(r"[[170,160,250,280,440,300,320,380,420]]"),
        ),
        (
            r"@A40 5 1 1 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@A40 5 1 2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A40 5 1 3 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A40 5 1 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@A40 5 2 1 HYBRIDTEST2", Ok(r"[[70,110,220]]")),
        (r"@A40 5 2 2 HYBRIDTEST2", Ok(r"[[70,20,110,100,220,230]]")),
        (
            r"@A40 5 2 3 HYBRIDTEST2",
            Ok(r"[[70,20,110,100,160,220,230,270]]"),
        ),
        (
            r"@A40 5 2 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@A40 5 3 1 HYBRIDTEST2", Ok(r"[[230]]")),
        (r"@A40 5 3 2 HYBRIDTEST2", Ok(r"[[230,220]]")),
        (r"@A40 5 3 3 HYBRIDTEST2", Ok(r"[[230,220,130]]")),
        (r"@A40 5 3 5 HYBRIDTEST2", Ok(r"[[230,220,130,180,120]]")),
        (
            r"@A30 5 1 1 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@A30 5 1 2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A30 5 1 3 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A30 5 1 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@A30 5 2 1 HYBRIDTEST2", Ok(r"[[120]]")),
        (r"@A30 5 2 2 HYBRIDTEST2", Ok(r"[[120,170]]")),
        (r"@A30 5 2 3 HYBRIDTEST2", Ok(r"[[120,170,230,240,210]]")),
        (
            r"@A30 5 2 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@A30 5 3 1 HYBRIDTEST2", Ok(r"[[120]]")),
        (r"@A30 5 3 2 HYBRIDTEST2", Ok(r"[[120,20]]")),
        (r"@A30 5 3 3 HYBRIDTEST2", Ok(r"[[120,20,70]]")),
        (r"@A30 5 3 5 HYBRIDTEST2", Ok(r"[[120,20,70]]")),
        (
            r"@A12 4 1 1 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@A12 4 1 2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A12 4 1 3 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A12 4 1 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@A12 4 2 1 HYBRIDTEST2", Ok(r"[[100]]")),
        (r"@A12 4 2 2 HYBRIDTEST2", Ok(r"[[100,60]]")),
        (r"@A12 4 2 3 HYBRIDTEST2", Ok(r"[[100,60]]")),
        (
            r"@A12 4 2 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@A12 4 3 1 HYBRIDTEST2", Ok(r"[[60]]")),
        (r"@A12 4 3 2 HYBRIDTEST2", Ok(r"[[60,20]]")),
        (r"@A12 4 3 3 HYBRIDTEST2", Ok(r"[[60,20,100]]")),
        (r"@A12 4 3 5 HYBRIDTEST2", Ok(r"[[60,20,100,50,0]]")),
        (
            r"@A60 12 1 1 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@A60 12 1 2 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@A60 12 1 3 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@A60 12 1 5 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (r"@A60 12 2 1 HYBRIDTEST2", Ok(r"[[250,500]]")),
        (r"@A60 12 2 2 HYBRIDTEST2", Ok(r"[[250,130,500,380]]")),
        (r"@A60 12 2 3 HYBRIDTEST2", Ok(r"[[250,130,140,500,380]]")),
        (
            r"@A60 12 2 5 HYBRIDTEST2",
            Ok(r"[[250,130,140,260,500,380]]"),
        ),
        (r"@A60 12 3 1 HYBRIDTEST2", Ok(r"[[500]]")),
        (r"@A60 12 3 2 HYBRIDTEST2", Ok(r"[[500,260]]")),
        (r"@A60 12 3 3 HYBRIDTEST2", Ok(r"[[500,260,380]]")),
        (r"@A60 12 3 5 HYBRIDTEST2", Ok(r"[[500,260,380]]")),
        (
            r"@CLEAN 4 1 1 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@CLEAN 4 1 2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@CLEAN 4 1 3 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@CLEAN 4 1 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@CLEAN 4 2 1 HYBRIDTEST2", Ok(r"[[20]]")),
        (r"@CLEAN 4 2 2 HYBRIDTEST2", Ok(r"[[20,60]]")),
        (r"@CLEAN 4 2 3 HYBRIDTEST2", Ok(r"[[20,60]]")),
        (
            r"@CLEAN 4 2 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
    ]);
}

#[test]
fn hybridtest2_1() {
    check(&[
        (r"@CLEAN 4 3 1 HYBRIDTEST2", Ok(r"[[]]")),
        (r"@CLEAN 4 3 2 HYBRIDTEST2", Ok(r"[[]]")),
        (r"@CLEAN 4 3 3 HYBRIDTEST2", Ok(r"[[50,20,10,230,150,200]]")),
        (
            r"@CLEAN 4 3 5 HYBRIDTEST2",
            Ok(r"[[50,20,10,230,150,200,120,180]]"),
        ),
        (
            r"@FLAT 4 1 1 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@FLAT 4 1 2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@FLAT 4 1 3 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@FLAT 4 1 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@FLAT 4 2 1 HYBRIDTEST2", Ok(r"[[]]")),
        (r"@FLAT 4 2 2 HYBRIDTEST2", Ok(r"[[]]")),
        (r"@FLAT 4 2 3 HYBRIDTEST2", Ok(r"[[]]")),
        (
            r"@FLAT 4 2 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@FLAT 4 3 1 HYBRIDTEST2", Ok(r"[[]]")),
        (r"@FLAT 4 3 2 HYBRIDTEST2", Ok(r"[[]]")),
        (r"@FLAT 4 3 3 HYBRIDTEST2", Ok(r"[[]]")),
        (r"@FLAT 4 3 5 HYBRIDTEST2", Ok(r"[[]]")),
        (
            r"@NEG 4 1 1 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@NEG 4 1 2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@NEG 4 1 3 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@NEG 4 1 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@NEG 4 2 1 HYBRIDTEST2", Ok(r"[[30,200]]")),
        (r"@NEG 4 2 2 HYBRIDTEST2", Ok(r"[[30,70,200,160]]")),
        (r"@NEG 4 2 3 HYBRIDTEST2", Ok(r"[[30,70,200,160]]")),
        (
            r"@NEG 4 2 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@NEG 4 3 1 HYBRIDTEST2", Ok(r"[[30,200]]")),
        (r"@NEG 4 3 2 HYBRIDTEST2", Ok(r"[[30,70,200,120]]")),
        (r"@NEG 4 3 3 HYBRIDTEST2", Ok(r"[[30,70,110,200,120,160]]")),
        (r"@NEG 4 3 5 HYBRIDTEST2", Ok(r"[[30,70,110,200,120,160]]")),
        (
            r"@LOC 4 1 1 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@LOC 4 1 2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@LOC 4 1 3 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@LOC 4 1 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@LOC 4 2 1 HYBRIDTEST2", Ok(r"[[10,130]]")),
        (r"@LOC 4 2 2 HYBRIDTEST2", Ok(r"[[10,50,130,90]]")),
        (r"@LOC 4 2 3 HYBRIDTEST2", Ok(r"[[10,50,130,90]]")),
        (
            r"@LOC 4 2 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@LOC 4 3 1 HYBRIDTEST2", Ok(r"[[90]]")),
        (r"@LOC 4 3 2 HYBRIDTEST2", Ok(r"[[90,10]]")),
        (r"@LOC 4 3 3 HYBRIDTEST2", Ok(r"[[90,10,50]]")),
        (
            r"@LOC 4 3 5 HYBRIDTEST2",
            Ok(r"[[90,10,50,30,70,200,130,180,140]]"),
        ),
        (
            r"@NL 4 1 1 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@NL 4 1 2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@NL 4 1 3 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@NL 4 1 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@NL 4 2 1 HYBRIDTEST2", Ok(r"[[90]]")),
        (r"@NL 4 2 2 HYBRIDTEST2", Ok(r"[[40,60,90,130]]")),
        (r"@NL 4 2 3 HYBRIDTEST2", Ok(r"[[40,60,0,90,130]]")),
        (
            r"@NL 4 2 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@NL 4 3 1 HYBRIDTEST2", Ok(r"[[90]]")),
        (r"@NL 4 3 2 HYBRIDTEST2", Ok(r"[[90,10,180,170]]")),
        (r"@NL 4 3 3 HYBRIDTEST2", Ok(r"[[90,10,50,180,170,140]]")),
        (
            r"@NL 4 3 5 HYBRIDTEST2",
            Ok(r"[[90,10,50,40,60,180,170,140,150]]"),
        ),
        (
            r"@WIDE 4 1 1 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@WIDE 4 1 2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@WIDE 4 1 3 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@WIDE 4 1 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@WIDE 4 2 1 HYBRIDTEST2", Ok(r"[[9000]]")),
        (r"@WIDE 4 2 2 HYBRIDTEST2", Ok(r"[[9000,13000]]")),
        (r"@WIDE 4 2 3 HYBRIDTEST2", Ok(r"[[9000,13000,15000]]")),
        (
            r"@WIDE 4 2 5 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@WIDE 4 3 1 HYBRIDTEST2", Ok(r"[[9000,18000]]")),
        (r"@WIDE 4 3 2 HYBRIDTEST2", Ok(r"[[9000,5000,18000,23000]]")),
        (
            r"@WIDE 4 3 3 HYBRIDTEST2",
            Ok(r"[[9000,5000,1000,18000,23000]]"),
        ),
        (
            r"@WIDE 4 3 5 HYBRIDTEST2",
            Ok(r"[[9000,5000,1000,18000,23000]]"),
        ),
    ]);
}

#[test]
fn hybridtest2_alpha_0() {
    check(&[
        (
            r"@A24 4 1 1 0.01 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@A24 4 1 1 0.2 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@A24 4 1 2 0.01 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A24 4 1 2 0.2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A24 4 1 3 0.01 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A24 4 1 3 0.2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A24 4 1 5 0.01 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A24 4 1 5 0.2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@A24 4 2 1 0.01 HYBRIDTEST2", Ok(r"[[30,90,170]]")),
        (r"@A24 4 2 1 0.2 HYBRIDTEST2", Ok(r"[[30,90,170]]")),
        (
            r"@A24 4 2 2 0.01 HYBRIDTEST2",
            Ok(r"[[30,70,90,130,170,210]]"),
        ),
        (
            r"@A24 4 2 2 0.2 HYBRIDTEST2",
            Ok(r"[[30,70,90,130,170,210]]"),
        ),
        (
            r"@A24 4 2 3 0.01 HYBRIDTEST2",
            Ok(r"[[30,70,90,130,170,210]]"),
        ),
        (
            r"@A24 4 2 3 0.2 HYBRIDTEST2",
            Ok(r"[[30,70,90,130,170,210]]"),
        ),
        (
            r"@A24 4 2 5 0.01 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A24 4 2 5 0.2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@A24 4 3 1 0.01 HYBRIDTEST2", Ok(r"[[90,170]]")),
        (r"@A24 4 3 1 0.2 HYBRIDTEST2", Ok(r"[[90,170]]")),
        (r"@A24 4 3 2 0.01 HYBRIDTEST2", Ok(r"[[90,50,170,210]]")),
        (r"@A24 4 3 2 0.2 HYBRIDTEST2", Ok(r"[[90,50,170,210]]")),
        (
            r"@A24 4 3 3 0.01 HYBRIDTEST2",
            Ok(r"[[90,50,10,170,210,130]]"),
        ),
        (
            r"@A24 4 3 3 0.2 HYBRIDTEST2",
            Ok(r"[[90,50,10,170,210,130]]"),
        ),
        (
            r"@A24 4 3 5 0.01 HYBRIDTEST2",
            Ok(r"[[90,50,10,70,170,210,130,180,230]]"),
        ),
        (
            r"@A24 4 3 5 0.2 HYBRIDTEST2",
            Ok(r"[[90,50,10,70,170,210,130,180,230]]"),
        ),
        (
            r"@A48 6 1 1 0.01 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@A48 6 1 1 0.2 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@A48 6 1 2 0.01 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@A48 6 1 2 0.2 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@A48 6 1 3 0.01 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A48 6 1 3 0.2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A48 6 1 5 0.01 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A48 6 1 5 0.2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@A48 6 2 1 0.01 HYBRIDTEST2", Ok(r"[[50,300,380]]")),
        (r"@A48 6 2 1 0.2 HYBRIDTEST2", Ok(r"[[50,300,380]]")),
        (
            r"@A48 6 2 2 0.01 HYBRIDTEST2",
            Ok(r"[[50,110,300,240,380,440]]"),
        ),
        (
            r"@A48 6 2 2 0.2 HYBRIDTEST2",
            Ok(r"[[50,110,300,240,380,440]]"),
        ),
        (
            r"@A48 6 2 3 0.01 HYBRIDTEST2",
            Ok(r"[[50,110,300,240,380,440]]"),
        ),
        (
            r"@A48 6 2 3 0.2 HYBRIDTEST2",
            Ok(r"[[50,110,300,240,380,440]]"),
        ),
        (
            r"@A48 6 2 5 0.01 HYBRIDTEST2",
            Ok(r"[[50,110,300,240,380,440]]"),
        ),
        (
            r"@A48 6 2 5 0.2 HYBRIDTEST2",
            Ok(r"[[50,110,160,220,130,190,230,300,240,380,440]]"),
        ),
        (r"@A48 6 3 1 0.01 HYBRIDTEST2", Ok(r"[[440]]")),
        (r"@A48 6 3 1 0.2 HYBRIDTEST2", Ok(r"[[440]]")),
        (r"@A48 6 3 2 0.01 HYBRIDTEST2", Ok(r"[[440,300]]")),
        (r"@A48 6 3 2 0.2 HYBRIDTEST2", Ok(r"[[440,300]]")),
        (r"@A48 6 3 3 0.01 HYBRIDTEST2", Ok(r"[[440,300,320]]")),
        (r"@A48 6 3 3 0.2 HYBRIDTEST2", Ok(r"[[440,300,320]]")),
        (
            r"@A48 6 3 5 0.01 HYBRIDTEST2",
            Ok(r"[[440,300,320,380,420]]"),
        ),
        (
            r"@A48 6 3 5 0.2 HYBRIDTEST2",
            Ok(r"[[170,160,250,280,180,440,300,320,380,420]]"),
        ),
        (
            r"@A40 5 1 1 0.01 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@A40 5 1 1 0.2 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@A40 5 1 2 0.01 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A40 5 1 2 0.2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A40 5 1 3 0.01 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A40 5 1 3 0.2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A40 5 1 5 0.01 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A40 5 1 5 0.2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@A40 5 2 1 0.01 HYBRIDTEST2", Ok(r"[[70,110,220]]")),
        (r"@A40 5 2 1 0.2 HYBRIDTEST2", Ok(r"[[70,110,220]]")),
        (
            r"@A40 5 2 2 0.01 HYBRIDTEST2",
            Ok(r"[[70,20,110,100,220,230]]"),
        ),
        (
            r"@A40 5 2 2 0.2 HYBRIDTEST2",
            Ok(r"[[70,20,110,100,220,230,300,350]]"),
        ),
        (
            r"@A40 5 2 3 0.01 HYBRIDTEST2",
            Ok(r"[[70,20,110,100,160,220,230,270]]"),
        ),
        (
            r"@A40 5 2 3 0.2 HYBRIDTEST2",
            Ok(r"[[70,20,0,110,100,160,220,230,270,300,350]]"),
        ),
        (
            r"@A40 5 2 5 0.01 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A40 5 2 5 0.2 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@A40 5 3 1 0.01 HYBRIDTEST2", Ok(r"[[230]]")),
        (r"@A40 5 3 1 0.2 HYBRIDTEST2", Ok(r"[[230]]")),
        (r"@A40 5 3 2 0.01 HYBRIDTEST2", Ok(r"[[230,220]]")),
        (r"@A40 5 3 2 0.2 HYBRIDTEST2", Ok(r"[[230,220]]")),
        (r"@A40 5 3 3 0.01 HYBRIDTEST2", Ok(r"[[230,220,130]]")),
        (r"@A40 5 3 3 0.2 HYBRIDTEST2", Ok(r"[[230,220,130]]")),
        (
            r"@A40 5 3 5 0.01 HYBRIDTEST2",
            Ok(r"[[230,220,130,180,120]]"),
        ),
        (
            r"@A40 5 3 5 0.2 HYBRIDTEST2",
            Ok(r"[[230,220,130,180,120]]"),
        ),
        (
            r"@A24 4 2 3 1.0 HYBRIDTEST2",
            Ok(r"[[30,70,90,130,170,210]]"),
        ),
        (r"@A24 4 2 3 0.0 HYBRIDTEST2", Ok(r"[[]]")),
        (
            r"@A24 4 2 3 -1.0 HYBRIDTEST2",
            Err(r"1.062 out of [0, 1] range"),
        ),
        (
            r"@A24 4 2 3 2.0 HYBRIDTEST2",
            Ok(r"[[30,70,0,90,130,170,210]]"),
        ),
        (
            r"@A24 4 2 3 0.05 HYBRIDTEST2",
            Ok(r"[[30,70,90,130,170,210]]"),
        ),
    ]);
}

#[test]
fn hybridtest2_errors() {
    check(&[
        (
            r"@PLAIN 4 2 3 HYBRIDTEST2",
            Err(r"HYBRIDTEST2 operates on bucketized, filled Geo Time Series."),
        ),
        (
            r"@GAP 4 2 3 HYBRIDTEST2",
            Err(r"HYBRIDTEST2 operates on bucketized, filled Geo Time Series."),
        ),
        (
            r"@LONG 4 2 3 HYBRIDTEST2",
            Err(r"GTS must be of type Double."),
        ),
        (
            r"@STR 4 2 3 HYBRIDTEST2",
            Err(r"GTS must be of type Double."),
        ),
        (
            r"@E 4 2 3 HYBRIDTEST2",
            Err(r"HYBRIDTEST2 operates on bucketized, filled Geo Time Series."),
        ),
        (
            r"NEWGTS 4 2 3 HYBRIDTEST2",
            Err(r"HYBRIDTEST2 operates on bucketized, filled Geo Time Series."),
        ),
        (
            r"@A24 4 2 4 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A24 4 2 8 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A24 4 2 7 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@A24 4 2 -1 HYBRIDTEST2", Ok(r"[[]]")),
        (r"@A24 4 2 0 HYBRIDTEST2", Ok(r"[[]]")),
        (r"@A24 4 2 -9 HYBRIDTEST2", Ok(r"[[]]")),
        (
            r"@A24 0 2 3 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A24 4 0 3 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A24 0 0 3 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@A24 0 0 -3 HYBRIDTEST2", Err(r"/ by zero")),
        (r"@A24 4 0 -3 HYBRIDTEST2", Err(r"/ by zero")),
        (r"@A24 0 2 -3 HYBRIDTEST2", Err(r"/ by zero")),
        (
            r"@A24 -4 2 3 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A24 4 -2 3 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A24 -4 -2 -3 HYBRIDTEST2",
            Err(r"Not enough buckets to make up at least one seasonal period."),
        ),
        (
            r"@A24 -4 2 -3 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A24 4 -2 -3 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (
            r"@A24 30 1 1 HYBRIDTEST2",
            Err(r"Not enough buckets to make up at least one seasonal period."),
        ),
        (
            r"@A24 25 1 1 HYBRIDTEST2",
            Err(r"Not enough buckets to make up at least one seasonal period."),
        ),
        (
            r"@A24 24 1 1 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (r"@A24 13 1 1 HYBRIDTEST2", Ok(r"[[]]")),
        (
            r"@A24 4 8 1 HYBRIDTEST2",
            Err(
                r"Not enough seasonal periods to make up at least one piece. Please use a lower number of periods per piece.",
            ),
        ),
        (
            r"@A24 4 7 1 HYBRIDTEST2",
            Err(
                r"Not enough seasonal periods to make up at least one piece. Please use a lower number of periods per piece.",
            ),
        ),
        (r"@A24 4 6 1 HYBRIDTEST2", Ok(r"[[90]]")),
        (
            r"@A24 12 1 1 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (r"@A24 12 2 1 HYBRIDTEST2", Ok(r"[[90]]")),
        (
            r"@A24 12 3 1 HYBRIDTEST2",
            Err(
                r"Not enough seasonal periods to make up at least one piece. Please use a lower number of periods per piece.",
            ),
        ),
        (r"@A24 2 12 1 HYBRIDTEST2", Ok(r"[[90]]")),
        (
            r"@A24 2 13 1 HYBRIDTEST2",
            Err(
                r"Not enough seasonal periods to make up at least one piece. Please use a lower number of periods per piece.",
            ),
        ),
        (r"@A24 3 7 1 HYBRIDTEST2", Ok(r"[[90]]")),
        (r"@A24 3 8 1 HYBRIDTEST2", Ok(r"[[90]]")),
        (
            r"@A24 3 9 1 HYBRIDTEST2",
            Err(
                r"Not enough seasonal periods to make up at least one piece. Please use a lower number of periods per piece.",
            ),
        ),
        (
            r"@OFF 4 2 3 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 103 in seasonal.ticks"),
        ),
        (
            r"@OFF0 4 2 3 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 3 in seasonal.ticks"),
        ),
        (
            r"@DUP 4 2 3 HYBRIDTEST2",
            Ok(
                r#"[[90,130,90],{"c":"x","l":{"l":"v"},"a":{},"la":0,"v":[[230,1.819],[220,5.228],[210,7.98],[200,4.797],[190,2.238],[180,5.154],[170,7.838],[160,5.134],[150,1.748],[140,4.773],[130,8.054],[120,4.902],[110,1.787],[100,4.963],[90,7.845],[80,5.246],[70,1.702],[60,4.86],[50,8.22],[40,4.785],[30,2.106],[20,5.169],[10,8.152],[0,4.967]]}]"#,
            ),
        ),
        (
            r"@OFF 4 1 1 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (
            r"@OFF0 4 1 1 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 3 in seasonal.ticks"),
        ),
        (
            r"@NEGT 3 1 1 HYBRIDTEST2",
            Err(
                r"Upper bound of number of outliers must be less than half of the number of observations per piece",
            ),
        ),
        (r"@NEGT 3 1 0 HYBRIDTEST2", Err(r"GTS must be bucketized")),
        (r"@NEGT 3 1 -1 HYBRIDTEST2", Err(r"GTS must be bucketized")),
        (
            r"@NEGT 3 1 0 0.01 HYBRIDTEST2",
            Err(r"GTS must be bucketized"),
        ),
        (r"@NEGT 3 3 1 HYBRIDTEST2", Ok(r"[[]]")),
        (r"@NEGT 3 3 0 HYBRIDTEST2", Ok(r"[[]]")),
        (r"@NEGT 2 4 1 HYBRIDTEST2", Ok(r"[[]]")),
        (r"@NEGT 4 2 1 HYBRIDTEST2", Ok(r"[[]]")),
        (
            r"@A24 4 2 3 'a' HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects a significance level (a DOUBLE) or an upper bound of the number of outliers (a LONG) on top of the stack.",
            ),
        ),
        (
            r"@A24 4 2 3 NULL HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects a significance level (a DOUBLE) or an upper bound of the number of outliers (a LONG) on top of the stack.",
            ),
        ),
        (
            r"@A24 4 2 'a' HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects a significance level (a DOUBLE) or an upper bound of the number of outliers (a LONG) on top of the stack.",
            ),
        ),
        (
            r"@A24 4 2 'a' 0.05 HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects an upper bound of the number of outliers (a LONG) below the significance level.",
            ),
        ),
        (
            r"@A24 4 'a' 3 HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects a number of periods per piece (a LONG) below the upper bound of the number of outliers.",
            ),
        ),
        (
            r"@A24 4 'a' 3 0.05 HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects a number of periods per piece (a LONG) below the upper bound of the number of outliers.",
            ),
        ),
        (
            r"@A24 'a' 2 3 HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects a number of buckets per period (a LONG) below the number of periods per piece.",
            ),
        ),
        (
            r"@A24 'a' 2 3 0.05 HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects a number of buckets per period (a LONG) below the number of periods per piece.",
            ),
        ),
        (
            r"@A24 4.0 2 3 HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects a number of buckets per period (a LONG) below the number of periods per piece.",
            ),
        ),
        (
            r"@A24 4 2.0 3 HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects a number of periods per piece (a LONG) below the upper bound of the number of outliers.",
            ),
        ),
        (
            r"@A24 4 2 3.0 HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects a number of buckets per period (a LONG) below the number of periods per piece.",
            ),
        ),
        (
            r"@A24 4 2 3.0 0.05 HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects an upper bound of the number of outliers (a LONG) below the significance level.",
            ),
        ),
        (
            r"@A24 4 2.0 3 0.05 HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects a number of periods per piece (a LONG) below the upper bound of the number of outliers.",
            ),
        ),
        (
            r"@A24 4.0 2 3 0.05 HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects a number of buckets per period (a LONG) below the number of periods per piece.",
            ),
        ),
        (
            r"@A24 2 3 HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects a number of buckets per period (a LONG) below the number of periods per piece.",
            ),
        ),
        (
            r"@A24 3 HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects a number of periods per piece (a LONG) below the upper bound of the number of outliers.",
            ),
        ),
        (
            r"@A24 HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects a significance level (a DOUBLE) or an upper bound of the number of outliers (a LONG) on top of the stack.",
            ),
        ),
        (r"HYBRIDTEST2", Err(r"Empty stack.")),
        (r"1 2 3 HYBRIDTEST2", Err(r"Empty stack.")),
        (
            r"1 2 3 4 HYBRIDTEST2",
            Err(r"HYBRIDTEST2 can only operate on Geo Time Series instances."),
        ),
        (r"1 2 3 0.05 HYBRIDTEST2", Err(r"Empty stack.")),
        (
            r"'a' 4 2 3 HYBRIDTEST2",
            Err(r"HYBRIDTEST2 can only operate on Geo Time Series instances."),
        ),
        (
            r"[ 'a' ] 4 2 3 HYBRIDTEST2",
            Err(r"HYBRIDTEST2 can only operate on Geo Time Series instances."),
        ),
        (
            r"[ 1 ] 4 2 3 HYBRIDTEST2",
            Err(r"HYBRIDTEST2 can only operate on Geo Time Series instances."),
        ),
        (
            r"{ } 4 2 3 HYBRIDTEST2",
            Err(r"HYBRIDTEST2 can only operate on Geo Time Series instances."),
        ),
        (
            r"@A24 4 2 3 { } HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects a significance level (a DOUBLE) or an upper bound of the number of outliers (a LONG) on top of the stack.",
            ),
        ),
        (
            r"@A24 4 2 3 T HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects a significance level (a DOUBLE) or an upper bound of the number of outliers (a LONG) on top of the stack.",
            ),
        ),
        (
            r"@A24 T 2 3 HYBRIDTEST2",
            Err(
                r"HYBRIDTEST2 expects a number of buckets per period (a LONG) below the number of periods per piece.",
            ),
        ),
        (
            r"[ @A24 @PLAIN ] 4 2 3 HYBRIDTEST2",
            Err(r"HYBRIDTEST2 operates on bucketized, filled Geo Time Series."),
        ),
        (
            r"[ @A24 @GAP ] 4 2 3 HYBRIDTEST2",
            Err(r"HYBRIDTEST2 operates on bucketized, filled Geo Time Series."),
        ),
        (
            r"[ @A24 @LONG ] 4 2 3 HYBRIDTEST2",
            Err(r"GTS must be of type Double."),
        ),
        (
            r"[ @A24 1 ] 4 2 3 HYBRIDTEST2",
            Err(r"HYBRIDTEST2 can only operate on Geo Time Series instances."),
        ),
    ]);
}

#[test]
fn hybridtest2_lists() {
    check(&[
        (
            r"[ @A24 @A48 ] 4 2 3 HYBRIDTEST2",
            Ok(r"[[[30,70,90,130,170,210],[50,140,80,130,300,380,320,370,400,440]]]"),
        ),
        (
            r"[ @A24 @A24 ] 4 2 3 HYBRIDTEST2",
            Ok(r"[[[30,70,90,130,170,210],[30,70,90,130,170,210]]]"),
        ),
        (r"[ ] 4 2 3 HYBRIDTEST2", Ok(r"[[]]")),
        (
            r"[ @A24 ] 4 2 3 0.05 HYBRIDTEST2",
            Ok(r"[[[30,70,90,130,170,210]]]"),
        ),
        (
            r"[ @A12 @A30 @A60 ] 4 1 1 HYBRIDTEST2",
            Err(r"Internal bug method entropyHybridTest: can't find tick 0 in seasonal.ticks"),
        ),
        (r"@A24 4 2 3 HYBRIDTEST2 SIZE", Ok(r"[6]")),
        (r"@A24 4 2 3 HYBRIDTEST2 TYPEOF", Ok(r#"["LIST"]"#)),
        (
            r"@A24 4 2 3 HYBRIDTEST2 @A24 SIZE",
            Ok(r"[24,[30,70,90,130,170,210]]"),
        ),
        (r"@A24 4 2 3 HYBRIDTEST2 DROP @A24 VALUES SIZE", Ok(r"[24]")),
        (
            r"@A24 4 2 3 HYBRIDTEST2 DROP @A24 TICKLIST",
            Ok(
                r"[[230,220,210,200,190,180,170,160,150,140,130,120,110,100,90,80,70,60,50,40,30,20,10,0]]",
            ),
        ),
        (r"@A24 4 2 3 HYBRIDTEST2 DROP @A24 BUCKETCOUNT", Ok(r"[24]")),
        (
            r"@A24 4 2 3 HYBRIDTEST2 DROP @A24 VALUES 0 GET",
            Ok(r"[1.789]"),
        ),
    ]);
}
