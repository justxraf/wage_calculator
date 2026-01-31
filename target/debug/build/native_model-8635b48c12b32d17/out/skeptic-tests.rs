extern crate skeptic;
#[test] fn readme_sect_usage_line_32() {
    let s = &format!(r####"
use native_model_macro::native_model;
use serde::{{Deserialize, Serialize}};

#[derive(Deserialize, Serialize, PartialEq, Debug)]
#[native_model(id = 1, version = 1)]
struct DotV1(u32, u32);

#[derive(Deserialize, Serialize, PartialEq, Debug)]
#[native_model(id = 1, version = 2, from = DotV1)]
struct DotV2 {{
    name: String,
    x: u64,
    y: u64,
}}

impl From<DotV1> for DotV2 {{
    fn from(dot: DotV1) -> Self {{
        DotV2 {{
            name: "".to_string(),
            x: dot.0 as u64,
            y: dot.1 as u64,
        }}
    }}
}}

impl From<DotV2> for DotV1 {{
    fn from(dot: DotV2) -> Self {{
        DotV1(dot.x as u32, dot.y as u32)
    }}
}}


fn main() {{
    {}
}}
"####, r####"// Application 1
let dot = DotV1(1, 2);
let bytes = native_model::encode(&dot).unwrap();

// Application 1 sends bytes to Application 2.

// Application 2
// We are able to decode the bytes directly into a new type DotV2 (upgrade).
let (mut dot, source_version) = native_model::decode::<DotV2>(bytes).unwrap();
assert_eq!(dot, DotV2 { 
    name: "".to_string(), 
    x: 1, 
    y: 2 
});
dot.name = "Dot".to_string();
dot.x = 5;
// For interoperability, we encode the data with the version compatible with Application 1 (downgrade).
let bytes = native_model::encode_downgrade(dot, source_version).unwrap();

// Application 2 sends bytes to Application 1.

// Application 1
let (dot, _) = native_model::decode::<DotV1>(bytes).unwrap();
assert_eq!(dot, DotV1(5, 2));
"####);
    skeptic::rt::run_test(r#"C:\Users\justx\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\native_model-0.4.20"#, r#"C:\Users\justx\Documents\Visual Studio Projects\learning_rust\wage_calculator\target\debug\build\native_model-8635b48c12b32d17\out"#, r#"x86_64-pc-windows-msvc"#, s);
}

#[test] fn readme_sect_data_model_line_104() {
    let s = &format!(r####"
use serde::{{Deserialize, Serialize}};

{}

impl From<DotV1> for DotV2 {{
    fn from(dot: DotV1) -> Self {{
        DotV2 {{
            name: "".to_string(),
            x: dot.0 as u64,
            y: dot.1 as u64,
        }}
    }}
}}

impl From<DotV2> for DotV1 {{
    fn from(dot: DotV2) -> Self {{
        DotV1(dot.x as u32, dot.y as u32)
    }}
}}

impl TryFrom<DotV2> for DotV3 {{
    type Error = anyhow::Error;

    fn try_from(dot: DotV2) -> Result<Self, Self::Error> {{
        Ok(DotV3 {{
            name: dot.name,
            cord: Cord {{ x: dot.x, y: dot.y }},
        }})
    }}
}}

impl TryFrom<DotV3> for DotV2 {{
    type Error = anyhow::Error;

    fn try_from(dot: DotV3) -> Result<Self, Self::Error> {{
        Ok(DotV2 {{
            name: dot.name,
            x: dot.cord.x,
            y: dot.cord.y,
        }})
    }}
}}



fn main() {{
    let dot = DotV1(1, 2);
    let bytes = native_model::encode(&dot).unwrap();

    let (dot_decoded, _) = native_model::decode::<DotV1>(bytes.clone()).unwrap();
    assert_eq!(dot, dot_decoded);

    let (dot_decoded, _) = native_model::decode::<DotV2>(bytes.clone()).unwrap();
    assert_eq!(
        DotV2 {{
            name: "".to_string(),
            x: 1,
            y: 2
        }},
        dot_decoded
    );

    let (dot_decoded, _) = native_model::decode::<DotV3>(bytes.clone()).unwrap();
    assert_eq!(
        DotV3 {{
            name: "".to_string(),
            cord: Cord {{ x: 1, y: 2 }}
        }},
        dot_decoded
    );
}}

"####, r####"use native_model::native_model;

#[derive(Deserialize, Serialize, PartialEq, Debug)]
#[native_model(id = 1, version = 1)]
struct DotV1(u32, u32);

#[derive(Deserialize, Serialize, PartialEq, Debug)]
#[native_model(id = 1, version = 2, from = DotV1)]
struct DotV2 {
    name: String,
    x: u64,
    y: u64,
}

// Implement the conversion between versions From<DotV1> for DotV2 and From<DotV2> for DotV1.

#[derive(Deserialize, Serialize, PartialEq, Debug)]
#[native_model(id = 1, version = 3, try_from = (DotV2, anyhow::Error))]
struct DotV3 {
    name: String,
    cord: Cord,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
struct Cord {
    x: u64,
    y: u64,
}

// Implement the conversion between versions From<DotV2> for DotV3 and From<DotV3> for DotV2.
"####);
    skeptic::rt::run_test(r#"C:\Users\justx\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\native_model-0.4.20"#, r#"C:\Users\justx\Documents\Visual Studio Projects\learning_rust\wage_calculator\target\debug\build\native_model-8635b48c12b32d17\out"#, r#"x86_64-pc-windows-msvc"#, s);
}

