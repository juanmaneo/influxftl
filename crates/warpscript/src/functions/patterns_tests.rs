// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of `PATTERNS`, `PATTERNDETECTION`, `DISCORDS` and their `Z` forms.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const W: &str = r"[ NEWGTS 'w' RENAME 10 NaN NaN NaN -0.095 ADDVALUE 20 NaN NaN NaN 1.357 ADDVALUE 30 NaN NaN NaN 3.246 ADDVALUE 40 NaN NaN NaN 3.339 ADDVALUE 50 NaN NaN NaN 4.223 ADDVALUE 60 NaN NaN NaN 4.92 ADDVALUE 70 NaN NaN NaN 4.357 ADDVALUE 80 NaN NaN NaN 4.944 ADDVALUE 90 NaN NaN NaN 4.806 ADDVALUE 100 NaN NaN NaN 4.476 ADDVALUE 110 NaN NaN NaN 2.181 ADDVALUE 120 NaN NaN NaN 1.515 ADDVALUE 130 NaN NaN NaN -0.113 ADDVALUE 140 NaN NaN NaN 0.078 ADDVALUE 150 NaN NaN NaN -1.367 ADDVALUE 160 NaN NaN NaN -3.774 ADDVALUE 170 NaN NaN NaN -2.82 ADDVALUE 180 NaN NaN NaN -3.545 ADDVALUE 190 NaN NaN NaN -4.58 ADDVALUE 200 NaN NaN NaN -4.765 ADDVALUE 210 NaN NaN NaN -5.48 ADDVALUE 220 NaN NaN NaN -5.265 ADDVALUE 230 NaN NaN NaN -3.471 ADDVALUE 240 NaN NaN NaN -3.422 ADDVALUE 250 NaN NaN NaN -2.017 ADDVALUE 260 NaN NaN NaN -0.682 ADDVALUE 270 NaN NaN NaN 0.136 ADDVALUE 280 NaN NaN NaN 2.178 ADDVALUE 290 NaN NaN NaN 3.166 ADDVALUE 300 NaN NaN NaN 4.8 ADDVALUE 310 NaN NaN NaN 4.728 ADDVALUE 320 NaN NaN NaN 5.254 ADDVALUE 330 NaN NaN NaN 4.946 ADDVALUE 340 NaN NaN NaN 4.938 ADDVALUE 350 NaN NaN NaN 3.907 ADDVALUE 360 NaN NaN NaN 2.68 ADDVALUE 370 NaN NaN NaN 3.056 ADDVALUE 380 NaN NaN NaN 1.861 ADDVALUE 390 NaN NaN NaN 0.305 ADDVALUE 400 NaN NaN NaN -1.182 ADDVALUE 410 NaN NaN NaN -3.09 ADDVALUE 420 NaN NaN NaN -4.214 ADDVALUE 430 NaN NaN NaN -4.82 ADDVALUE 440 NaN NaN NaN -5.71 ADDVALUE 450 NaN NaN NaN -4.467 ADDVALUE 460 NaN NaN NaN -5.038 ADDVALUE 470 NaN NaN NaN -3.684 ADDVALUE 480 NaN NaN NaN -3.87 ADDVALUE bucketizer.last 480 10 0 ] BUCKETIZE 0 GET";
const P: &str = r"[ NEWGTS 'p' RENAME 10 NaN NaN NaN 0.0 ADDVALUE 20 NaN NaN NaN 0.327 ADDVALUE 30 NaN NaN NaN 0.618 ADDVALUE 40 NaN NaN NaN 0.841 ADDVALUE 50 NaN NaN NaN 0.972 ADDVALUE 60 NaN NaN NaN 0.995 ADDVALUE 70 NaN NaN NaN 0.909 ADDVALUE 80 NaN NaN NaN 0.723 ADDVALUE 90 NaN NaN NaN 0.457 ADDVALUE 100 NaN NaN NaN 0.141 ADDVALUE 110 NaN NaN NaN -0.191 ADDVALUE 120 NaN NaN NaN -0.501 ADDVALUE 130 NaN NaN NaN -0.757 ADDVALUE 140 NaN NaN NaN -0.929 ADDVALUE 150 NaN NaN NaN -0.999 ADDVALUE 160 NaN NaN NaN -0.959 ADDVALUE 170 NaN NaN NaN -0.813 ADDVALUE 180 NaN NaN NaN -0.578 ADDVALUE 190 NaN NaN NaN -0.279 ADDVALUE 200 NaN NaN NaN 0.05 ADDVALUE 210 NaN NaN NaN 0.374 ADDVALUE 220 NaN NaN NaN 0.657 ADDVALUE 230 NaN NaN NaN 0.867 ADDVALUE 240 NaN NaN NaN 0.983 ADDVALUE 250 NaN NaN NaN 0.989 ADDVALUE 260 NaN NaN NaN 9.0 ADDVALUE 270 NaN NaN NaN -7.5 ADDVALUE 280 NaN NaN NaN 0.412 ADDVALUE 290 NaN NaN NaN 0.091 ADDVALUE 300 NaN NaN NaN -0.24 ADDVALUE 310 NaN NaN NaN -0.544 ADDVALUE 320 NaN NaN NaN -0.789 ADDVALUE 330 NaN NaN NaN -0.946 ADDVALUE 340 NaN NaN NaN -1.0 ADDVALUE 350 NaN NaN NaN -0.943 ADDVALUE 360 NaN NaN NaN -0.783 ADDVALUE 370 NaN NaN NaN -0.537 ADDVALUE 380 NaN NaN NaN -0.231 ADDVALUE 390 NaN NaN NaN 0.1 ADDVALUE 400 NaN NaN NaN 0.42 ADDVALUE bucketizer.last 400 10 0 ] BUCKETIZE 0 GET";
const L: &str = r"[ NEWGTS 'l' RENAME 10 NaN NaN NaN 10 ADDVALUE 20 NaN NaN NaN 9 ADDVALUE 30 NaN NaN NaN 9 ADDVALUE 40 NaN NaN NaN 8 ADDVALUE 50 NaN NaN NaN 6 ADDVALUE 60 NaN NaN NaN 5 ADDVALUE 70 NaN NaN NaN 3 ADDVALUE 80 NaN NaN NaN 1 ADDVALUE 90 NaN NaN NaN 0 ADDVALUE 100 NaN NaN NaN -2 ADDVALUE 110 NaN NaN NaN -4 ADDVALUE 120 NaN NaN NaN -5 ADDVALUE 130 NaN NaN NaN -7 ADDVALUE 140 NaN NaN NaN -8 ADDVALUE 150 NaN NaN NaN -9 ADDVALUE 160 NaN NaN NaN -9 ADDVALUE 170 NaN NaN NaN -9 ADDVALUE 180 NaN NaN NaN -9 ADDVALUE 190 NaN NaN NaN -8 ADDVALUE 200 NaN NaN NaN -7 ADDVALUE 210 NaN NaN NaN -6 ADDVALUE 220 NaN NaN NaN -4 ADDVALUE 230 NaN NaN NaN -3 ADDVALUE 240 NaN NaN NaN -1 ADDVALUE 250 NaN NaN NaN 0 ADDVALUE 260 NaN NaN NaN 2 ADDVALUE 270 NaN NaN NaN 4 ADDVALUE 280 NaN NaN NaN 6 ADDVALUE 290 NaN NaN NaN 7 ADDVALUE 300 NaN NaN NaN 8 ADDVALUE 310 NaN NaN NaN 29 ADDVALUE 320 NaN NaN NaN 9 ADDVALUE 330 NaN NaN NaN 9 ADDVALUE 340 NaN NaN NaN 9 ADDVALUE 350 NaN NaN NaN 8 ADDVALUE 360 NaN NaN NaN 7 ADDVALUE 370 NaN NaN NaN 6 ADDVALUE 380 NaN NaN NaN 4 ADDVALUE 390 NaN NaN NaN 2 ADDVALUE 400 NaN NaN NaN 0 ADDVALUE bucketizer.last 400 10 0 ] BUCKETIZE 0 GET";
const Z: &str = r"[ NEWGTS 'z' RENAME 10 NaN NaN NaN 0.0 ADDVALUE 20 NaN NaN NaN -0.0 ADDVALUE 30 NaN NaN NaN 0.0 ADDVALUE 40 NaN NaN NaN 1.0 ADDVALUE 50 NaN NaN NaN -1.0 ADDVALUE 60 NaN NaN NaN 0.0 ADDVALUE 70 NaN NaN NaN 0.0 ADDVALUE 80 NaN NaN NaN -0.0 ADDVALUE 90 NaN NaN NaN 2.0 ADDVALUE 100 NaN NaN NaN -2.0 ADDVALUE 110 NaN NaN NaN 0.0 ADDVALUE 120 NaN NaN NaN 0.0 ADDVALUE bucketizer.last 120 10 0 ] BUCKETIZE 0 GET";
const C: &str = r"[ NEWGTS 'c' RENAME 10 NaN NaN NaN 3.0 ADDVALUE 20 NaN NaN NaN 3.0 ADDVALUE 30 NaN NaN NaN 3.0 ADDVALUE 40 NaN NaN NaN 3.0 ADDVALUE 50 NaN NaN NaN 3.0 ADDVALUE 60 NaN NaN NaN 3.0 ADDVALUE 70 NaN NaN NaN 3.0 ADDVALUE 80 NaN NaN NaN 3.0 ADDVALUE 90 NaN NaN NaN 3.0 ADDVALUE 100 NaN NaN NaN 3.0 ADDVALUE 110 NaN NaN NaN 3.0 ADDVALUE 120 NaN NaN NaN 3.0 ADDVALUE bucketizer.last 120 10 0 ] BUCKETIZE 0 GET";
const Y: &str = r"[ NEWGTS 'y' RENAME 10 NaN NaN NaN 0.0 ADDVALUE 20 NaN NaN NaN 0.0 ADDVALUE 30 NaN NaN NaN 0.0 ADDVALUE 40 NaN NaN NaN 5.0 ADDVALUE 50 NaN NaN NaN -5.0 ADDVALUE 60 NaN NaN NaN 0.0 ADDVALUE 70 NaN NaN NaN 0.0 ADDVALUE 80 NaN NaN NaN 0.0 ADDVALUE bucketizer.last 80 10 0 ] BUCKETIZE 0 GET";
const N: &str = r"[ NEWGTS 'n' RENAME 10 NaN NaN NaN 1.0 ADDVALUE 20 NaN NaN NaN 2.0 ADDVALUE 30 NaN NaN NaN NaN ADDVALUE 40 NaN NaN NaN 4.0 ADDVALUE 50 NaN NaN NaN 5.0 ADDVALUE 60 NaN NaN NaN 6.0 ADDVALUE 70 NaN NaN NaN 7.0 ADDVALUE 80 NaN NaN NaN 8.0 ADDVALUE bucketizer.last 80 10 0 ] BUCKETIZE 0 GET";
const U: &str = r"NEWGTS 'u' RENAME 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN 2.0 ADDVALUE";
const S: &str = r"[ NEWGTS 's' RENAME 10 NaN NaN NaN 'a' ADDVALUE 20 NaN NaN NaN 'b' ADDVALUE bucketizer.last 20 10 0 ] BUCKETIZE 0 GET";
const G: &str = r"[ NEWGTS 'g' RENAME { 'k' 'v' } RELABEL 10 1.0 2.0 5 1.0 ADDVALUE 20 NaN NaN NaN 3.0 ADDVALUE 30 NaN NaN NaN 2.0 ADDVALUE 40 NaN NaN NaN 5.0 ADDVALUE 50 NaN NaN NaN 4.0 ADDVALUE 60 NaN NaN NaN 1.0 ADDVALUE bucketizer.last 60 10 0 ] BUCKETIZE 0 GET";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@W", W)
        .replace("@P", P)
        .replace("@L", L)
        .replace("@Z", Z)
        .replace("@C", C)
        .replace("@Y", Y)
        .replace("@N", N)
        .replace("@U", U)
        .replace("@S", S)
        .replace("@G", G)
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
fn patterns() {
    check(&[
        (
            r"@W 8 4 4 PATTERNS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"Rk"],[20,"CF"],[30,"R."],[40,"PV"],[50,"s."],[60,"lV"],[70,"nV"],[80,"mV"],[90,"mV"],[100,"mV"],[110,"m."],[120,"mV"],[130,"mV"],[140,"YV"],[150,"mV"],[160,"mF"],[170,"bF"],[180,"Zk"],[190,"DF"],[200,"CF"],[210,"CF"],[220,"CF"],[230,"CF"],[240,"CF"],[250,"CF"],[260,"Ck"],[270,"RV"],[280,"RV"],[290,"QV"],[300,"t."],[310,"nV"],[320,"mV"],[330,"mV"],[340,"mV"],[350,"mV"],[360,"mV"],[370,"mV"],[380,"mV"],[390,"YF"],[400,"YF"],[410,"ak"]]}]"#,
            ),
        ),
        (
            r"@W 8 2 2 PATTERNS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"F."],[20,"F."],[30,"F."],[40,"V."],[50,"V."],[60,"V."],[70,"V."],[80,"V."],[90,"V."],[100,"V."],[110,"V."],[120,"V."],[130,"V."],[140,"V."],[150,"V."],[160,"V."],[170,"V."],[180,"F."],[190,"F."],[200,"F."],[210,"F."],[220,"F."],[230,"F."],[240,"F."],[250,"F."],[260,"F."],[270,"F."],[280,"F."],[290,"V."],[300,"V."],[310,"V."],[320,"V."],[330,"V."],[340,"V."],[350,"V."],[360,"V."],[370,"V."],[380,"V."],[390,"V."],[400,"V."],[410,"F."]]}]"#,
            ),
        ),
        (
            r"@W 6 3 8 PATTERNS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"OV."],[20,"OV."],[30,"NN."],[40,"Ok."],[50,"Pc."],[60,"mc."],[70,"kk."],[80,"o7."],[90,"aN."],[100,"aN."],[110,"o7."],[120,"o7."],[130,"a7."],[140,"a7."],[150,"o7."],[160,"o7."],[170,"aN."],[180,"gF."],[190,"Ck."],[200,"Ak."],[210,"Ak."],[220,"Ak."],[230,"Ak."],[240,"Ak."],[250,"Ak."],[260,"OV."],[270,"OV."],[280,"Ok."],[290,"NN."],[300,"m7."],[310,"kk."],[320,"o7."],[330,"aN."],[340,"o7."],[350,"oN."],[360,"o7."],[370,"o7."],[380,"aN."],[390,"aN."],[400,"a7."],[410,"_N."],[420,"dk."],[430,"Ak."]]}]"#,
            ),
        ),
        (
            r"@W 12 4 16 PATTERNS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"Now"],[20,"QVZ"],[30,"t1Z"],[40,"leg"],[50,"nVB"],[60,"mZw"],[70,"mZw"],[80,"mdF"],[90,"mh."],[100,"mdF"],[110,"mdN"],[120,"md3"],[130,"YTJ"],[140,"bBB"],[150,"_PZ"],[160,"5xc"],[170,"5rc"],[180,"DKF"],[190,"COc"],[200,"CQ."],[210,"CQF"],[220,"CHw"],[230,"CoZ"],[240,"Cok"],[250,"RWk"],[260,"Npw"],[270,"OsF"],[280,"Q7w"],[290,"t7o"],[300,"maR"],[310,"maR"],[320,"mao"],[330,"mZw"],[340,"m_k"],[350,"mE7"],[360,"mAR"],[370,"YTN"]]}]"#,
            ),
        ),
        (
            r"@W 4 4 256 PATTERNS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"Cknln."],[20,"RFwd4."],[30,"BUl2H."],[40,"RVgfq."],[50,"KPCetV"],[60,"gcq.L."],[70,"OPOh1."],[80,"sByiVk"],[90,"nWVCNF"],[100,"mf09Vk"],[110,"m6qyXk"],[120,"fBJQM."],[130,"leihvV"],[140,"mT.Ir."],[150,"eRH6VV"],[160,"QBgFhF"],[170,"mE8nYF"],[180,"mf.cj."],[190,"mLnFsV"],[200,"5xmw6F"],[210,"BwInaF"],[220,"RFap6k"],[230,"BStf5."],[240,"CHzJy."],[250,"COGbqV"],[260,"CPBUHk"],[270,"CJjnzF"],[280,"Ckmy2k"],[290,"RJcls."],[300,"DaHWD."],[310,"GzC1g."],[320,"u.UiS."],[330,"nX3jqF"],[340,"mPGdwF"],[350,"fBCIXk"],[360,"lehEvF"],[370,"mZjx6V"],[380,"maaSl."],[390,"mh0XLk"],[400,"md2pO."],[410,"mdNiNV"],[420,"b4x54V"],[430,"ctRwKV"],[440,"JtOZRk"],[450,"DWzOsk"]]}]"#,
            ),
        ),
        (
            r"@W 8 8 65536 PATTERNS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"6mKeAmAVaPPw5zhkabCAqV"],[20,"6qg3B4eER3UmvKAr64vWjF"],[30,"6nNVSNabrj8mTiql.9CALk"],[40,"UXm1Z1WGm6cjDMOjgp3xIV"],[50,"z4.gXVrPqWuY3WqYbinnok"],[60,"yAG9H9BR9g.QwFqU1tG1xF"],[70,"wEk7Np5a2tVBJfrr859xeV"],[80,"wDXDOoWnN7cI5FReD4npyV"],[90,"wBna5eZ2L18USS6aKOC_AV"],[100,"uCHlg_FbaDKrZ6FBQ8wK6."],[110,"wBXvGhlufCRRKYuFrlgwH."],[120,"wDH94yaufvfB_RMLAU.R.k"],[130,"sCnfA51ukjBtu0lJ7bbaRV"],[140,"oDXirpLbWrX7cwAS31h4pk"],[150,"w9m2aE4JAn4RQRMQkV0OXV"],[160,"sMb14C6BAhn7RToIBus4UV"],[170,"kvE3PB0telt4wS66Xu3BtF"],[180,"Wq5EKt2vAk1NG2WCNxhWvk"],[190,"2wBl0tetHUKB.E111fdLqV"],[200,"0ljzYG1khEk2vBX5wTcmYV"],[210,"2nC4hsmF7T3RLsrfTBSP9."],[220,"2lCpuXU6Joem3xG4nJEPAk"],[230,"2lSlcu1mgeeTF6YFiXHrMk"],[240,"2nCUW.GZvaKc8_bLxvh8bV"],[250,"2lSshn4GU0waos5IUIn.1k"],[260,"6mw.Ni5NGuufUWx36LNki."],[270,"6mGQTmTnE8zfVaOtrXeLH."],[280,"EV_rA_PLf7dzDqBypWARcF"],[290,"U0dZpeLqKxFxKxU8jivCNk"],[300,"y2I7fJxeeDaxX0gBXMCsDF"],[310,"w6XDmjyATgFfjn5TtMgTeV"],[320,"wDk5ydqJ36U49GvUaGCdOk"],[330,"yBFXvDMTAvGCTKetcj0eM."],[340,"yBHm45bHInfIby38nVZ0PV"],[350,"wDmN1YxvSnMb_YuSiAUdMk"],[360,"wDWD_QkOyOavWaC1l4hI0."],[370,"wBXaUTU8ea8qB2ls6IaJ8k"],[380,"sCaqx3WZWD98dgJDdFLIGV"],[390,"sCe4wp6xoeQR4Y8mOfYaoV"],[400,"kQIauxh9s4X0QWld6L9jxV"],[410,"kueCIQDJudq1gYPBxN_ApF"]]}]"#,
            ),
        ),
        (
            r"@W 10 5 4 PATTERNS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"DV."],[20,"Bw."],[30,"SN."],[40,"ss."],[50,"ss."],[60,"tV."],[70,"tc."],[80,"ls."],[90,"lF."],[100,"t.."],[110,"lN."],[120,"ls."],[130,"lF."],[140,"lJ."],[150,"n.."],[160,"Xg."],[170,"a3."],[180,"6J."],[190,"5J."],[200,"5J."],[210,"5o."],[220,"5R."],[230,"DB."],[240,"Dg."],[250,"Dg."],[260,"Dc."],[270,"RR."],[280,"Rk."],[290,"ss."],[300,"sc."],[310,"tc."],[320,"tc."],[330,"t7."],[340,"tV."],[350,"tV."],[360,"lk."],[370,"lF."],[380,"lJ."],[390,"Wg."]]}]"#,
            ),
        ),
        (
            r"@W 8 1 4 PATTERNS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"F."],[20,"F."],[30,"F."],[40,"F."],[50,"F."],[60,"F."],[70,"F."],[80,"F."],[90,"F."],[100,"F."],[110,"F."],[120,"F."],[130,"F."],[140,"F."],[150,"F."],[160,"F."],[170,"F."],[180,"F."],[190,"F."],[200,"F."],[210,"F."],[220,"F."],[230,"F."],[240,"F."],[250,"F."],[260,"F."],[270,"F."],[280,"F."],[290,"F."],[300,"F."],[310,"F."],[320,"F."],[330,"F."],[340,"F."],[350,"F."],[360,"F."],[370,"F."],[380,"F."],[390,"F."],[400,"F."],[410,"F."]]}]"#,
            ),
        ),
        (
            r"@W 8 4 4 ZPATTERNS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"xk"],[20,"zk"],[30,"zk"],[40,"zk"],[50,"zk"],[60,"zk"],[70,"vk"],[80,"vk"],[90,"nV"],[100,"nV"],[110,"Y."],[120,"Y."],[130,"1."],[140,"1."],[150,".."],[160,".."],[170,".."],[180,".."],[190,".."],[200,".F"],[210,"3F"],[220,"3k"],[230,"Bk"],[240,"Ck"],[250,"Sk"],[260,"Uk"],[270,"zk"],[280,"zk"],[290,"zk"],[300,"zk"],[310,"zk"],[320,"zk"],[330,"vk"],[340,"vV"],[350,"nV"],[360,"n."],[370,"Y."],[380,"X."],[390,"1."],[400,".."],[410,".."]]}]"#,
            ),
        ),
        (
            r"@W 8 2 2 ZPATTERNS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"k."],[20,"k."],[30,"k."],[40,"k."],[50,"k."],[60,"k."],[70,"k."],[80,"k."],[90,"V."],[100,"V."],[110,"V."],[120,"V."],[130,".."],[140,".."],[150,".."],[160,".."],[170,".."],[180,".."],[190,".."],[200,".."],[210,".."],[220,"F."],[230,"F."],[240,"F."],[250,"F."],[260,"k."],[270,"k."],[280,"k."],[290,"k."],[300,"k."],[310,"k."],[320,"k."],[330,"k."],[340,"V."],[350,"V."],[360,"V."],[370,"V."],[380,".."],[390,".."],[400,".."],[410,".."]]}]"#,
            ),
        ),
        (
            r"@W 6 3 8 ZPATTERNS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"vs."],[20,"zs."],[30,"zs."],[40,"zs."],[50,"zs."],[60,"zs."],[70,"zs."],[80,"zk."],[90,"rs."],[100,"rV."],[110,"ak."],[120,"a.."],[130,"3V."],[140,"3.."],[150,"..."],[160,"..."],[170,"..."],[180,"..."],[190,"..."],[200,"..."],[210,"..."],[220,"07."],[230,"87."],[240,"AN."],[250,"QN."],[260,"Us."],[270,"zs."],[280,"zs."],[290,"zs."],[300,"zs."],[310,"zs."],[320,"zs."],[330,"zs."],[340,"zk."],[350,"rk."],[360,"qV."],[370,"aV."],[380,"Z.."],[390,"3.."],[400,"..."],[410,"..."],[420,"..."],[430,"..."]]}]"#,
            ),
        ),
        (
            r"@W 12 4 16 ZPATTERNS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"zzR"],[20,"zzs"],[30,"zjw"],[40,"vyw"],[50,"vik"],[60,"vDs"],[70,"ngs"],[80,"nBV"],[90,"mBk"],[100,"Y7k"],[110,"X7."],[120,"V7V"],[130,"1.V"],[140,"..."],[150,"..."],[160,".03"],[170,"303"],[180,"3G3"],[190,"3IB"],[200,"BIB"],[210,"BnB"],[220,"BrR"],[230,"RrR"],[240,"SrR"],[250,"Szw"],[260,"xzw"],[270,"zzw"],[280,"zzw"],[290,"zjs"],[300,"viw"],[310,"vis"],[320,"vDk"],[330,"nBs"],[340,"nBk"],[350,"mBV"],[360,"X7k"],[370,"X7V"]]}]"#,
            ),
        ),
        (
            r"@W 4 4 256 ZPATTERNS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"Uzhvhk"],[20,"zzUrUk"],[30,"zzzzzk"],[40,"zzzzzk"],[50,"zzzzzk"],[60,"zzzzzk"],[70,"zzzzzk"],[80,"zzzzvV"],[90,"zzvzrF"],[100,"vzrjeV"],[110,"rifEKF"],[120,"fBLEek"],[130,"H7cDGV"],[140,"V.FBY."],[150,"..V71."],[160,"......"],[170,"......"],[180,"......"],[190,"......"],[200,"......"],[210,"......"],[220,"...0.F"],[230,".03I3k"],[240,"317r8k"],[250,"BKKzIV"],[260,"RvjzbF"],[270,"xrUzDk"],[280,"zzzzSk"],[290,"zzzzzk"],[300,"zzzzzk"],[310,"zzzzzk"],[320,"zzzzzk"],[330,"zzzzzk"],[340,"zzzzzk"],[350,"zzzjzk"],[360,"zizUzV"],[370,"vByzz."],[380,"m7tiu."],[390,"V.nBk."],[400,"..X7V."],[410,"......"],[420,"......"],[430,"......"],[440,"......"],[450,"......"]]}]"#,
            ),
        ),
        (
            r"@W 8 8 65536 ZPATTERNS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"UzzzjryzjrxzjpwjftyEfk"],[20,"zzzzUzxzUzzzUvxUMnwUMk"],[30,"zzzzzzzzzzzzzryzjrwzjk"],[40,"zzzzzzzzzjvyzztyUjtzUk"],[50,"zzzzzjzzzUrwzzrwzEnzzk"],[60,"zjzzzUvzyjfszzjtyUYzzk"],[70,"zUvyyjnzxUMlzzUnwj6yzk"],[80,"yjnwxUYzuijYzyzbtDExzk"],[90,"xEYsuj2ypCQ5zhvDmBUuzk"],[100,"uE2kpD2we9mCzAmSZNzozk"],[110,"oD2VeB2sJ4VPy6Vu7lzczV"],[120,"cB2.J72kcA.owE0oGYvFz."],[130,"F71.c.2VF50csD2cY6mVy."],[140,"V...F.2.VB2FkB2F5EW.w."],[150,"....V.1..71VV71VBE1.s."],[160,"..........0...00ND..k."],[170,"..........1...12kB3.V."],[180,".......0..30...6V7B0.F"],[190,"...0.F32.FB1.F3D..N2.k"],[200,".F.1.VB6.VR4.k7S.Fk50V"],[210,".k340FRE0.s90VJv.lZB2."],[220,"0kBA1kwU1GoK2Fdq0nBN5F"],[230,"2kRM4lwz3ngf5lMh2qRlBk"],[240,"6kwjAnxz8rSMCmjQ6wxYOk"],[250,"ElxUMrzzIyyjQpTrEtz6nk"],[260,"UnyzjzzzbxxUruxjUnyEbk"],[270,"zrxzUzzzEvyzjpzUzrwUEk"],[280,"zzzzzzzzUrxzUvyzzzwyUV"],[290,"zzzzzzzzzzzyzbtyzzxxz."],[300,"zzzzzzzzzzzxzErxzjzvy."],[310,"zzzzzzvzzzzvyUjvzEzrwF"],[320,"zzvyzzrzzzvrwjUqyUzisV"],[330,"zjnwzzjzzznitTvhwzzRl."],[340,"zEYszjQyzjYRmxnPtjusXF"],[350,"yE2kzDnwzE1s_fXonElk3V"],[360,"wD2VyCYsyD0kA60caEYV8."],[370,"sB2.wA2kwB2VLD2FBE2.H."],[380,"k71.s52Vs72.gB1VND1.Z."],[390,"V...kB2.k.1.N70.kB..7."],[400,"....V71.V...k.1.V730FF"],[410,"............V.....72Vk"]]}]"#,
            ),
        ),
        (
            r"@W 10 5 4 ZPATTERNS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"yw."],[20,"zw."],[30,"zw."],[40,"zw."],[50,"xw."],[60,"xw."],[70,"ts."],[80,"ts."],[90,"lk."],[100,"lk."],[110,"WV."],[120,"WV."],[130,"0.."],[140,"0.."],[150,"..."],[160,"..."],[170,"..."],[180,".3."],[190,"13."],[200,"1B."],[210,"5B."],[220,"5R."],[230,"DR."],[240,"Dw."],[250,"Tw."],[260,"Uw."],[270,"zw."],[280,"zw."],[290,"zw."],[300,"zw."],[310,"xw."],[320,"xs."],[330,"ts."],[340,"tk."],[350,"lk."],[360,"lV."],[370,"WV."],[380,"W.."],[390,"0.."]]}]"#,
            ),
        ),
        (
            r"@W 8 1 4 ZPATTERNS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"k."],[20,"k."],[30,"k."],[40,"k."],[50,"k."],[60,"k."],[70,"k."],[80,"k."],[90,"k."],[100,"V."],[110,".."],[120,".."],[130,".."],[140,".."],[150,".."],[160,".."],[170,".."],[180,".."],[190,".."],[200,".."],[210,".."],[220,".."],[230,"V."],[240,"k."],[250,"k."],[260,"k."],[270,"k."],[280,"k."],[290,"k."],[300,"k."],[310,"k."],[320,"k."],[330,"k."],[340,"k."],[350,"V."],[360,".."],[370,".."],[380,".."],[390,".."],[400,".."],[410,".."]]}]"#,
            ),
        ),
        (
            r"@L 8 4 4 PATTERNS",
            Ok(
                r#"[{"c":"l","l":{},"a":{},"la":0,"v":[[10,"mV"],[20,"mV"],[30,"mV"],[40,"mV"],[50,"mV"],[60,"mV"],[70,"mV"],[80,"mV"],[90,"mV"],[100,"m."],[110,"Y."],[120,"YF"],[130,"aF"],[140,"5k"],[150,"BF"],[160,"CF"],[170,"CF"],[180,"CF"],[190,"CF"],[200,"CF"],[210,"CF"],[220,"CF"],[230,"CF"],[240,"4k"],[250,"4k"],[260,"8k"],[270,"8k"],[280,"Gk"],[290,"Ik"],[300,"YV"],[310,"YV"],[320,"mV"],[330,"mV"]]}]"#,
            ),
        ),
        (
            r"@L 8 4 4 ZPATTERNS",
            Ok(
                r#"[{"c":"l","l":{},"a":{},"la":0,"v":[[10,"zk"],[20,"zV"],[30,"vV"],[40,"v."],[50,"n."],[60,"m."],[70,"X."],[80,"V."],[90,".."],[100,".."],[110,".."],[120,".."],[130,".."],[140,".."],[150,".."],[160,".."],[170,".."],[180,".F"],[190,"3F"],[200,"3k"],[210,"Bk"],[220,"Ck"],[230,"Sk"],[240,"Uk"],[250,"zk"],[260,"zk"],[270,"zk"],[280,"zk"],[290,"zk"],[300,"zk"],[310,"zk"],[320,"zk"],[330,"zk"]]}]"#,
            ),
        ),
        (
            r"@Z 4 2 4 ZPATTERNS",
            Ok(
                r#"[{"c":"z","l":{},"a":{},"la":0,"v":[[10,"N."],[20,"B."],[30,"Z."],[40,"B."],[50,"B."],[60,"R."],[70,"B."],[80,"c."],[90,"B."]]}]"#,
            ),
        ),
        (
            r"@Z 4 4 2 ZPATTERNS",
            Ok(
                r#"[{"c":"z","l":{},"a":{},"la":0,"v":[[10,"3."],[20,"7."],[30,"F."],[40,"V."],[50,".."],[60,"3."],[70,"7."],[80,"F."],[90,"V."]]}]"#,
            ),
        ),
        (
            r"@Z 2 1 2 ZPATTERNS",
            Ok(
                r#"[{"c":"z","l":{},"a":{},"la":0,"v":[[10,".."],[20,".."],[30,"V."],[40,".."],[50,".."],[60,".."],[70,".."],[80,"V."],[90,".."],[100,".."],[110,".."]]}]"#,
            ),
        ),
        (
            r"@Z 4 2 4 PATTERNS",
            Ok(
                r#"[{"c":"z","l":{},"a":{},"la":0,"v":[[10,"J."],[20,"B."],[30,"c."],[40,"B."],[50,"J."],[60,"J."],[70,"B."],[80,"c."],[90,"B."]]}]"#,
            ),
        ),
        (
            r"@C 4 2 4 PATTERNS",
            Ok(
                r#"[{"c":"c","l":{},"a":{},"la":0,"v":[[10,"B."],[20,"B."],[30,"B."],[40,"B."],[50,"B."],[60,"B."],[70,"B."],[80,"B."],[90,"B."]]}]"#,
            ),
        ),
        (
            r"@C 4 2 4 ZPATTERNS",
            Ok(
                r#"[{"c":"c","l":{},"a":{},"la":0,"v":[[10,"w."],[20,"w."],[30,"w."],[40,"w."],[50,"w."],[60,"w."],[70,"w."],[80,"w."],[90,"w."]]}]"#,
            ),
        ),
        (
            r"@C 4 1 4 PATTERNS",
            Ok(
                r#"[{"c":"c","l":{},"a":{},"la":0,"v":[[10,"F."],[20,"F."],[30,"F."],[40,"F."],[50,"F."],[60,"F."],[70,"F."],[80,"F."],[90,"F."]]}]"#,
            ),
        ),
        (
            r"@N 4 2 4 PATTERNS",
            Ok(
                r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[10,"w."],[20,"w."],[30,"w."],[40,"J."],[50,"J."]]}]"#,
            ),
        ),
        (
            r"@N 4 2 4 ZPATTERNS",
            Ok(
                r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[10,"w."],[20,"w."],[30,"w."],[40,"w."],[50,"w."]]}]"#,
            ),
        ),
        (
            r"@G 4 2 4 PATTERNS",
            Ok(r#"[{"c":"g","l":{"k":"v"},"a":{},"la":0,"v":[[10,"J."],[20,"J."],[30,"c."]]}]"#),
        ),
        (
            r"@G 6 3 4 PATTERNS",
            Ok(r#"[{"c":"g","l":{"k":"v"},"a":{},"la":0,"v":[[10,"I."]]}]"#),
        ),
        (
            r"@G 7 7 2 PATTERNS",
            Ok(r#"[{"c":"g","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@G 8 4 4 PATTERNS",
            Ok(r#"[{"c":"g","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@W 8 4 4.9 PATTERNS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"Rk"],[20,"CF"],[30,"R."],[40,"PV"],[50,"s."],[60,"lV"],[70,"nV"],[80,"mV"],[90,"mV"],[100,"mV"],[110,"m."],[120,"mV"],[130,"mV"],[140,"YV"],[150,"mV"],[160,"mF"],[170,"bF"],[180,"Zk"],[190,"DF"],[200,"CF"],[210,"CF"],[220,"CF"],[230,"CF"],[240,"CF"],[250,"CF"],[260,"Ck"],[270,"RV"],[280,"RV"],[290,"QV"],[300,"t."],[310,"nV"],[320,"mV"],[330,"mV"],[340,"mV"],[350,"mV"],[360,"mV"],[370,"mV"],[380,"mV"],[390,"YF"],[400,"YF"],[410,"ak"]]}]"#,
            ),
        ),
        (
            r"@W 8.5 4 4 PATTERNS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"Rk"],[20,"CF"],[30,"R."],[40,"PV"],[50,"s."],[60,"lV"],[70,"nV"],[80,"mV"],[90,"mV"],[100,"mV"],[110,"m."],[120,"mV"],[130,"mV"],[140,"YV"],[150,"mV"],[160,"mF"],[170,"bF"],[180,"Zk"],[190,"DF"],[200,"CF"],[210,"CF"],[220,"CF"],[230,"CF"],[240,"CF"],[250,"CF"],[260,"Ck"],[270,"RV"],[280,"RV"],[290,"QV"],[300,"t."],[310,"nV"],[320,"mV"],[330,"mV"],[340,"mV"],[350,"mV"],[360,"mV"],[370,"mV"],[380,"mV"],[390,"YF"],[400,"YF"],[410,"ak"]]}]"#,
            ),
        ),
        (
            r"@W 8 4 3 PATTERNS",
            Err(r"Alphabet size MUST be a power of two."),
        ),
        (
            r"@W 8 4 0 PATTERNS",
            Err(r"Alphabet size MUST be a power of two."),
        ),
        (
            r"@W 8 4 1 PATTERNS",
            Err(r"Alphabet size MUST be a power of two between 2 and 2^16"),
        ),
        (
            r"@W 8 4 -4 PATTERNS",
            Err(r"Alphabet size MUST be a power of two."),
        ),
        (
            r"@W 8 4 131072 PATTERNS",
            Err(r"Alphabet size MUST be a power of two between 2 and 2^16"),
        ),
        (r"@W 8 3 4 PATTERNS", Err(r"Wordlen MUST divide windowlen.")),
        (r"@W 8 0 4 PATTERNS", Err(r"/ by zero")),
        (r"@W 8 -4 4 PATTERNS", Err(r"-4")),
        (r"@W 8 -2 4 PATTERNS", Err(r"-2")),
        (
            r"@W 0 4 4 PATTERNS",
            Err(r"Index 48 out of bounds for length 48"),
        ),
        (
            r"@W -8 4 4 PATTERNS",
            Err(r"Index 48 out of bounds for length 48"),
        ),
        (
            r"@W 100 4 4 PATTERNS",
            Ok(r#"[{"c":"w","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@U 2 1 2 PATTERNS",
            Err(r"Function can only be applied to numeric, bucketized, filled Geo Time Series."),
        ),
        (
            r"@S 2 1 2 PATTERNS",
            Err(r"Function can only be applied to numeric, bucketized, filled Geo Time Series."),
        ),
        (
            r"NEWGTS 2 1 2 PATTERNS",
            Err(r"Function can only be applied to numeric, bucketized, filled Geo Time Series."),
        ),
        (
            r"[ @W @G ] 4 2 4 PATTERNS",
            Ok(
                r#"[[{"c":"w","l":{},"a":{},"la":0,"v":[[10,"J."],[20,"J."],[30,"J."],[40,"J."],[50,"J."],[60,"J."],[70,"c."],[80,"c."],[90,"c."],[100,"c."],[110,"c."],[120,"c."],[130,"c."],[140,"c."],[150,"c."],[160,"c."],[170,"c."],[180,"c."],[190,"c."],[200,"J."],[210,"J."],[220,"J."],[230,"J."],[240,"J."],[250,"J."],[260,"J."],[270,"J."],[280,"J."],[290,"J."],[300,"J."],[310,"c."],[320,"c."],[330,"c."],[340,"c."],[350,"c."],[360,"c."],[370,"c."],[380,"c."],[390,"c."],[400,"c."],[410,"c."],[420,"c."],[430,"J."],[440,"J."],[450,"J."]]},{"c":"g","l":{"k":"v"},"a":{},"la":0,"v":[[10,"J."],[20,"J."],[30,"c."]]}]]"#,
            ),
        ),
        (
            r"[ @W 1 ] 4 2 4 PATTERNS",
            Err(r"PATTERNS can only operate on Geo Time Series instances."),
        ),
        (
            r"1 4 2 4 PATTERNS",
            Err(r"PATTERNS can only operate on Geo Time Series instances."),
        ),
        (
            r"@W 'x' 2 4 PATTERNS",
            Err(r"PATTERNS expects a window size at the third level of the stack."),
        ),
        (
            r"@W 4 'x' 4 PATTERNS",
            Err(r"PATTERNS expects a pattern length at the second level of the stack."),
        ),
        (
            r"@W 4 2 'x' PATTERNS",
            Err(r"PATTERNS expects a quantization scale on top of the stack."),
        ),
        (r"PATTERNS", Err(r"Empty stack.")),
        (r"@W 8 4 4 PATTERNS DROP @W SIZE", Ok(r"[48]")),
        (
            r"@G 4 2 4 PATTERNS DUP BUCKETSPAN SWAP LASTBUCKET",
            Ok(r"[60,10]"),
        ),
    ]);
}

#[test]
fn patterndetection() {
    check(&[
        (
            r"@W [ 'Fl' '5F' 'FF' ] 8 4 4 PATTERNDETECTION",
            Ok(r#"[{"c":"w","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@W @W 8 4 4 PATTERNS VALUES 8 4 4 PATTERNDETECTION",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,-0.095],[20,1.357],[30,3.246],[40,3.339],[50,4.223],[60,4.92],[70,4.357],[80,4.944],[90,4.806],[100,4.476],[110,2.181],[120,1.515],[130,-0.113],[140,0.078],[150,-1.367],[160,-3.774],[170,-2.82],[180,-3.545],[190,-4.58],[200,-4.765],[210,-5.48],[220,-5.265],[230,-3.471],[240,-3.422],[250,-2.017],[260,-0.682],[270,0.136],[280,2.178],[290,3.166],[300,4.8],[310,4.728],[320,5.254],[330,4.946],[340,4.938],[350,3.907],[360,2.68],[370,3.056],[380,1.861],[390,0.305],[400,-1.182],[410,-3.09],[420,-4.214],[430,-4.82],[440,-5.71],[450,-4.467],[460,-5.038],[470,-3.684],[480,-3.87]]}]"#,
            ),
        ),
        (
            r"@W @W 8 4 4 PATTERNS VALUES 0 3 SUBLIST 8 4 4 PATTERNDETECTION",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[10,-0.095],[20,1.357],[30,3.246],[40,3.339],[50,4.223],[60,4.92],[70,4.357],[80,4.944],[90,4.806],[100,4.476],[110,2.181],[200,-4.765],[210,-5.48],[220,-5.265],[230,-3.471],[240,-3.422],[250,-2.017],[260,-0.682],[270,0.136],[280,2.178],[290,3.166],[300,4.8],[310,4.728],[320,5.254]]}]"#,
            ),
        ),
        (
            r"@W @W 6 3 8 ZPATTERNS VALUES 2 GET 1 ->LIST 6 3 8 ZPATTERNDETECTION",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[20,1.357],[30,3.246],[40,3.339],[50,4.223],[60,4.92],[70,4.357],[80,4.944],[90,4.806],[100,4.476],[110,2.181],[120,1.515],[270,0.136],[280,2.178],[290,3.166],[300,4.8],[310,4.728],[320,5.254],[330,4.946],[340,4.938],[350,3.907],[360,2.68],[370,3.056],[380,1.861]]}]"#,
            ),
        ),
        (
            r"@P @P 4 2 4 PATTERNS VALUES 20 GET 1 ->LIST 4 2 4 PATTERNDETECTION",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[10,0.0],[20,0.327],[30,0.618],[40,0.841],[50,0.972],[60,0.995],[70,0.909],[140,-0.929],[150,-0.999],[160,-0.959],[170,-0.813],[180,-0.578],[190,-0.279],[200,0.05],[210,0.374],[220,0.657],[230,0.867],[240,0.983],[250,0.989],[260,9.0],[270,-7.5],[280,0.412],[290,0.091],[300,-0.24],[330,-0.946],[340,-1.0],[350,-0.943],[360,-0.783],[370,-0.537],[380,-0.231],[390,0.1],[400,0.42]]}]"#,
            ),
        ),
        (
            r"@G [ ] 4 2 4 PATTERNDETECTION",
            Ok(r#"[{"c":"g","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@G [ 1 2 ] 4 2 4 PATTERNDETECTION",
            Ok(r#"[{"c":"g","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@G @G 4 2 4 PATTERNS VALUES 4 2 4 PATTERNDETECTION",
            Ok(
                r#"[{"c":"g","l":{"k":"v"},"a":{},"la":0,"v":[[10,0.9999999823048711,1.9999999646097422,5,1.0],[20,3.0],[30,2.0],[40,5.0],[50,4.0],[60,1.0]]}]"#,
            ),
        ),
        (
            r"@G @G 2 2 4 PATTERNS VALUES 2 2 4 PATTERNDETECTION",
            Ok(
                r#"[{"c":"g","l":{"k":"v"},"a":{},"la":0,"v":[[10,0.9999999823048711,1.9999999646097422,5,1.0],[20,3.0],[30,2.0],[40,5.0],[50,4.0],[60,1.0]]}]"#,
            ),
        ),
        (
            r"[ @G @W ] @G 4 2 4 PATTERNS VALUES 4 2 4 PATTERNDETECTION",
            Ok(
                r#"[[{"c":"g","l":{"k":"v"},"a":{},"la":0,"v":[[10,0.9999999823048711,1.9999999646097422,5,1.0],[20,3.0],[30,2.0],[40,5.0],[50,4.0],[60,1.0]]},{"c":"w","l":{},"a":{},"la":0,"v":[[10,-0.095],[20,1.357],[30,3.246],[40,3.339],[50,4.223],[60,4.92],[70,4.357],[80,4.944],[90,4.806],[100,4.476],[110,2.181],[120,1.515],[130,-0.113],[140,0.078],[150,-1.367],[160,-3.774],[170,-2.82],[180,-3.545],[190,-4.58],[200,-4.765],[210,-5.48],[220,-5.265],[230,-3.471],[240,-3.422],[250,-2.017],[260,-0.682],[270,0.136],[280,2.178],[290,3.166],[300,4.8],[310,4.728],[320,5.254],[330,4.946],[340,4.938],[350,3.907],[360,2.68],[370,3.056],[380,1.861],[390,0.305],[400,-1.182],[410,-3.09],[420,-4.214],[430,-4.82],[440,-5.71],[450,-4.467],[460,-5.038],[470,-3.684],[480,-3.87]]}]]"#,
            ),
        ),
        (
            r"@G 'x' 4 2 4 PATTERNDETECTION",
            Err(r"PATTERNDETECTION expects a list of patterns as the fourth level of the stack."),
        ),
        (
            r"@G [ ] 4 3 4 PATTERNDETECTION",
            Err(r"Wordlen MUST divide windowlen."),
        ),
        (
            r"1 [ ] 4 2 4 PATTERNDETECTION",
            Err(r"PATTERNDETECTION can only operate on Geo Time Series instances."),
        ),
        (
            r"[ @G 1 ] [ ] 4 2 4 PATTERNDETECTION",
            Err(r"PATTERNDETECTION can only operate on Geo Time Series instances."),
        ),
        (
            r"@G [ ] 0 2 4 PATTERNDETECTION",
            Err(r"Index 6 out of bounds for length 6"),
        ),
        (
            r"@G [ ] 4 2 'x' ZPATTERNDETECTION",
            Err(r"ZPATTERNDETECTION expects a quantization scale on top of the stack."),
        ),
        (
            r"@U [ ] 4 2 4 PATTERNDETECTION",
            Err(r"Function can only be applied to numeric, bucketized, filled Geo Time Series."),
        ),
        (r"PATTERNDETECTION", Err(r"Empty stack.")),
        (r"4 2 4 PATTERNDETECTION", Err(r"Empty stack.")),
    ]);
}

#[test]
fn discords() {
    check(&[
        (
            r"@P 4 2 4 1 false 0.0 DISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[260,9.0],[270,-7.5],[280,0.412],[290,0.091]]}]"#,
            ),
        ),
        (
            r"@P 4 2 4 2 false 0.0 DISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[260,9.0],[270,-7.5],[280,0.412],[290,0.091],[70,0.909],[80,0.723],[90,0.457],[100,0.141]]}]"#,
            ),
        ),
        (
            r"@P 4 2 4 3 true 0.0 DISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[290,0.091],[280,0.412],[240,0.983],[250,0.989],[260,9.0],[270,-7.5]]}]"#,
            ),
        ),
        (
            r"@P 6 3 8 2 false 0.0 DISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[250,0.989],[260,9.0],[270,-7.5],[280,0.412],[290,0.091],[300,-0.24],[20,0.327],[30,0.618],[40,0.841],[50,0.972],[60,0.995],[70,0.909]]}]"#,
            ),
        ),
        (
            r"@P 8 4 4 1 false 0.0 DISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[190,-0.279],[200,0.05],[210,0.374],[220,0.657],[230,0.867],[240,0.983],[250,0.989],[260,9.0]]}]"#,
            ),
        ),
        (
            r"@P 4 4 2 5 false 0.0 DISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[260,9.0],[270,-7.5],[50,0.972],[60,0.995],[70,0.909],[80,0.723],[90,0.457],[100,0.141],[110,-0.191],[120,-0.501],[280,0.412],[290,0.091],[300,-0.24],[310,-0.544],[320,-0.789]]}]"#,
            ),
        ),
        (
            r"@P 4 2 4 3 false 1.5 DISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[260,9.0],[270,-7.5],[280,0.412],[290,0.091]]}]"#,
            ),
        ),
        (
            r"@P 4 2 4 3 true 2.0 DISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[290,0.091],[280,0.412],[240,0.983],[250,0.989],[260,9.0],[270,-7.5]]}]"#,
            ),
        ),
        (
            r"@P 2 1 2 4 false 0.0 DISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[260,9.0],[270,-7.5],[80,0.723],[90,0.457],[100,0.141],[110,-0.191],[290,0.091],[300,-0.24]]}]"#,
            ),
        ),
        (
            r"@P 4 2 4 1 false 0.0 ZDISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[260,9.0],[270,-7.5],[280,0.412],[290,0.091]]}]"#,
            ),
        ),
        (
            r"@P 4 2 4 2 false 0.0 ZDISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[260,9.0],[270,-7.5],[280,0.412],[290,0.091],[360,-0.783],[370,-0.537],[380,-0.231],[390,0.1]]}]"#,
            ),
        ),
        (
            r"@P 4 2 4 3 true 0.0 ZDISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[290,0.091],[280,0.412],[240,0.983],[250,0.989],[260,9.0],[270,-7.5]]}]"#,
            ),
        ),
        (
            r"@P 6 3 8 2 false 0.0 ZDISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[130,-0.757],[140,-0.929],[150,-0.999],[160,-0.959],[170,-0.813],[180,-0.578],[60,0.995],[70,0.909],[80,0.723],[90,0.457],[100,0.141],[110,-0.191]]}]"#,
            ),
        ),
        (
            r"@P 8 4 4 1 false 0.0 ZDISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[130,-0.757],[140,-0.929],[150,-0.999],[160,-0.959],[170,-0.813],[180,-0.578],[190,-0.279],[200,0.05]]}]"#,
            ),
        ),
        (
            r"@P 4 4 2 5 false 0.0 ZDISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[260,9.0],[270,-7.5],[280,0.412],[290,0.091],[80,0.723],[10,0.0],[20,0.327],[30,0.618],[40,0.841],[190,-0.279],[200,0.05],[210,0.374],[220,0.657],[90,0.457],[100,0.141],[110,-0.191],[120,-0.501]]}]"#,
            ),
        ),
        (
            r"@P 4 2 4 3 false 1.5 ZDISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[260,9.0],[270,-7.5],[280,0.412],[290,0.091]]}]"#,
            ),
        ),
        (
            r"@P 4 2 4 3 true 2.0 ZDISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[290,0.091],[280,0.412],[240,0.983],[250,0.989],[260,9.0],[270,-7.5]]}]"#,
            ),
        ),
        (
            r"@P 2 1 2 4 false 0.0 ZDISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[260,9.0],[270,-7.5],[80,0.723],[90,0.457],[10,0.0],[20,0.327],[200,0.05],[210,0.374]]}]"#,
            ),
        ),
        (
            r"@W 8 4 4 2 false 0.0 DISCORDS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[40,3.339],[50,4.223],[60,4.92],[70,4.357],[80,4.944],[90,4.806],[100,4.476],[110,2.181],[400,-1.182],[410,-3.09],[420,-4.214],[430,-4.82],[440,-5.71],[450,-4.467],[460,-5.038],[470,-3.684]]}]"#,
            ),
        ),
        (
            r"@W 8 4 4 3 true 0.0 DISCORDS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[40,3.339],[50,4.223],[60,4.92],[70,4.357],[80,4.944],[90,4.806],[100,4.476],[110,2.181],[120,1.515],[130,-0.113],[400,-1.182],[410,-3.09],[420,-4.214],[430,-4.82],[440,-5.71],[450,-4.467],[460,-5.038],[470,-3.684]]}]"#,
            ),
        ),
        (
            r"@W 4 2 8 3 false 0.0 ZDISCORDS",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[260,-0.682],[270,0.136],[280,2.178],[290,3.166],[380,1.861],[390,0.305],[400,-1.182],[410,-3.09],[10,-0.095],[20,1.357],[30,3.246],[40,3.339]]}]"#,
            ),
        ),
        (
            r"@L 8 4 4 2 false 0.0 DISCORDS",
            Ok(
                r#"[{"c":"l","l":{},"a":{},"la":0,"v":[[240,-1],[250,0],[260,2],[270,4],[280,6],[290,7],[300,8],[310,29],[120,-5],[130,-7],[140,-8],[150,-9],[160,-9],[170,-9],[180,-9],[190,-8]]}]"#,
            ),
        ),
        (
            r"@L 8 4 4 2 false 0.0 ZDISCORDS",
            Ok(
                r#"[{"c":"l","l":{},"a":{},"la":0,"v":[[100,-2],[110,-4],[120,-5],[130,-7],[140,-8],[150,-9],[160,-9],[170,-9],[240,-1],[250,0],[260,2],[270,4],[280,6],[290,7],[300,8],[310,29]]}]"#,
            ),
        ),
        (
            r"@G 2 1 2 1 false 0.0 DISCORDS",
            Ok(r#"[{"c":"g","l":{"k":"v"},"a":{},"la":0,"v":[[40,5.0],[50,4.0]]}]"#),
        ),
        (
            r"@G 2 1 2 2 true 0.0 DISCORDS",
            Ok(
                r#"[{"c":"g","l":{"k":"v"},"a":{},"la":0,"v":[[40,5.0],[50,4.0],[10,0.9999999823048711,1.9999999646097422,5,1.0],[20,3.0]]}]"#,
            ),
        ),
        (
            r"@C 4 2 4 2 false 0.0 DISCORDS",
            Ok(
                r#"[{"c":"c","l":{},"a":{},"la":0,"v":[[10,3.0],[20,3.0],[30,3.0],[40,3.0],[50,3.0],[60,3.0],[70,3.0],[80,3.0]]}]"#,
            ),
        ),
        (
            r"@N 2 1 2 1 false 0.0 DISCORDS",
            Ok(r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[20,2.0],[30,NaN]]}]"#),
        ),
        (
            r"@Z 2 1 2 2 false 0.0 ZDISCORDS",
            Ok(r#"[{"c":"z","l":{},"a":{},"la":0,"v":[[30,0.0],[40,1.0],[80,-0.0],[90,2.0]]}]"#),
        ),
        (
            r"@P 4 2 4 0 false 0.0 DISCORDS",
            Ok(r#"[{"c":"p","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@P 4 2 4 -1 false 0.0 DISCORDS",
            Ok(r#"[{"c":"p","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@P 100 2 4 1 false 0.0 DISCORDS",
            Ok(r#"[{"c":"p","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@P 4 2 4 1 false NaN DISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[260,9.0],[270,-7.5],[280,0.412],[290,0.091]]}]"#,
            ),
        ),
        (
            r"@P 4 2 4 1 false 1.0E10 DISCORDS",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[260,9.0],[270,-7.5],[280,0.412],[290,0.091]]}]"#,
            ),
        ),
        (
            r"@P 4 2 4 1 false -1.0 DISCORDS",
            Err(r"DISCORDS expects a positive distance ratio."),
        ),
        (
            r"@P 4 2 4 1 false 1 DISCORDS",
            Err(r"DISCORDS expects a double distance ratio."),
        ),
        (
            r"@P 4 2 4 1 'x' 0.0 DISCORDS",
            Err(r"DISCORDS expects a boolean flag to indicate tolerance to overlapping."),
        ),
        (
            r"@P 4 2 4 1.0 false 0.0 DISCORDS",
            Err(r"DISCORDS expects an integer discord count."),
        ),
        (
            r"@P 4 2 4.0 1 false 0.0 DISCORDS",
            Err(r"DISCORDS expects an integer quantization scale."),
        ),
        (
            r"@P 4 2 3 1 false 0.0 DISCORDS",
            Err(
                r"DISCORDS expects a quantization scale which is a power of 2 greater or equal to 2.",
            ),
        ),
        (
            r"@P 4 2 1 1 false 0.0 DISCORDS",
            Err(
                r"DISCORDS expects a quantization scale which is a power of 2 greater or equal to 2.",
            ),
        ),
        (
            r"@P 4 2 0 1 false 0.0 DISCORDS",
            Err(
                r"DISCORDS expects a quantization scale which is a power of 2 greater or equal to 2.",
            ),
        ),
        (
            r"@P 4 2 -4 1 false 0.0 DISCORDS",
            Err(
                r"DISCORDS expects a quantization scale which is a power of 2 greater or equal to 2.",
            ),
        ),
        (
            r"@P 4 2.0 4 1 false 0.0 DISCORDS",
            Err(r"DISCORDS expects an integer pattern length."),
        ),
        (
            r"@P 4.0 2 4 1 false 0.0 DISCORDS",
            Err(r"DISCORDS expects an integer detection window length."),
        ),
        (r"@P 4 0 4 1 false 0.0 DISCORDS", Err(r"/ by zero")),
        (
            r"@P 4 3 4 1 false 0.0 DISCORDS",
            Err(r"DISCORDS expects pattern length to divide detection window length."),
        ),
        (
            r"@P 4 2 131072 1 false 0.0 DISCORDS",
            Err(r"Alphabet size MUST be a power of two between 2 and 2^16"),
        ),
        (
            r"@U 2 1 2 1 false 0.0 DISCORDS",
            Err(r"Function can only be applied to numeric, bucketized, filled Geo Time Series."),
        ),
        (
            r"[ @P @W ] 4 2 4 1 false 0.0 DISCORDS",
            Ok(
                r#"[[{"c":"p","l":{},"a":{},"la":0,"v":[[260,9.0],[270,-7.5],[280,0.412],[290,0.091]]},{"c":"w","l":{},"a":{},"la":0,"v":[[240,-3.422],[250,-2.017],[260,-0.682],[270,0.136]]}]]"#,
            ),
        ),
        (
            r"[ @P 1 ] 4 2 4 1 false 0.0 DISCORDS",
            Err(r"DISCORDS can only operate on Geo Time Series instances."),
        ),
        (
            r"1 4 2 4 1 false 0.0 DISCORDS",
            Err(r"DISCORDS can only operate on Geo Time Series instances."),
        ),
        (r"DISCORDS", Err(r"Empty stack.")),
        (
            r"@P 4 2 4 1 false 0.0 DISCORDS DUP BUCKETSPAN SWAP LASTBUCKET",
            Ok(r"[400,10]"),
        ),
        (r"0.0 DISCORDS", Err(r"Empty stack.")),
        (r"false 0.0 DISCORDS", Err(r"Empty stack.")),
        (r"1 false 0.0 DISCORDS", Err(r"Empty stack.")),
        (r"4 1 false 0.0 DISCORDS", Err(r"Empty stack.")),
        (r"2 4 1 false 0.0 DISCORDS", Err(r"Empty stack.")),
        (
            r"@G 2 1 2 0 false 0.0 DISCORDS",
            Ok(r#"[{"c":"g","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@G 2 1 2 -1 false 0.0 DISCORDS",
            Ok(r#"[{"c":"g","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@G 2 1 2 -3 false 0.0 DISCORDS",
            Ok(r#"[{"c":"g","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@G 2 1 2 0 true 0.0 DISCORDS",
            Ok(r#"[{"c":"g","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@Z 2 1 2 0 false 0.0 ZDISCORDS",
            Err(r"Index -1 out of bounds for length 0"),
        ),
        (
            r"@Y 2 1 2 -1 false 0.0 ZDISCORDS",
            Err(r"Index -1 out of bounds for length 1"),
        ),
        (
            r"@Y 2 1 2 -2 false 0.0 ZDISCORDS",
            Ok(r#"[{"c":"y","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@Y 2 1 2 1 false 0.0 ZDISCORDS",
            Ok(r#"[{"c":"y","l":{},"a":{},"la":0,"v":[[30,0.0],[40,5.0]]}]"#),
        ),
        (
            r"@Y 2 1 2 2 false 0.0 ZDISCORDS",
            Ok(r#"[{"c":"y","l":{},"a":{},"la":0,"v":[[40,5.0],[50,-5.0],[10,0.0],[20,0.0]]}]"#),
        ),
    ]);
}
